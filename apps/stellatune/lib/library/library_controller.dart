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

    // Hydrate persisted roots on startup.
    unawaited(bridge.listRoots());

    return const LibraryState.initial();
  }

  Future<void> addRoot(String path, {bool scanAfter = true}) async {
    if (path.trim().isEmpty) return;
    if (state.roots.contains(path)) return;

    state = state.copyWith(
      roots: [...state.roots, path],
      lastError: null,
      lastLog: '',
    );

    await ref.read(libraryBridgeProvider).addRoot(path);
    if (scanAfter) await scanAll();
  }

  Future<void> removeRoot(String path) async {
    state = state.copyWith(
      roots: state.roots.where((r) => r != path).toList(),
      lastError: null,
      lastLog: '',
    );
    await ref.read(libraryBridgeProvider).removeRoot(path);
  }

  Future<void> scanAll() async {
    state = state.copyWith(
      isScanning: true,
      progress: const LibraryScanProgress.zero(),
      lastFinishedMs: null,
      lastError: null,
      lastLog: '',
    );
    await ref.read(libraryBridgeProvider).scanAll();
  }

  void setQuery(String query) {
    final q = query.trim();
    state = state.copyWith(query: q, lastError: null);

    _debounce?.cancel();
    if (q.isEmpty) {
      unawaited(ref.read(libraryBridgeProvider).search(''));
      return;
    }

    _debounce = Timer(const Duration(milliseconds: 250), () {
      unawaited(ref.read(libraryBridgeProvider).search(q));
    });
  }

  void _onEvent(LibraryEvent event) {
    event.when(
      roots: (paths) {
        state = state.copyWith(roots: paths, lastError: null);
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
        // Refresh visible results after scanning (empty query lists recent tracks).
        unawaited(ref.read(libraryBridgeProvider).search(state.query));
      },
      searchResult: (query, items) {
        if (state.query != query) return;
        state = state.copyWith(results: items);
      },
      error: (message) {
        ref.read(loggerProvider).e(message);
        state = state.copyWith(lastError: message, isScanning: false);
      },
      log: (message) {
        ref.read(loggerProvider).d(message);
        state = state.copyWith(lastLog: message);
      },
    );
  }
}
