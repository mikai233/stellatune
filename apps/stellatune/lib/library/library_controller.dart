import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_state.dart';

final libraryControllerProvider =
    NotifierProvider<LibraryController, LibraryState>(LibraryController.new);

class LibraryController extends Notifier<LibraryState> {
  StreamSubscription<LibraryEvent>? _sub;
  Timer? _debounce;

  @override
  LibraryState build() {
    unawaited(_sub?.cancel());
    _debounce?.cancel();

    final bridge = ref.read(libraryBridgeProvider);
    _sub = bridge.events().listen(
      _onEvent,
      onError: (Object err, StackTrace st) {
        ref
            .read(loggerProvider)
            .e('library events error: $err', error: err, stackTrace: st);
        state = state.copyWith(lastError: err.toString());
      },
    );

    ref.onDispose(() {
      _debounce?.cancel();
      unawaited(_sub?.cancel());
    });

    // Hydrate persisted roots / folders / tracks on startup.
    //
    // Important: schedule the initial requests after `build()` returns so we
    // don't risk receiving events before the initial state is installed.
    Future.microtask(() {
      unawaited(bridge.listRoots());
      unawaited(bridge.listFolders());
      unawaited(bridge.listExcludedFolders());
      unawaited(bridge.listPlaylists());
      unawaited(bridge.listLikedTrackIds());
      unawaited(_refreshTracks());
    });

    return const LibraryState.initial();
  }

  Future<void> addRoot(String path, {bool scanAfter = true}) async {
    if (path.trim().isEmpty) return;
    final norm = _normalizePath(path);
    if (state.roots.contains(norm)) return;

    state = state.copyWith(
      roots: [...state.roots, norm],
      lastError: null,
      lastLog: '',
    );

    await ref.read(libraryBridgeProvider).addRoot(path);
    if (scanAfter) await scanAll();
  }

  Future<void> removeRoot(String path) async {
    final norm = _normalizePath(path);
    state = state.copyWith(
      roots: state.roots.where((r) => r != norm).toList(),
      lastError: null,
      lastLog: '',
    );
    await ref.read(libraryBridgeProvider).removeRoot(path);
    unawaited(ref.read(libraryBridgeProvider).listFolders());
  }

  Future<void> scanAll({bool force = false}) async {
    state = state.copyWith(
      isScanning: true,
      progress: const LibraryScanProgress.zero(),
      lastFinishedMs: null,
      lastError: null,
      lastLog: '',
    );
    if (force) {
      await ref.read(libraryBridgeProvider).scanAllForce();
    } else {
      await ref.read(libraryBridgeProvider).scanAll();
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

    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 250), () {
      unawaited(_refreshTracks());
    });
  }

  Future<void> _refreshTracks() {
    final playlistId = state.selectedPlaylistId;
    if (playlistId != null) {
      return ref
          .read(libraryBridgeProvider)
          .listPlaylistTracks(playlistId: playlistId, query: state.query);
    }
    return ref
        .read(libraryBridgeProvider)
        .listTracks(
          folder: state.selectedFolder,
          recursive: state.includeSubfolders,
          query: state.query,
        );
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
    event.maybeWhen(
      roots: (paths) {
        final roots = paths.map(_normalizePath).toList();
        state = state.copyWith(roots: roots, lastError: null);
        unawaited(ref.read(libraryBridgeProvider).listFolders());
      },
      folders: (paths) {
        state = state.copyWith(folders: paths.map(_normalizePath).toList());
      },
      excludedFolders: (paths) {
        state = state.copyWith(
          excludedFolders: paths.map(_normalizePath).toList(),
        );
      },
      changed: () {
        unawaited(ref.read(libraryBridgeProvider).listRoots());
        unawaited(ref.read(libraryBridgeProvider).listFolders());
        unawaited(ref.read(libraryBridgeProvider).listExcludedFolders());
        unawaited(ref.read(libraryBridgeProvider).listPlaylists());
        unawaited(ref.read(libraryBridgeProvider).listLikedTrackIds());
        unawaited(_refreshTracks());
      },
      tracks: (folder, recursive, query, items) {
        if (state.selectedPlaylistId != null) return;
        final folderN = _normalizePath(folder);
        if (folderN != state.selectedFolder) return;
        if (query != state.query) return;
        if (recursive != state.includeSubfolders) return;
        state = state.copyWith(results: items);
      },
      playlists: (items) {
        final selected = state.selectedPlaylistId;
        final selectedExists =
            selected == null || items.any((p) => p.id == selected);
        state = state.copyWith(
          playlists: items,
          selectedPlaylistId: selectedExists ? selected : null,
        );
      },
      playlistTracks: (playlistId, query, items) {
        if (state.selectedPlaylistId != playlistId) return;
        if (query != state.query) return;
        state = state.copyWith(results: items);
      },
      likedTrackIds: (trackIds) {
        state = state.copyWith(
          likedTrackIds: trackIds.map((v) => v.toInt()).toSet(),
        );
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
        unawaited(ref.read(libraryBridgeProvider).listFolders());
        unawaited(_refreshTracks());
      },
      error: (message) {
        ref.read(loggerProvider).e(message);
        state = state.copyWith(lastError: message, isScanning: false);
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
