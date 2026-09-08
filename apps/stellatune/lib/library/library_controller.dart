import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_state.dart';
import 'package:stellatune/platform/directory_access_service.dart';

final libraryControllerProvider =
    NotifierProvider<LibraryController, LibraryState>(LibraryController.new);

final libraryDirectoryAccessProvider = Provider<DirectoryAccessService>(
  (ref) => DirectoryAccessService.instance,
);

enum _LibraryQuery { roots, folders, excluded, playlists, liked, tracks }

class LibraryController extends Notifier<LibraryState> {
  StreamSubscription<LibraryEvent>? _sub;
  Timer? _debounce;
  Future<void>? _scheduledRefresh;
  final _queryVersions = <_LibraryQuery, int>{};
  final _queryErrors = <_LibraryQuery, String>{};
  String? _eventError;
  int _lifetime = 0;
  int _scanGeneration = 0;
  bool _scanActive = false;
  DirectoryAccessLease? _scanLease;
  Future<void>? _scanAdmission;

  @override
  LibraryState build() {
    final lifetime = ++_lifetime;
    unawaited(_finishScan());
    unawaited(_sub?.cancel());
    _debounce?.cancel();
    _queryErrors.clear();
    _eventError = null;
    _scheduledRefresh = null;

    final bridge = ref.read(libraryBridgeProvider);
    _sub = bridge.events().listen(
      (event) {
        if (ref.mounted && lifetime == _lifetime) _onEvent(event);
      },
      onError: (Object err, StackTrace st) {
        if (!ref.mounted || lifetime != _lifetime) return;
        ref
            .read(loggerProvider)
            .e('library events error: $err', error: err, stackTrace: st);
        _eventError = DiagnosticsService.instance.failureMessage(
          err,
          operation: 'library',
        );
        unawaited(_finishScan());
        state = state.copyWith(isScanning: false);
        _publishErrors();
      },
      onDone: () {
        if (!ref.mounted || lifetime != _lifetime) return;
        unawaited(_finishScan());
        state = state.copyWith(isScanning: false);
      },
    );

    ref.onDispose(() {
      if (lifetime == _lifetime) _lifetime++;
      unawaited(_finishScan());
      _debounce?.cancel();
      unawaited(_sub?.cancel());
    });

    Future.microtask(() {
      if (ref.mounted && lifetime == _lifetime) unawaited(refresh());
    });

    return const LibraryState.initial();
  }

  Future<void> addRoot(String path, {bool scanAfter = true}) async {
    if (path.trim().isEmpty) return;
    final store = ref.read(settingsStoreServiceProvider);
    final access = ref.read(libraryDirectoryAccessProvider);
    final bridge = ref.read(libraryBridgeProvider);
    final grantedPath = await access.registerDirectory(
      path: path,
      store: store,
    );
    if (!ref.mounted) return;
    final norm = _normalizePath(grantedPath);
    if (state.roots.contains(norm)) return;
    try {
      await bridge.addRoot(grantedPath);
      if (!ref.mounted) return;
      _invalidateQuery(_LibraryQuery.roots);
      state = state.copyWith(
        roots: [...state.roots, norm],
        lastError: null,
        lastLog: '',
      );
      if (scanAfter) await scanAll();
    } catch (_) {
      await access.forgetDirectory(path: grantedPath, store: store);
      rethrow;
    }
  }

  Future<void> removeRoot(String path) async {
    final norm = _normalizePath(path);
    final access = ref.read(libraryDirectoryAccessProvider);
    final store = ref.read(settingsStoreServiceProvider);
    await ref.read(libraryBridgeProvider).removeRoot(path);
    await access.forgetDirectory(path: path, store: store);
    if (!ref.mounted) return;
    _invalidateQuery(_LibraryQuery.roots);
    state = state.copyWith(
      roots: state.roots.where((r) => r != norm).toList(),
      lastError: null,
      lastLog: '',
    );
    unawaited(_refreshFolders());
  }

  Future<void> scanAll({bool force = false}) {
    if (_scanActive) return _scanAdmission ?? Future<void>.value();
    if (state.isScanning) return Future<void>.value();
    _scanActive = true;
    final generation = ++_scanGeneration;
    final lifetime = _lifetime;
    _eventError = null;
    state = state.copyWith(
      isScanning: true,
      progress: const LibraryScanProgress.zero(),
      lastFinishedMs: null,
      lastError: null,
      lastLog: '',
    );
    final store = ref.read(settingsStoreServiceProvider);
    final access = ref.read(libraryDirectoryAccessProvider);
    final bridge = ref.read(libraryBridgeProvider);
    final roots = state.roots;
    return _scanAdmission = _admitScan(
      generation: generation,
      lifetime: lifetime,
      force: force,
      roots: roots,
      store: store,
      access: access,
      bridge: bridge,
    );
  }

  Future<void> _admitScan({
    required int generation,
    required int lifetime,
    required bool force,
    required List<String> roots,
    required SettingsStore store,
    required DirectoryAccessService access,
    required LibraryBridge bridge,
  }) async {
    bool isCurrent() =>
        ref.mounted &&
        lifetime == _lifetime &&
        generation == _scanGeneration &&
        _scanActive;
    DirectoryAccessLease? lease;
    try {
      await access.ensureRootsAuthorized(roots: roots, store: store);
      if (!isCurrent()) return;
      lease = await access.acquireRoots(roots: roots, store: store);
      if (!isCurrent()) return;
      // scanAll acknowledges admission to the Rust actor, not scan completion.
      // Transfer ownership before sending, since completion can precede its ACK.
      _scanLease = lease;
      lease = null;
      if (force) {
        await bridge.scanAllForce();
      } else {
        await bridge.scanAll();
      }
    } catch (e, s) {
      if (!isCurrent()) return;
      ref
          .read(loggerProvider)
          .w('library scan failed: $e', error: e, stackTrace: s);
      _eventError = DiagnosticsService.instance.failureMessage(
        e,
        operation: 'library',
      );
      state = state.copyWith(isScanning: false);
      await _finishScan();
      if (!ref.mounted || lifetime != _lifetime) return;
      _publishErrors();
    } finally {
      await _releaseScanLease(lease);
    }
  }

  Future<void> _finishScan() {
    _scanGeneration++;
    _scanActive = false;
    _scanAdmission = null;
    final lease = _scanLease;
    _scanLease = null;
    return _releaseScanLease(lease);
  }

  Future<void> _releaseScanLease(DirectoryAccessLease? lease) async {
    try {
      await lease?.release();
    } catch (error, stack) {
      logger.w(
        'failed to release library scan access',
        error: error,
        stackTrace: stack,
      );
    }
  }

  void selectFolder(String folder) {
    final norm = _normalizePath(folder);
    if (state.selectedFolder == norm && state.selectedPlaylistId == null) {
      return;
    }
    // Selecting a folder defaults to recursive listing (include subfolders).
    state = state.copyWith(
      selectedFolder: norm,
      selectedPlaylistId: null,
      includeSubfolders: true,
      lastError: null,
    );
    unawaited(_refreshTracks());
  }

  void selectAllMusic() {
    if (state.selectedFolder.isEmpty && state.selectedPlaylistId == null) {
      return;
    }
    state = state.copyWith(
      selectedFolder: '',
      selectedPlaylistId: null,
      lastError: null,
    );
    unawaited(_refreshTracks());
  }

  void selectPlaylist(int playlistId) {
    if (playlistId <= 0) return;
    if (state.selectedPlaylistId == playlistId) return;
    state = state.copyWith(
      selectedPlaylistId: playlistId,
      selectedFolder: '',
      lastError: null,
    );
    unawaited(_refreshTracks());
  }

  void toggleIncludeSubfolders() {
    state = state.copyWith(includeSubfolders: !state.includeSubfolders);
    unawaited(_refreshTracks());
  }

  Future<void> deleteFolder(String folder) async {
    final norm = _normalizePath(folder);
    if (norm.isEmpty) return;

    // If the current selection is removed, fall back to "All music".
    if (state.selectedFolder == norm ||
        state.selectedFolder.startsWith('$norm/')) {
      state = state.copyWith(
        selectedFolder: '',
        includeSubfolders: false,
        lastError: null,
      );
    }

    await ref.read(libraryBridgeProvider).deleteFolder(norm);
  }

  Future<void> restoreFolder(String folder) async {
    final norm = _normalizePath(folder);
    if (norm.isEmpty) return;
    await ref.read(libraryBridgeProvider).restoreFolder(norm);
  }

  void setQuery(String query) {
    final q = query.trim();
    state = state.copyWith(query: q, lastError: null);
    // Invalidate immediately, including A -> B -> A within the debounce delay.
    _invalidateQuery(_LibraryQuery.tracks);

    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 250), () {
      unawaited(_refreshTracks());
    });
  }

  /// Merge notifications in one event turn. A later refresh may overtake an
  /// in-flight one; each query has its own version for committing results.
  Future<void> refresh() {
    final lifetime = _lifetime;
    return _scheduledRefresh ??= Future<void>.microtask(() async {
      if (!ref.mounted || lifetime != _lifetime) return;
      _scheduledRefresh = null;
      await Future.wait([
        _refreshRoots(),
        _refreshFolders(),
        _refreshExcludedFolders(),
        _refreshPlaylists(),
        _refreshLikedTrackIds(),
        _refreshTracks(),
      ]);
    });
  }

  Future<void> _refreshRoots() => _runQuery(
    _LibraryQuery.roots,
    // Bookmark synchronization already runs at bootstrap and on acquisition.
    // Keep query responses free of persistent side effects before version checks.
    () => ref.read(libraryBridgeProvider).listRoots(),
    (roots) =>
        state = state.copyWith(roots: roots.map(_normalizePath).toList()),
  );

  Future<void> _refreshFolders() => _runQuery(
    _LibraryQuery.folders,
    () => ref.read(libraryBridgeProvider).listFolders(),
    (folders) =>
        state = state.copyWith(folders: folders.map(_normalizePath).toList()),
  );

  Future<void> _refreshExcludedFolders() => _runQuery(
    _LibraryQuery.excluded,
    () => ref.read(libraryBridgeProvider).listExcludedFolders(),
    (folders) => state = state.copyWith(
      excludedFolders: folders.map(_normalizePath).toList(),
    ),
  );

  Future<void> _refreshPlaylists() => _runQuery(
    _LibraryQuery.playlists,
    () => ref.read(libraryBridgeProvider).listPlaylists(),
    (playlists) {
      final selected = state.selectedPlaylistId;
      final selectedExists =
          selected == null || playlists.any((p) => p.id == selected);
      state = state.copyWith(
        playlists: playlists,
        selectedPlaylistId: selectedExists ? selected : null,
      );
      if (!selectedExists) unawaited(_refreshTracks());
    },
  );

  Future<void> _refreshLikedTrackIds() => _runQuery(
    _LibraryQuery.liked,
    () => ref.read(libraryBridgeProvider).listLikedTrackIds(),
    (ids) => state = state.copyWith(likedTrackIds: ids.toSet()),
  );

  Future<void> _refreshTracks() {
    if (!ref.mounted) return Future.value();
    final bridge = ref.read(libraryBridgeProvider);
    final playlist = state.selectedPlaylistId;
    final folder = state.selectedFolder;
    final recursive = state.includeSubfolders;
    final query = state.query;
    return _runQuery(
      _LibraryQuery.tracks,
      () => playlist != null
          ? bridge.listPlaylistTracks(playlistId: playlist, query: query)
          : bridge.listTracks(
              folder: folder,
              recursive: recursive,
              query: query,
            ),
      (items) => state = state.copyWith(results: items),
      isApplicable: () =>
          state.selectedPlaylistId == playlist &&
          state.selectedFolder == folder &&
          state.includeSubfolders == recursive &&
          state.query == query,
    );
  }

  Future<void> _runQuery<T>(
    _LibraryQuery kind,
    Future<T> Function() load,
    void Function(T) commit, {
    bool Function()? isApplicable,
  }) async {
    if (!ref.mounted) return;
    final lifetime = _lifetime;
    final version = (_queryVersions[kind] ?? 0) + 1;
    _queryVersions[kind] = version;
    bool isCurrent() =>
        ref.mounted &&
        lifetime == _lifetime &&
        _queryVersions[kind] == version &&
        (isApplicable?.call() ?? true);
    try {
      final value = await load();
      if (!isCurrent()) return;
      _queryErrors.remove(kind);
      commit(value);
      _publishErrors();
    } catch (error, stack) {
      if (!isCurrent()) return;
      // Other concurrent queries must not erase this failure on success.
      _queryErrors.remove(kind);
      _queryErrors[kind] = DiagnosticsService.instance.failureMessage(
        error,
        operation: 'library',
      );
      _publishErrors();
      ref
          .read(loggerProvider)
          .w(
            'library ${kind.name} query failed',
            error: error,
            stackTrace: stack,
          );
    }
  }

  void _invalidateQuery(_LibraryQuery kind) {
    _queryVersions.update(kind, (value) => value + 1, ifAbsent: () => 1);
  }

  void _publishErrors() {
    final error = _eventError ?? _queryErrors.values.lastOrNull;
    if (state.lastError != error) state = state.copyWith(lastError: error);
  }

  Future<void> createPlaylist(String name) {
    return ref.read(libraryBridgeProvider).createPlaylist(name);
  }

  Future<void> renamePlaylist(int id, String name) {
    return ref.read(libraryBridgeProvider).renamePlaylist(id: id, name: name);
  }

  Future<void> deletePlaylist(int id) {
    if (state.selectedPlaylistId == id) {
      state = state.copyWith(selectedPlaylistId: null, selectedFolder: '');
    }
    return ref.read(libraryBridgeProvider).deletePlaylist(id: id);
  }

  Future<void> addTrackToPlaylist(int playlistId, int trackId) {
    return ref
        .read(libraryBridgeProvider)
        .addTrackToPlaylist(playlistId: playlistId, trackId: trackId);
  }

  Future<void> addTracksToPlaylist({
    required int playlistId,
    required List<int> trackIds,
  }) {
    return ref
        .read(libraryBridgeProvider)
        .addTracksToPlaylist(playlistId: playlistId, trackIds: trackIds);
  }

  Future<void> removeTrackFromPlaylist(int playlistId, int trackId) {
    return ref
        .read(libraryBridgeProvider)
        .removeTrackFromPlaylist(playlistId: playlistId, trackId: trackId);
  }

  Future<void> removeTracksFromPlaylist({
    required int playlistId,
    required List<int> trackIds,
  }) {
    return ref
        .read(libraryBridgeProvider)
        .removeTracksFromPlaylist(playlistId: playlistId, trackIds: trackIds);
  }

  Future<void> moveTrackInPlaylist({
    required int playlistId,
    required int trackId,
    required int newIndex,
  }) {
    return ref
        .read(libraryBridgeProvider)
        .moveTrackInPlaylist(
          playlistId: playlistId,
          trackId: trackId,
          newIndex: newIndex,
        );
  }

  Future<void> setTrackLiked(int trackId, bool liked) {
    return ref
        .read(libraryBridgeProvider)
        .setTrackLiked(trackId: trackId, liked: liked);
  }

  void _onEvent(LibraryEvent event) {
    if (!ref.mounted) return;
    event.maybeWhen(
      changed: () {
        unawaited(refresh());
      },
      scanProgress: (scanned, updated, skipped, errors) {
        state = state.copyWith(
          isScanning: true,
          progress: LibraryScanProgress(
            scanned: scanned.toInt(),
            updated: updated.toInt(),
            skipped: skipped.toInt(),
            errors: errors.toInt(),
          ),
        );
      },
      scanFinished: (durationMs, scanned, updated, skipped, errors) {
        unawaited(_finishScan());
        state = state.copyWith(
          isScanning: false,
          lastFinishedMs: durationMs.toInt(),
          progress: LibraryScanProgress(
            scanned: scanned.toInt(),
            updated: updated.toInt(),
            skipped: skipped.toInt(),
            errors: errors.toInt(),
          ),
        );
        unawaited(_refreshFolders());
        unawaited(_refreshTracks());
      },
      error: (message) {
        DiagnosticsService.instance.report(message, operation: 'library');
        unawaited(_finishScan());
        ref.read(loggerProvider).e(message);
        _eventError = DiagnosticsService.instance.messageFor(
          message,
          operation: 'library',
        );
        state = state.copyWith(isScanning: false);
        _publishErrors();
      },
      log: (message) {
        ref.read(loggerProvider).d(message);
        state = state.copyWith(lastLog: message);
      },
      orElse: () {},
    );
  }

  static String _normalizePath(String input) {
    var s = input.replaceAll('\\', '/');
    while (s.endsWith('/')) {
      s = s.substring(0, s.length - 1);
    }
    return s;
  }
}
