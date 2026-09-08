import 'package:stellatune/bridge/api/error.dart';

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/platform/directory_access_noop.dart';
import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/platform/directory_access_store.dart';

void main() {
  late _Harness h;
  setUp(() async {
    h = _Harness();
    await h.controller.refresh();
  });
  tearDown(() async {
    h.container.dispose();
    await h.bridge.eventsController.close();
  });

  for (final force in [false, true]) {
    test(
      '${force ? 'force' : 'normal'} scan owns access beyond command admission',
      () async {
        await h.controller.scanAll(force: force);
        expect(h.bridge.scans, 1);
        expect(h.bridge.lastForce, force);
        expect(h.access.leases.single.releases, 0);
        expect(h.scanning, isTrue);
        await h.controller.scanAll();
        expect(
          h.bridge.scans,
          1,
          reason: 'coalesce while background scan is active',
        );
        h.bridge.eventsController.add(
          LibraryEvent.scanProgress(
            scanned: 1,
            updated: 1,
            skipped: 0,
            errors: 0,
          ),
        );
        expect(h.access.leases.single.releases, 0);
        h.finish();
        await _flush();
        expect(h.scanning, isFalse);
        expect(h.access.leases.single.releases, 1);
        h.finish();
        await _flush();
        expect(h.access.leases.single.releases, 1);
      },
    );
  }

  test('scan failure releases its lease and permits another scan', () async {
    await h.controller.scanAll();
    h.bridge.eventsController.add(
      const LibraryEvent.error(
        error: AppError(
          category: ErrorCategory.internal,
          operation: 'library_scan',
          diagnosticId: 'test',
          fingerprint: 'scan-failed',
          context: '',
        ),
      ),
    );
    await _flush();
    expect(h.access.leases.single.releases, 1);
    expect(h.scanning, isFalse);
    expect(h.container.read(libraryControllerProvider).lastError, '音乐库操作失败');
    await h.controller.scanAll();
    expect(h.bridge.scans, 2);
    expect(h.access.leases.last.releases, 0);
    expect(h.container.read(libraryControllerProvider).lastError, isNull);
  });

  test('command admission failure releases access immediately', () async {
    h.bridge.admissionError = StateError('channel closed');
    await h.controller.scanAll();
    expect(h.scanning, isFalse);
    expect(h.access.leases.single.releases, 1);
    expect(
      h.container.read(libraryControllerProvider).lastError,
      equals('音乐库操作失败'),
    );
  });

  test('event stream error releases the accepted scan', () async {
    await h.controller.scanAll();
    h.bridge.eventsController.addError(StateError('events failed'));
    await _flush();
    expect(h.access.leases.single.releases, 1);
    expect(h.scanning, isFalse);
    expect(
      h.container.read(libraryControllerProvider).lastError,
      equals('音乐库操作失败'),
    );
  });

  test('event stream closure releases the accepted scan', () async {
    await h.controller.scanAll();
    await h.bridge.eventsController.close();
    await _flush();
    expect(h.access.leases.single.releases, 1);
    expect(h.scanning, isFalse);
  });

  test('authorization failure does not acquire or send a scan', () async {
    h.access.authorizationError = StateError('not authorized');
    await h.controller.scanAll();
    expect(h.access.leases, isEmpty);
    expect(h.bridge.scans, 0);
    expect(h.scanning, isFalse);
    expect(
      h.container.read(libraryControllerProvider).lastError,
      equals('音乐库操作失败'),
    );
  });

  test('completion before admission ACK still releases exactly once', () async {
    h.bridge.admission = Completer<void>();
    final pending = h.controller.scanAll();
    await _flush();
    expect(h.bridge.scans, 1);
    h.finish();
    await _flush();
    expect(h.access.leases.single.releases, 1);
    h.bridge.admission!.complete();
    await pending;
    expect(h.access.leases.single.releases, 1);
    expect(h.scanning, isFalse);
  });

  test('disposal releases an accepted scan', () async {
    await h.controller.scanAll();
    h.container.dispose();
    await _flush();
    expect(h.access.leases.single.releases, 1);
  });

  test('disposal consumes a late acquisition without sending a scan', () async {
    h.access.acquisition = Completer<void>();
    final pending = h.controller.scanAll();
    await _flush();
    h.container.dispose();
    h.access.acquisition!.complete();
    await pending;
    expect(h.access.leases.single.releases, 1);
    expect(h.bridge.scans, 0);
  });

  test(
    'rebuild discards the old acquisition before it can send a scan',
    () async {
      h.access.acquisition = Completer<void>();
      final old = h.controller.scanAll();
      await _flush();
      final gate = h.access.acquisition!;
      final oldLease = h.access.leases.single;
      h.container.invalidate(libraryControllerProvider);
      h.controller = h.container.read(libraryControllerProvider.notifier);
      await h.controller.refresh();
      h.access.acquisition = null;
      await h.controller.scanAll();
      final newLease = h.access.leases.last;
      gate.complete();
      await old;
      expect(h.bridge.scans, 1);
      expect(oldLease.releases, 1);
      expect(newLease.releases, 0);
      expect(h.scanning, isTrue);
      h.finish();
      await _flush();
      expect(newLease.releases, 1);
    },
  );

  test('old roots responses never synchronize persistent bookmarks', () async {
    h.bridge.roots = Completer<List<String>>();
    final oldGate = h.bridge.roots!;
    final first = h.controller.refresh();
    await _flush();
    h.bridge.roots = Completer<List<String>>();
    final second = h.controller.refresh();
    await _flush();
    h.bridge.roots!.complete(['/new']);
    await second;
    oldGate.complete(['/old']);
    await first;
    expect(h.container.read(libraryControllerProvider).roots, ['/new']);
    expect(h.access.syncCalls, 0);
  });

  test('an old roots response cannot overwrite a newly added root', () async {
    h.bridge.roots = Completer<List<String>>();
    final pending = h.controller.refresh();
    await _flush();
    await h.controller.addRoot('/new', scanAfter: false);
    h.bridge.roots!.complete(['/music']);
    await pending;
    expect(h.container.read(libraryControllerProvider).roots, [
      '/music',
      '/new',
    ]);
  });

  test('an old roots response cannot restore a removed root', () async {
    h.bridge.roots = Completer<List<String>>();
    final pending = h.controller.refresh();
    await _flush();
    await h.controller.removeRoot('/music');
    h.bridge.roots!.complete(['/music']);
    await pending;
    expect(h.container.read(libraryControllerProvider).roots, isEmpty);
  });
}

Future<void> _flush() => Future<void>.delayed(Duration.zero);

class _Settings extends Fake implements SettingsStore {}

class _Lease implements DirectoryAccessLease {
  int releases = 0;
  @override
  Future<void> release() async {
    releases++;
  }
}

class _Access extends NoopDirectoryAccessService {
  final leases = <_Lease>[];
  Completer<void>? acquisition;
  Object? authorizationError;
  int syncCalls = 0;

  @override
  Future<void> ensureRootsAuthorized({
    required Iterable<String> roots,
    required DirectoryAccessStore store,
  }) async {
    if (authorizationError != null) throw authorizationError!;
  }

  @override
  Future<DirectoryAccessLease?> acquireRoots({
    required Iterable<String> roots,
    required DirectoryAccessStore store,
  }) async {
    final lease = _Lease();
    leases.add(lease);
    await acquisition?.future;
    return lease;
  }

  @override
  Future<void> syncStoredDirectories({
    required Iterable<String> paths,
    required DirectoryAccessStore store,
  }) async {
    syncCalls++;
  }
}

class _Bridge extends Fake implements LibraryBridge {
  final eventsController = StreamController<LibraryEvent>.broadcast(sync: true);
  int scans = 0;
  bool? lastForce;
  Object? admissionError;
  Completer<void>? admission;
  Completer<List<String>>? roots;

  @override
  Stream<LibraryEvent> events() => eventsController.stream;
  @override
  Future<List<String>> listRoots() async =>
      roots == null ? ['/music'] : roots!.future;
  @override
  Future<List<String>> listFolders() async => [];
  @override
  Future<List<String>> listExcludedFolders() async => [];
  @override
  Future<void> addRoot(String path) async {}
  @override
  Future<void> removeRoot(String path) async {}
  @override
  Future<List<PlaylistLite>> listPlaylists() async => [];
  @override
  Future<List<int>> listLikedTrackIds() async => [];
  @override
  Future<List<TrackLite>> listTracks({
    required String folder,
    required bool recursive,
    required String query,
    int limit = 5000,
    int offset = 0,
  }) async => [];
  @override
  Future<void> scanAll() => _scan(false);
  @override
  Future<void> scanAllForce() => _scan(true);
  Future<void> _scan(bool force) async {
    scans++;
    lastForce = force;
    if (admissionError != null) throw admissionError!;
    await admission?.future;
  }
}

class _Harness {
  _Harness() {
    container = ProviderContainer(
      overrides: [
        libraryBridgeProvider.overrideWithValue(bridge),
        settingsStoreServiceProvider.overrideWithValue(_Settings()),
        libraryDirectoryAccessProvider.overrideWithValue(access),
      ],
    );
    controller = container.read(libraryControllerProvider.notifier);
  }
  final bridge = _Bridge();
  final access = _Access();
  late final ProviderContainer container;
  late LibraryController controller;
  bool get scanning => container.read(libraryControllerProvider).isScanning;
  void finish() => bridge.eventsController.add(
    LibraryEvent.scanFinished(
      durationMs: 10,
      scanned: 1,
      updated: 1,
      skipped: 0,
      errors: 0,
    ),
  );
}
