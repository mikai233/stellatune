import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/platform/directory_access_store.dart';
import 'package:stellatune/platform/macos_directory_access.dart';
import 'package:stellatune/platform/queue_path_leases.dart';

class _Bookmarks implements DirectoryAccessStore {
  _Bookmarks([Map<String, String> initial = const {}])
    : bookmarks = {...initial};
  final Map<String, String> bookmarks;
  String? failWrite;
  String? failRemove;
  @override
  Map<String, String> get macosDirectoryBookmarks => {...bookmarks};
  @override
  String? macosDirectoryBookmarkForPath(String path) => bookmarks[path];
  @override
  Future<void> setMacosDirectoryBookmark({
    required String path,
    required String bookmark,
  }) async {
    if (path == failWrite) throw StateError('bookmark save failed');
    bookmarks[path] = bookmark;
  }

  @override
  Future<void> removeMacosDirectoryBookmark(String path) async {
    if (path == failRemove) throw StateError('bookmark remove failed');
    bookmarks.remove(path);
  }
}

class _Lease implements DirectoryAccessLease {
  int releases = 0;
  @override
  Future<void> release() async {
    releases++;
  }
}

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('stellatune/macos_directory_access');
  const service = MacosDirectoryAccessService();
  late _Bookmarks store;
  late List<String> starts;
  late List<String> stops;
  String? failStart;
  String? movedPath;
  Completer<void>? startGate;
  Completer<void>? stopGate;

  setUp(() {
    store = _Bookmarks({'/A': 'a', '/B': 'b'});
    starts = [];
    stops = [];
    failStart = null;
    movedPath = null;
    startGate = null;
    stopGate = null;
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, (
      call,
    ) async {
      final args = call.arguments as Map;
      if (call.method == 'startAccessingDirectory') {
        final bookmark = args['bookmark'] as String;
        if (bookmark == failStart) {
          throw PlatformException(code: 'access_denied');
        }
        final path = movedPath ?? (bookmark == 'a' ? '/A' : '/B');
        starts.add(path);
        await startGate?.future;
        return {'path': path, 'bookmark': bookmark};
      }
      if (call.method == 'stopAccessingDirectory') {
        await stopGate?.future;
        stops.add(args['path'] as String);
        return null;
      }
      throw StateError('unexpected native method ${call.method}');
    });
  });

  tearDown(() {
    binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, null);
  });

  test('second native acquisition failure rolls back the first root', () async {
    failStart = 'b';
    await expectLater(
      service.acquireRoots(roots: ['/A', '/B'], store: store),
      throwsA(isA<PlatformException>()),
    );
    expect(starts, ['/A']);
    expect(stops, ['/A']);
  });

  test('missing second bookmark rolls back the first root', () async {
    await expectLater(
      service.acquireRoots(roots: ['/A', '/missing'], store: store),
      throwsA(isA<DirectoryAccessException>()),
    );
    expect(starts, ['/A']);
    expect(stops, ['/A']);
  });

  test(
    'saving the second acquired root fails and releases both in reverse order',
    () async {
      store.failWrite = '/B';
      await expectLater(
        service.acquireRoots(roots: ['/A', '/B'], store: store),
        throwsStateError,
      );
      expect(starts, ['/A', '/B']);
      expect(stops, ['/B', '/A']);
    },
  );

  test('local path save failure releases the actual moved directory', () async {
    movedPath = '/Moved';
    store.failWrite = '/Moved';
    await expectLater(
      service.acquireLocalPath(path: '/A/song.flac', store: store),
      throwsStateError,
    );
    expect(starts, ['/Moved']);
    expect(stops, ['/Moved']);
  });

  test(
    'bookmark removal failure after refreshing a moved root releases access',
    () async {
      movedPath = '/Moved';
      store.failRemove = '/A';
      await expectLater(
        service.acquireLocalPath(path: '/A/song.flac', store: store),
        throwsStateError,
      );
      expect(stops, starts);
    },
  );

  test('release is idempotent and all callers await the native stop', () async {
    final lease = await service.acquireRoots(
      roots: ['/A/', '/A'],
      store: store,
    );
    stopGate = Completer<void>();
    final first = lease!.release();
    final second = lease.release();
    expect(identical(first, second), isTrue);
    expect(starts, ['/A']);
    expect(stops, isEmpty);
    stopGate!.complete();
    await Future.wait([first, second]);
    expect(stops, ['/A']);
  });

  test(
    'forgetting a bookmark does not release another consumer\'s lease',
    () async {
      final lease = await service.acquireRoots(roots: ['/A'], store: store);
      await service.forgetDirectory(path: '/A', store: store);
      expect(store.macosDirectoryBookmarkForPath('/A'), isNull);
      expect(stops, isEmpty);
      await lease!.release();
      expect(stops, starts);
    },
  );

  test(
    'real bridge coalesces concurrent queue access and releases once',
    () async {
      final bridge = await PlayerBridge.create(directoryAccessService: service);
      bridge.bindDirectoryAccessStore(store);
      startGate = Completer<void>();
      final first = bridge.retainQueuePaths(['/A/song.flac']);
      final second = bridge.retainQueuePaths(['/A/song.flac']);
      await Future<void>.delayed(Duration.zero);
      expect(starts, ['/A']);
      startGate!.complete();
      await Future.wait([first, second]);
      await bridge.releaseRemovedQueuePaths(const []);
      await bridge.dispose();
      expect(stops, ['/A']);
    },
  );

  test(
    'queue disposal owns and releases acquisitions that finish late',
    () async {
      final bridge = await PlayerBridge.create(directoryAccessService: service);
      bridge.bindDirectoryAccessStore(store);
      startGate = Completer<void>();
      final retaining = bridge.retainQueuePaths(['/A/song.flac']);
      final disposal = bridge.dispose();
      var finished = false;
      unawaited(disposal.then((_) => finished = true));
      await Future<void>.delayed(Duration.zero);
      expect(finished, isFalse);
      startGate!.complete();
      await Future.wait([retaining, disposal]);
      expect(starts, ['/A']);
      expect(stops, ['/A']);
      await expectLater(
        bridge.retainQueuePaths(['/A/song.flac']),
        throwsStateError,
      );
    },
  );

  test('disposal awaits an entry already removed by a queue refresh', () async {
    final bridge = await PlayerBridge.create(directoryAccessService: service);
    bridge.bindDirectoryAccessStore(store);
    await bridge.retainQueuePaths(['/A/song.flac']);
    stopGate = Completer<void>();
    final removing = bridge.releaseRemovedQueuePaths(const []);
    final disposing = bridge.dispose();
    var disposed = false;
    unawaited(disposing.then((_) => disposed = true));
    await Future<void>.delayed(Duration.zero);
    expect(disposed, isFalse);
    stopGate!.complete();
    await Future.wait([removing, disposing]);
    expect(stops, ['/A']);
  });

  test('failed queue batch releases its new paths but preserves a concurrent claim', () async {
    final successful = _Lease();
    final alone = _Lease();
    final failure = Completer<DirectoryAccessLease?>();
    final owner = QueuePathLeases(
      (path) async => switch (path) {
        'shared' => successful,
        'alone' => alone,
        _ => failure.future,
      },
    );
    final bad = owner.retain(['shared', 'alone', 'bad']);
    final checkError = expectLater(bad, throwsStateError);
    await owner.retain(['shared']);
    failure.completeError(StateError('not authorized'));
    await checkError;
    expect(alone.releases, 1);
    expect(successful.releases, 0);
    await owner.dispose();
    expect(successful.releases, 1);
    expect(alone.releases, 1);
  });
}
