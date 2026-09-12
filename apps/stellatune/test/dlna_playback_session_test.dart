import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/player/dlna_playback_session.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

import 'playback_navigation_test.dart' show ControlledBridge, until;

DlnaRenderer _renderer(String id) => DlnaRenderer(
  usn: id,
  location: 'http://$id',
  friendlyName: id,
  avTransportControlUrl: 'http://$id/transport',
  renderingControlUrl: 'http://$id/volume',
);

class _Dlna extends DlnaBridge {
  Completer<DlnaTransportInfo>? infoGate;
  Completer<DlnaPositionInfo>? positionGate;
  Completer<void>? seekGate;
  Completer<String>? playGate;
  Completer<int>? volumeGate;
  int infoCalls = 0;
  int positionCalls = 0;
  int seekCalls = 0;
  int playCalls = 0;
  int volumeCalls = 0;
  final seekUrls = <String>[];
  final playedPaths = <String>[];
  final playedIds = <int>[];
  @override
  Future<String> playLocalTrack({
    required DlnaRenderer renderer,
    required int libraryTrackId,
  }) async {
    playedIds.add(libraryTrackId);
    return playLocalPath(renderer: renderer, path: 'track:');
  }

  final transportActions = <String>[];
  final stopped = <String>[];
  int unpublishes = 0;
  @override
  Future<DlnaTransportInfo> avTransportGetTransportInfo({
    required String controlUrl,
    String? serviceType,
  }) async {
    infoCalls++;
    return await infoGate?.future ??
        const DlnaTransportInfo(currentTransportState: 'PLAYING');
  }

  @override
  Future<DlnaPositionInfo> avTransportGetPositionInfo({
    required String controlUrl,
    String? serviceType,
  }) async {
    positionCalls++;
    return await positionGate?.future ??
        DlnaPositionInfo(relTimeMs: BigInt.from(8000));
  }

  @override
  Future<void> avTransportSeekMs({
    required String controlUrl,
    required int positionMs,
    String? serviceType,
  }) async {
    seekCalls++;
    seekUrls.add(controlUrl);
    transportActions.add('seek:$positionMs');
    await seekGate?.future;
  }

  @override
  Future<void> avTransportStop({
    required String controlUrl,
    String? serviceType,
  }) async {
    stopped.add(controlUrl);
  }

  @override
  Future<void> avTransportPlay({
    required String controlUrl,
    String? serviceType,
  }) async {}
  @override
  Future<void> avTransportPause({
    required String controlUrl,
    String? serviceType,
  }) async {}
  @override
  Future<void> httpUnpublishAll() async {
    unpublishes++;
  }

  @override
  Future<String> playLocalPath({
    required DlnaRenderer renderer,
    required String path,
  }) async {
    playCalls++;
    playedPaths.add(path);
    transportActions.add('play:$path');
    return await playGate?.future ?? 'http://localhost/media';
  }

  @override
  Future<void> renderingControlSetMute({
    required String controlUrl,
    required bool mute,
    String? serviceType,
  }) async {}
  @override
  Future<void> renderingControlSetVolume({
    required String controlUrl,
    required int volume0To100,
    String? serviceType,
  }) async {}
  @override
  Future<int> renderingControlGetVolume({
    required String controlUrl,
    String? serviceType,
  }) async {
    volumeCalls++;
    return await volumeGate?.future ?? 50;
  }
}

class _Lease implements DirectoryAccessLease {
  int releases = 0;
  @override
  Future<void> release() async {
    releases++;
  }
}

class _Local extends ControlledBridge {
  Completer<PlaybackQueue>? queueGate;
  int queueCalls = 0;
  @override
  Future<PlaybackQueue> playbackQueue() async {
    queueCalls++;
    return await queueGate?.future ?? queue;
  }
}

void main() {
  group('renderer session', () {
    late _Dlna bridge;
    late DlnaPlaybackSession session;
    late List<DlnaPlaybackUpdate> updates;
    late List<String> errors;
    late _Lease lease;
    setUp(() {
      bridge = _Dlna();
      updates = [];
      errors = [];
      lease = _Lease();
      session = DlnaPlaybackSession(
        renderer: _renderer('A'),
        bridge: bridge,
        ready: Future.value(),
        acquirePath: (_) async => lease,
        coverDirectory: '',
        onUpdate: updates.add,
        onError: errors.add,
        pollInterval: const Duration(hours: 1),
      );
    });
    tearDown(() => session.close());

    test(
      'CUE songs in the same file are published by distinct track IDs',
      () async {
        await session.playItem(
          QueueItem(
            trackId: BigInt.from(101),
            id: 1,
            path: 'album.wav',
            isSegment: true,
            durationMs: 1000,
          ),
        );
        await session.playItem(
          QueueItem(
            trackId: BigInt.from(102),
            id: 2,
            path: 'album.wav',
            isSegment: true,
            durationMs: 1000,
          ),
        );
        await session.poll();
        expect(bridge.playedIds, [1, 2]);
        expect(updates.last.trackKey, '102');
      },
    );

    test('invalidated transport response does not request another resource or update', () async {
      bridge.infoGate = Completer<DlnaTransportInfo>();
      final poll = session.poll();
      await until(() => bridge.infoCalls == 1);
      session.invalidate();
      bridge.infoGate!.complete(
        const DlnaTransportInfo(currentTransportState: 'STOPPED'),
      );
      await poll;
      expect(bridge.positionCalls, 0);
      expect(updates, isEmpty);
    });

    test(
      'position response predating a seek cannot rewind its new position',
      () async {
        bridge.positionGate = Completer<DlnaPositionInfo>();
        final poll = session.poll();
        await until(() => bridge.positionCalls == 1);
        expect(await session.seek(1234), isTrue);
        bridge.positionGate!.complete(
          DlnaPositionInfo(relTimeMs: BigInt.from(99)),
        );
        await poll;
        expect(updates, isEmpty);
      },
    );

    test('polling waits while a transport command is being applied', () async {
      bridge.seekGate = Completer<void>();
      final seek = session.seek(1234);
      await until(() => bridge.seekCalls == 1);
      await session.poll();
      expect(bridge.infoCalls, 0);
      bridge.seekGate!.complete();
      await seek;
      await session.poll();
      expect(bridge.infoCalls, 1);
    });

    test(
      'close drains an in-flight media publication before releasing its lease',
      () async {
        bridge.playGate = Completer<String>();
        final play = session.playItem(
          const QueueItem(trackId: null, path: 'a.mp3'),
        );
        await until(() => bridge.playCalls == 1);
        final close = session.close();
        expect(bridge.stopped, isEmpty);
        bridge.playGate!.complete('http://localhost/a');
        expect(await play, isFalse);
        await close;
        expect(lease.releases, 1);
        expect(bridge.stopped, ['http://A/transport']);
        expect(bridge.unpublishes, 1);
        expect(session.path, isNull);
      },
    );

    test(
      'successful playback retains its lease until stop and releases once',
      () async {
        expect(
          await session.playItem(const QueueItem(trackId: null, path: 'a.mp3')),
          isTrue,
        );
        expect(lease.releases, 0);
        expect(await session.pause(), isTrue);
        expect(lease.releases, 0);
        expect(await session.stop(), isTrue);
        expect(lease.releases, 1);
        await session.close();
        expect(lease.releases, 1);
      },
    );

    test('late volume failure belongs only to the old renderer', () async {
      bridge.volumeGate = Completer<int>();
      final volume = session.setVolume(.7);
      await until(() => bridge.volumeCalls == 1);
      session.invalidate();
      bridge.volumeGate!.completeError(StateError('old volume response'));
      await volume;
      expect(errors, isEmpty);
    });

    test(
      'queued seek preserves media publication and its file lease',
      () async {
        bridge.playGate = Completer<String>();
        final playing = session.playItem(
          const QueueItem(trackId: null, path: 'a.mp3'),
        );
        await until(() => bridge.playCalls == 1);
        final seeking = session.seek(8000);
        bridge.playGate!.complete('http://localhost/a');
        expect(await playing, isTrue);
        expect(await seeking, isTrue);
        expect(session.path, 'a.mp3');
        expect(lease.releases, 0);
      },
    );

    test('seek does not discard a queued change of media', () async {
      await session.close();
      final leases = <_Lease>[];
      session = DlnaPlaybackSession(
        renderer: _renderer('A'),
        bridge: bridge,
        ready: Future.value(),
        acquirePath: (_) async {
          final lease = _Lease();
          leases.add(lease);
          return lease;
        },
        coverDirectory: '',
        onUpdate: updates.add,
        onError: errors.add,
        pollInterval: const Duration(hours: 1),
      );
      bridge.playGate = Completer<String>();
      final first = session.playItem(
        const QueueItem(trackId: null, path: 'a.mp3'),
      );
      await until(() => bridge.playCalls == 1);
      final second = session.playItem(
        const QueueItem(trackId: null, path: 'b.mp3'),
      );
      final seeking = session.seek(8000);
      bridge.playGate!.complete('http://localhost/media');
      await Future.wait([first, second, seeking]);
      expect(bridge.playedPaths, ['a.mp3', 'b.mp3']);
      expect(bridge.transportActions, [
        'play:a.mp3',
        'play:b.mp3',
        'seek:8000',
      ]);
      expect(session.path, 'b.mp3');
      expect(bridge.seekCalls, 1);
      expect(leases.first.releases, 1);
      expect(leases.last.releases, 0);
      await session.close();
      expect(leases.last.releases, 1);
    });
  });

  group('playback controller output switch', () {
    late Directory directory;
    late _Local local;
    late _Dlna dlna;
    late ProviderContainer container;
    late PlaybackController controller;
    setUp(() async {
      directory = await Directory.systemTemp.createTemp(
        'stellatune-dlna-session-',
      );
      Hive.init(directory.path);
      await Hive.openBox('settings');
      local = _Local();
      dlna = _Dlna();
      container = ProviderContainer(
        overrides: [
          settingsStoreServiceProvider.overrideWithValue(SettingsStore()),
          playerBridgeProvider.overrideWithValue(local),
          dlnaBridgeProvider.overrideWithValue(dlna),
          coverDirProvider.overrideWithValue(directory.path),
        ],
      );
      controller = container.read(playbackControllerProvider.notifier);
      await until(
        () => container.read(queueControllerProvider).items.isNotEmpty,
      );
    });
    tearDown(() async {
      container.read(dlnaSelectedRendererProvider.notifier).set(null);
      await Future<void>.delayed(Duration.zero);
      container.dispose();
      await local.eventStream.close();
      await local.queueStream.close();
      await Hive.close();
      await directory.delete(recursive: true);
    });

    test(
      'deferred renderer initialization cannot override a new selection',
      () async {
        container
            .read(dlnaSelectedRendererProvider.notifier)
            .set(_renderer('A'));
        container.invalidate(playbackControllerProvider);
        controller = container.read(playbackControllerProvider.notifier);
        container
            .read(dlnaSelectedRendererProvider.notifier)
            .set(_renderer('B'));
        await controller.seekMs(10);
        expect(dlna.seekUrls, ['http://B/transport']);
        expect(container.read(dlnaSelectedRendererProvider)?.usn, 'B');
      },
    );

    for (final pause in [true, false]) {
      test(
        '${pause ? 'pause' : 'seek'} during publication preserves loaded media',
        () async {
          container
              .read(dlnaSelectedRendererProvider.notifier)
              .set(_renderer('A'));
          await until(() => local.stopCalls == 1);
          dlna.playGate = Completer<String>();
          final playing = controller.setQueueAndPlayItems([
            const QueueItem(trackId: null, path: 'a.mp3'),
          ]);
          await until(() => dlna.playCalls == 1);
          final command = pause ? controller.pause() : controller.seekMs(1234);
          dlna.playGate!.complete('http://localhost/a');
          await Future.wait([playing, command]);
          final state = container.read(playbackControllerProvider);
          expect(state.currentPath, 'a.mp3');
          expect(state.pendingItem, isNull);
          expect(state.lastError, isNull);
          expect(
            state.playerState,
            pause ? PlayerState.paused : PlayerState.playing,
          );
          if (!pause) expect(state.positionMs, 1234);
          expect(dlna.stopped, isEmpty);
        },
      );
    }

    test(
      'switching to local rejects the previous renderer position response',
      () async {
        dlna.positionGate = Completer<DlnaPositionInfo>();
        container
            .read(dlnaSelectedRendererProvider.notifier)
            .set(_renderer('A'));
        await until(() => dlna.positionCalls == 1);
        container.read(dlnaSelectedRendererProvider.notifier).set(null);
        await until(() => dlna.unpublishes == 1);
        local.eventStream.add(
          const Event.stateChanged(state: PlayerState.playing),
        );
        local.eventStream.add(
          Event.position(
            ms: 321,
            trackId: BigInt.one,
            itemId: BigInt.one,
            sessionId: BigInt.one,
          ),
        );
        dlna.positionGate!.complete(
          DlnaPositionInfo(relTimeMs: BigInt.from(7777)),
        );
        await Future<void>.delayed(Duration.zero);
        expect(container.read(playbackControllerProvider).positionMs, 321);
        expect(
          container.read(playbackControllerProvider).playerState,
          PlayerState.playing,
        );
      },
    );

    test(
      'old seek failure cannot change a newly selected renderer state',
      () async {
        container
            .read(dlnaSelectedRendererProvider.notifier)
            .set(_renderer('A'));
        await until(() => local.stopCalls == 1);
        dlna.seekGate = Completer<void>();
        final seek = controller.seekMs(4444);
        await until(() => dlna.seekCalls == 1);
        container
            .read(dlnaSelectedRendererProvider.notifier)
            .set(_renderer('B'));
        dlna.seekGate!.completeError(StateError('renderer A left'));
        await seek;
        await until(() => local.stopCalls == 2);
        expect(container.read(playbackControllerProvider).positionMs, 0);
        expect(container.read(playbackControllerProvider).lastError, isNull);
        expect(dlna.stopped, ['http://A/transport']);
      },
    );

    test(
      'local queue refresh cannot replace a DLNA queue after output switches',
      () async {
        local.queueGate = Completer<PlaybackQueue>();
        final calls = local.queueCalls;
        local.eventStream.add(
          Event.trackChanged(trackId: BigInt.two, itemId: BigInt.two),
        );
        await until(() => local.queueCalls > calls);
        container
            .read(dlnaSelectedRendererProvider.notifier)
            .set(_renderer('A'));
        container.read(queueControllerProvider.notifier).setQueue([
          const QueueItem(trackId: null, path: 'dlna.mp3'),
        ]);
        local.queueGate!.complete(local.queue);
        await Future<void>.delayed(Duration.zero);
        expect(
          container.read(queueControllerProvider).items.single.path,
          'dlna.mp3',
        );
      },
    );
  });
}
