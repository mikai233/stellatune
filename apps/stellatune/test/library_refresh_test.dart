import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/library/library_state.dart';
import 'package:stellatune/platform/directory_access_noop.dart';

class _Settings extends Fake implements SettingsStore {}

class _TrackRequest {
  _TrackRequest(this.folder, this.query);
  final String folder;
  final String query;
  final response = Completer<List<TrackLite>>();
}

class _Library extends Fake implements LibraryBridge {
  final eventsController = StreamController<LibraryEvent>.broadcast(sync: true);
  final tracks = <_TrackRequest>[];
  bool controlTracks = false;
  String? rootsError;
  String? foldersError;
  Completer<List<String>>? rootsGate;
  Completer<void>? scanGate;

  @override
  Stream<LibraryEvent> events() => eventsController.stream;
  @override
  Future<List<String>> listRoots() async {
    if (rootsError != null) throw StateError(rootsError!);
    if (rootsGate != null) return rootsGate!.future;
    return ['/music'];
  }

  @override
  Future<List<String>> listFolders() async {
    if (foldersError != null) throw StateError(foldersError!);
    return ['/music/album'];
  }

  @override
  Future<List<String>> listExcludedFolders() async => [];
  @override
  Future<List<PlaylistLite>> listPlaylists() async => [];
  @override
  Future<List<int>> listLikedTrackIds() async => [7];
  @override
  Future<List<TrackLite>> listTracks({
    required String folder,
    required bool recursive,
    required String query,
    int limit = 5000,
    int offset = 0,
  }) {
    if (!controlTracks) return Future.value([]);
    final request = _TrackRequest(folder, query);
    tracks.add(request);
    return request.response.future;
  }

  @override
  Future<void> scanAll() async {
    await scanGate?.future;
  }
}

Future<void> _until(bool Function() ready) async {
  for (var attempt = 0; attempt < 100; attempt++) {
    if (ready()) return;
    await Future<void>.delayed(const Duration(milliseconds: 1));
  }
  fail('async request did not arrive');
}

List<TrackLite> _song(int id) => [
  TrackLite(id: id, path: '$id.flac', title: 'Song $id'),
];

void main() {
  late _Library bridge;
  late ProviderContainer container;
  late LibraryController controller;
  setUp(() async {
    bridge = _Library();
    container = ProviderContainer(
      overrides: [
        libraryBridgeProvider.overrideWithValue(bridge),
        settingsStoreServiceProvider.overrideWithValue(_Settings()),
        libraryDirectoryAccessProvider.overrideWithValue(
          const NoopDirectoryAccessService(),
        ),
      ],
    );
    controller = container.read(libraryControllerProvider.notifier);
    await controller.refresh();
  });
  tearDown(() async {
    container.dispose();
    for (final request in bridge.tracks) {
      if (!request.response.isCompleted) request.response.complete([]);
    }
    await bridge.eventsController.close();
  });

  test(
    'nullable state fields distinguish unchanged and explicitly cleared',
    () {
      final previous = const LibraryState.initial().copyWith(
        lastError: 'failed',
        lastFinishedMs: 42,
      );
      expect(previous.copyWith().lastError, 'failed');
      expect(previous.copyWith().lastFinishedMs, 42);
      final cleared = previous.copyWith(lastError: null, lastFinishedMs: null);
      expect(cleared.lastError, isNull);
      expect(cleared.lastFinishedMs, isNull);
    },
  );

  test(
    'initial hydration reports errors without aborting unrelated queries',
    () async {
      bridge.rootsError = 'initial roots failed';
      container.invalidate(libraryControllerProvider);
      controller = container.read(libraryControllerProvider.notifier);
      await controller.refresh();
      final hydrated = container.read(libraryControllerProvider);
      expect(hydrated.lastError, contains('initial roots failed'));
      expect(hydrated.likedTrackIds, {7});
      expect(hydrated.folders, ['/music/album']);
    },
  );

  test(
    'changing tracks does not invalidate the concurrent roots query',
    () async {
      bridge.rootsGate = Completer<List<String>>();
      final refreshing = controller.refresh();
      await Future<void>.delayed(Duration.zero);
      controller.selectFolder('/other');
      bridge.rootsGate!.complete(['/latest']);
      await refreshing;
      final refreshed = container.read(libraryControllerProvider);
      expect(refreshed.roots, ['/latest']);
      expect(refreshed.selectedFolder, '/other');
    },
  );

  test('event refresh failure stays visible while unrelated queries succeed; recovery clears it', () async {
    bridge.rootsError = 'roots offline';
    bridge.eventsController.add(const LibraryEvent.changed());
    await controller.refresh();
    final failed = container.read(libraryControllerProvider);
    expect(failed.lastError, contains('roots offline'));
    expect(failed.folders, ['/music/album']);
    expect(failed.likedTrackIds, {7});
    bridge.rootsError = null;
    await controller.refresh();
    expect(container.read(libraryControllerProvider).lastError, isNull);
  });

  test('a newer result for identical filters wins even when the older query completes last', () async {
    bridge.controlTracks = true;
    final first = controller.refresh();
    await _until(() => bridge.tracks.length == 1);
    final second = controller.refresh();
    await _until(() => bridge.tracks.length == 2);
    bridge.tracks[1].response.complete(_song(2));
    await second;
    bridge.tracks[0].response.complete(_song(1));
    await first;
    expect(container.read(libraryControllerProvider).results.single.id, 2);
  });

  test(
    'stale query failure cannot replace a later success with an error',
    () async {
      bridge.controlTracks = true;
      final first = controller.refresh();
      await _until(() => bridge.tracks.length == 1);
      final second = controller.refresh();
      await _until(() => bridge.tracks.length == 2);
      bridge.tracks[1].response.complete(_song(2));
      await second;
      bridge.tracks[0].response.completeError(StateError('old query failed'));
      await first;
      expect(container.read(libraryControllerProvider).lastError, isNull);
      expect(container.read(libraryControllerProvider).results.single.id, 2);
    },
  );

  test('a query error is cleared after the same query succeeds', () async {
    bridge.controlTracks = true;
    final first = controller.refresh();
    await _until(() => bridge.tracks.length == 1);
    bridge.tracks[0].response.completeError(StateError('tracks offline'));
    await first;
    expect(
      container.read(libraryControllerProvider).lastError,
      contains('tracks offline'),
    );
    final second = controller.refresh();
    await _until(() => bridge.tracks.length == 2);
    bridge.tracks[1].response.complete(_song(2));
    await second;
    expect(container.read(libraryControllerProvider).lastError, isNull);
  });

  test('same-turn changed events merge into one refresh', () async {
    bridge.controlTracks = true;
    for (var i = 0; i < 3; i++) {
      bridge.eventsController.add(const LibraryEvent.changed());
    }
    final refreshed = controller.refresh();
    await _until(() => bridge.tracks.length == 1);
    bridge.tracks.single.response.complete([]);
    await refreshed;
    expect(bridge.tracks, hasLength(1));
  });

  test('changing a filter invalidates the previous result before debounce finishes', () async {
    bridge.controlTracks = true;
    final old = controller.refresh();
    await _until(() => bridge.tracks.length == 1);
    controller.setQuery('B');
    controller.setQuery('');
    bridge.tracks.single.response.complete(_song(1));
    await old;
    expect(container.read(libraryControllerProvider).results, isEmpty);
  });

  test(
    'late query errors after disposal are consumed without touching state',
    () async {
      bridge.controlTracks = true;
      final pending = controller.refresh();
      await _until(() => bridge.tracks.length == 1);
      container.dispose();
      bridge.tracks.single.response.completeError(StateError('after disposal'));
      await pending;
    },
  );

  test(
    'starting a scan clears the previous finished time and scan error',
    () async {
      bridge.eventsController.add(
        LibraryEvent.scanFinished(
          durationMs: 50,
          scanned: 1,
          updated: 1,
          skipped: 0,
          errors: 0,
        ),
      );
      bridge.eventsController.add(
        const LibraryEvent.error(message: 'previous scan failed'),
      );
      expect(container.read(libraryControllerProvider).lastFinishedMs, 50);
      bridge.scanGate = Completer<void>();
      final scan = controller.scanAll();
      expect(container.read(libraryControllerProvider).lastFinishedMs, isNull);
      expect(container.read(libraryControllerProvider).lastError, isNull);
      bridge.scanGate!.complete();
      await scan;
    },
  );
}
