import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    as frb;
import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/bridge/api/error.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

class ControlledBridge implements PlayerBridge {
  final eventStream = StreamController<Event>.broadcast(sync: true);
  final queueStream = StreamController<PlaybackQueue>.broadcast(sync: true);
  final selections = <BigInt, Completer<bool>>{};
  Completer<void>? retainGate;
  Completer<PlaybackSnapshot>? snapshotGate;
  int snapshotCalls = 0;
  int stopCalls = 0;
  int replaceCalls = 0;
  int nextCalls = 0, previousCalls = 0;
  TrackDecodeInfo? trackInfo;
  Completer<TrackDecodeInfo?>? trackInfoGate;
  final seeks = <int>[];
  final queue = PlaybackQueue(
    items: [
      for (var id = 1; id <= 3; id++)
        QueueEntry(
          itemId: BigInt.from(id),
          trackId: BigInt.from(id),
          localLibraryTrackId: id,
          localPath: '$id.mp3',
        ),
    ],
    order: frb.Uint64List.fromList([1, 2, 3]),
    currentItemId: BigInt.one,
    repeatMode: QueueRepeatMode.off,
    shuffle: false,
    revision: BigInt.one,
  );
  @override
  Stream<Event> events() => eventStream.stream;
  @override
  Stream<PlaybackQueue> queueEvents() => queueStream.stream;
  @override
  Future<void> setVolume(
    double value, {
    required int seq,
    required int rampMs,
  }) async {}
  @override
  Future<List<String>> decoderSupportedExtensions() async => ['mp3'];
  @override
  Future<PlaybackSnapshot> playbackSnapshot() async {
    snapshotCalls++;
    return await snapshotGate?.future ??
        const PlaybackSnapshot(state: PlayerState.stopped, positionMs: 0);
  }

  @override
  Future<PlaybackQueue> playbackQueue() async => queue;
  @override
  Future<void> retainQueuePaths(Iterable<String> paths) async {
    final gate = retainGate;
    retainGate = null;
    await gate?.future;
  }

  @override
  Future<void> releaseRemovedQueuePaths(Iterable<String> paths) async {}
  @override
  Future<PlaybackQueue> replaceQueue(List<BigInt> ids) async {
    replaceCalls++;
    return queue;
  }

  @override
  Future<PlaybackQueue> setQueueMode(
    QueueRepeatMode repeat,
    bool shuffle,
  ) async => queue;
  @override
  Future<bool> selectQueueItem(BigInt itemId, {bool autoplay = true}) {
    final completion = Completer<bool>();
    selections[itemId] = completion;
    return completion.future;
  }

  @override
  Future<bool> nextQueueItem() async {
    nextCalls++;
    return true;
  }

  @override
  Future<bool> previousQueueItem() async {
    previousCalls++;
    return true;
  }

  @override
  Future<void> stop() async {
    stopCalls++;
  }

  @override
  Future<TrackDecodeInfo?> currentTrackInfo() async {
    final gate = trackInfoGate;
    trackInfoGate = null;
    return gate == null ? trackInfo : await gate.future;
  }

  @override
  Future<void> play() async {
    eventStream.add(const Event.stateChanged(state: PlayerState.playing));
    eventStream.add(const Event.audioStart());
  }

  @override
  Future<void> seekMs(int positionMs) async => seeks.add(positionMs);
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

Future<void> until(bool Function() predicate) => Future<void>(() async {
  while (!predicate()) {
    await Future<void>.delayed(const Duration(milliseconds: 1));
  }
}).timeout(const Duration(seconds: 2));

void main() {
  late Directory directory;
  late ProviderContainer container;
  late ControlledBridge bridge;
  late PlaybackController controller;
  setUp(() async {
    directory = await Directory.systemTemp.createTemp('stellatune_navigation_');
    Hive.init(directory.path);
    await Hive.openBox('settings');
    bridge = ControlledBridge();
    container = ProviderContainer(
      overrides: [
        settingsStoreServiceProvider.overrideWithValue(SettingsStore()),
        playerBridgeProvider.overrideWithValue(bridge),
      ],
    );
    controller = container.read(playbackControllerProvider.notifier);
    await until(() => container.read(queueControllerProvider).items.isNotEmpty);
    await Future<void>.delayed(Duration.zero);
  });
  tearDown(() async {
    await Future<void>.delayed(Duration.zero);
    container.dispose();
    await bridge.eventStream.close();
    await bridge.queueStream.close();
    await Hive.close();
    await directory.delete(recursive: true);
  });

  test(
    'background playback events and stream errors never show notices',
    () async {
      final diagnostics = DiagnosticsService.instance;
      diagnostics.notice.value = null;
      bridge.eventStream.add(
        const Event.error(
          error: AppError(
            category: ErrorCategory.unavailable,
            operation: 'playback_event',
            diagnosticId: 'background-playback',
            fingerprint: 'background-playback',
            context: '',
          ),
        ),
      );
      await Future<void>.delayed(Duration.zero);
      expect(container.read(playbackControllerProvider).lastError, isNotNull);
      expect(diagnostics.notice.value, isNull);
      bridge.eventStream.addError(StateError('background stream disconnected'));
      await Future<void>.delayed(Duration.zero);
      expect(diagnostics.notice.value, isNull);
      expect(
        diagnostics.records.any(
          (r) => r.details.contains('background stream disconnected'),
        ),
        isTrue,
      );
    },
  );

  test(
    'stale failure cannot stop a newer selection or erase pending feedback',
    () async {
      final first = controller.playIndex(1);
      await until(() => bridge.selections.containsKey(BigInt.two));
      final second = controller.playIndex(2);
      expect(
        container.read(playbackControllerProvider).pendingItem?.itemId,
        BigInt.from(3),
      );
      await until(() => bridge.selections.containsKey(BigInt.from(3)));
      bridge.selections[BigInt.two]!.completeError(
        StateError('old request failed'),
      );
      await first;
      expect(bridge.stopCalls, 0);
      expect(container.read(playbackControllerProvider).lastError, isNull);
      expect(
        container.read(playbackControllerProvider).pendingItem?.itemId,
        BigInt.from(3),
      );
      bridge.selections[BigInt.from(3)]!.complete(true);
      await second;
      expect(
        container.read(queueControllerProvider).currentItem?.itemId,
        BigInt.one,
      );
      bridge.eventStream.add(
        Event.trackChanged(trackId: BigInt.from(3), itemId: BigInt.from(3)),
      );
      expect(container.read(playbackControllerProvider).pendingItem, isNull);
      expect(
        container.read(queueControllerProvider).currentItem?.itemId,
        BigInt.from(3),
      );
      expect(bridge.stopCalls, 0);
    },
  );

  test('complete queue starts at the selected row and forwards next/previous to the backend', () async {
    final items = [
      for (var id = 1; id <= 3; id++)
        QueueItem(
          trackId: BigInt.from(id),
          path: '$id.mp3',
          title: 'Known title $id',
          catalogItem: CatalogItem(
            artistRefs: const [],
            reference: MediaRef(
              sourceInstanceId: 'local',
              kind: MediaKind.track,
              id: '$id',
            ),
            title: 'Known title $id',
          ),
        ),
    ];
    final pending = controller.setQueueAndPlayItems(items, startIndex: 1);
    await until(() => bridge.selections.containsKey(BigInt.two));
    bridge.selections[BigInt.two]!.complete(true);
    await pending;
    expect(container.read(queueControllerProvider).items, hasLength(3));
    expect(
      container.read(queueControllerProvider).items[1].catalogItem,
      same(items[1].catalogItem),
    );
    expect(
      container.read(playbackControllerProvider).pendingItem?.title,
      'Known title 2',
    );
    expect(bridge.selections.keys, [BigInt.two]);
    await controller.next();
    await controller.previous();
    expect(bridge.nextCalls, 1);
    expect(bridge.previousCalls, 1);
    expect(container.read(queueControllerProvider).items, hasLength(3));
  });

  test('backend superseded response is not a playback failure', () async {
    final pending = controller.playIndex(1);
    await until(() => bridge.selections.containsKey(BigInt.two));
    bridge.selections[BigInt.two]!.complete(false);
    await pending;
    expect(bridge.stopCalls, 0);
    expect(container.read(playbackControllerProvider).pendingItem, isNull);
    expect(container.read(playbackControllerProvider).lastError, isNull);
  });

  test(
    'latest failure still stops playback and a new request clears the error',
    () async {
      DiagnosticsService.instance.notice.value = null;
      final failed = controller.playIndex(1);
      await until(() => bridge.selections.containsKey(BigInt.two));
      bridge.selections[BigInt.two]!.completeError(
        StateError('current request failed'),
      );
      await failed;
      await Future<void>.delayed(Duration.zero);
      expect(DiagnosticsService.instance.notice.value, isNotNull);
      DiagnosticsService.instance.notice.value = null;
      expect(bridge.stopCalls, 1);
      expect(
        container.read(playbackControllerProvider).lastError,
        equals('播放操作失败'),
      );
      expect(container.read(playbackControllerProvider).pendingItem, isNull);
      final next = controller.playIndex(2);
      expect(container.read(playbackControllerProvider).lastError, isNull);
      await until(() => bridge.selections.containsKey(BigInt.from(3)));
      // A boundary can arrive before the command acknowledgement.
      bridge.eventStream.add(
        Event.trackChanged(trackId: BigInt.from(3), itemId: BigInt.from(3)),
      );
      bridge.selections[BigInt.from(3)]!.complete(true);
      await next;
      expect(container.read(playbackControllerProvider).pendingItem, isNull);
    },
  );

  test(
    'superseded queue preparation never replaces the latest queue',
    () async {
      final gate = Completer<void>();
      bridge.retainGate = gate;
      final items = container.read(queueControllerProvider).items;
      final old = controller.setQueueAndPlayItems(items, startIndex: 1);
      final latest = controller.playIndex(2);
      await until(() => bridge.selections.containsKey(BigInt.from(3)));
      gate.complete();
      await old;
      expect(bridge.replaceCalls, 0);
      expect(bridge.selections.keys, [BigInt.from(3)]);
      bridge.selections[BigInt.from(3)]!.complete(true);
      await latest;
      bridge.eventStream.add(
        Event.trackChanged(trackId: BigInt.from(3), itemId: BigInt.from(3)),
      );
    },
  );

  test('stop cancels a selection that has not reached the backend', () async {
    final gate = Completer<void>();
    bridge.retainGate = gate;
    final pending = controller.setQueueAndPlayItems(
      container.read(queueControllerProvider).items,
    );
    await controller.stop();
    gate.complete();
    await pending;
    expect(bridge.replaceCalls, 0);
    expect(bridge.selections, isEmpty);
    expect(container.read(playbackControllerProvider).pendingItem, isNull);
  });

  test('late metadata enriches entries already created by a queue event', () {
    final notifier = container.read(queueControllerProvider.notifier);
    expect(container.read(queueControllerProvider).items.first.title, isNull);
    notifier.applyBackend(
      bridge.queue,
      metadata: [
        QueueItem(
          trackId: BigInt.one,
          path: 'song.ncm',
          title: 'Title',
          artist: 'Artist',
        ),
      ],
    );
    final item = container.read(queueControllerProvider).items.first;
    expect(item.title, 'Title');
    expect(item.artist, 'Artist');
    notifier.applyBackend(bridge.queue);
    expect(container.read(queueControllerProvider).items.first.title, 'Title');
  });

  test(
    'backend queue events update metadata and mode without a playback event',
    () async {
      final id = BigInt.from(90);
      bridge.queueStream.add(
        PlaybackQueue(
          items: [
            QueueEntry(
              itemId: id,
              trackId: BigInt.from(40),
              providerTrack: const QueueProviderTrack(
                providerId: 'netease',
                providerKey: '42',
                pluginId: 'netease-plugin',
                capabilityId: 'source',
              ),
              metadata: TrackPresentation(
                title: 'From HTTP',
                artist: 'Artist',
                durationMs: BigInt.from(42000),
                cover: const TrackCover(
                  kind: TrackCoverKind.url,
                  value: 'https://example.test/cover.jpg',
                ),
              ),
            ),
          ],
          order: frb.Uint64List.fromList([id]),
          repeatMode: QueueRepeatMode.all,
          shuffle: true,
          revision: BigInt.from(20),
        ),
      );
      final queue = container.read(queueControllerProvider);
      expect(queue.items.single.title, 'From HTTP');
      expect(queue.items.single.providerTrack?.providerKey, '42');
      expect(queue.items.single.cover?.value, 'https://example.test/cover.jpg');
      expect(queue.repeatMode, RepeatMode.all);
      expect(queue.shuffle, isTrue);
      bridge.queueStream.add(
        bridge.queue,
      ); // An older initial snapshot arrives late.
      expect(container.read(queueControllerProvider).items.single.itemId, id);
      bridge.queueStream.add(
        PlaybackQueue(
          items: const [],
          order: frb.Uint64List.fromList([]),
          repeatMode: QueueRepeatMode.off,
          shuffle: false,
          revision: BigInt.from(21),
        ),
      );
      expect(container.read(queueControllerProvider).items, isEmpty);
      await Future<void>.delayed(Duration.zero);
    },
  );

  test(
    'startup snapshot cannot overwrite a newer backend track and position',
    () async {
      final gate = Completer<PlaybackSnapshot>();
      bridge.snapshotGate = gate;
      final previousCalls = bridge.snapshotCalls;
      container.invalidate(playbackControllerProvider);
      controller = container.read(playbackControllerProvider.notifier);
      await until(() => bridge.snapshotCalls > previousCalls);
      final id = BigInt.from(3);
      bridge.eventStream.add(Event.trackChanged(trackId: id, itemId: id));
      bridge.eventStream.add(
        const Event.stateChanged(state: PlayerState.playing),
      );
      bridge.eventStream.add(
        Event.position(
          ms: 3000,
          trackId: id,
          itemId: id,
          sessionId: BigInt.one,
        ),
      );
      gate.complete(
        PlaybackSnapshot(
          state: PlayerState.paused,
          positionMs: 500,
          trackId: BigInt.one,
          itemId: BigInt.one,
        ),
      );
      await Future<void>.delayed(Duration.zero);
      final state = container.read(playbackControllerProvider);
      expect(state.playerState, PlayerState.playing);
      expect(state.positionMs, 3000);
      expect(container.read(queueControllerProvider).currentItem?.itemId, id);
    },
  );

  test(
    'startup volume acknowledgements and logs do not cancel position restore',
    () async {
      final gate = Completer<PlaybackSnapshot>();
      bridge.snapshotGate = gate;
      final previousCalls = bridge.snapshotCalls;
      container.invalidate(playbackControllerProvider);
      controller = container.read(playbackControllerProvider.notifier);
      await until(() => bridge.snapshotCalls > previousCalls);
      bridge.eventStream.add(Event.volumeChanged(volume: .5, seq: BigInt.one));
      bridge.eventStream.add(const Event.log(message: 'Output ready'));
      gate.complete(
        PlaybackSnapshot(
          state: PlayerState.paused,
          positionMs: 15000,
          trackId: BigInt.one,
          itemId: BigInt.one,
        ),
      );
      await Future<void>.delayed(Duration.zero);
      final state = container.read(playbackControllerProvider);
      expect(state.playerState, PlayerState.paused);
      expect(state.positionMs, 15000);
      expect(state.currentPath, '1.mp3');
    },
  );

  test(
    'first resume hydrates track info without another trackChanged event',
    () async {
      bridge.trackInfo = TrackDecodeInfo(
        sampleRate: 48000,
        channels: 2,
        durationMs: BigInt.from(120000),
        metadataJson: '{}',
      );
      bridge.snapshotGate = Completer<PlaybackSnapshot>()
        ..complete(
          PlaybackSnapshot(
            state: PlayerState.paused,
            positionMs: 15000,
            trackId: BigInt.one,
            itemId: BigInt.one,
          ),
        );
      container.invalidate(playbackControllerProvider);
      controller = container.read(playbackControllerProvider.notifier);
      await until(
        () => container.read(playbackControllerProvider).positionMs == 15000,
      );
      await Future<void>.delayed(Duration.zero);
      var state = container.read(playbackControllerProvider);
      expect(state.currentPath, '1.mp3');
      expect(state.trackInfo?.durationMs, BigInt.from(120000));
      await controller.play();
      bridge.eventStream.add(
        Event.position(
          ms: 16000,
          trackId: BigInt.one,
          itemId: BigInt.one,
          sessionId: BigInt.one,
        ),
      );
      state = container.read(playbackControllerProvider);
      expect(state.audioStarted, isTrue);
      expect(state.positionMs, 16000);
      await controller.seekMs(60000);
      expect(bridge.seeks, [60000]);
      expect(container.read(playbackControllerProvider).positionMs, 60000);
    },
  );

  test(
    'a queue arriving after trackChanged repairs the current path',
    () async {
      final id = BigInt.from(99);
      bridge.eventStream.add(Event.trackChanged(trackId: id, itemId: id));
      await Future<void>.delayed(Duration.zero);
      expect(container.read(playbackControllerProvider).currentPath, isEmpty);
      bridge.queueStream.add(
        PlaybackQueue(
          items: [
            QueueEntry(
              itemId: id,
              trackId: id,
              localLibraryTrackId: 99,
              localPath: '99.mp3',
            ),
          ],
          order: frb.Uint64List.fromList([99]),
          currentItemId: id,
          repeatMode: QueueRepeatMode.off,
          shuffle: false,
          revision: BigInt.two,
        ),
      );
      expect(container.read(playbackControllerProvider).currentPath, '99.mp3');
    },
  );

  test(
    'late restored decode info cannot replace a newly selected track',
    () async {
      final oldInfo = Completer<TrackDecodeInfo?>();
      bridge.trackInfoGate = oldInfo;
      bridge.snapshotGate = Completer<PlaybackSnapshot>()
        ..complete(
          PlaybackSnapshot(
            state: PlayerState.playing,
            positionMs: 15000,
            trackId: BigInt.one,
            itemId: BigInt.one,
          ),
        );
      container.invalidate(playbackControllerProvider);
      controller = container.read(playbackControllerProvider.notifier);
      await until(() => bridge.trackInfoGate == null);
      expect(container.read(playbackControllerProvider).audioStarted, isTrue);
      bridge.trackInfo = const TrackDecodeInfo(sampleRate: 96000, channels: 2);
      bridge.eventStream.add(
        Event.trackChanged(trackId: BigInt.two, itemId: BigInt.two),
      );
      await Future<void>.delayed(Duration.zero);
      oldInfo.complete(const TrackDecodeInfo(sampleRate: 48000, channels: 2));
      await Future<void>.delayed(Duration.zero);
      final state = container.read(playbackControllerProvider);
      expect(state.currentPath, '2.mp3');
      expect(state.trackInfo?.sampleRate, 96000);
    },
  );

  test(
    'restored queue displays library metadata without a previous UI item',
    () {
      final snapshot = PlaybackQueue(
        items: [
          QueueEntry(
            itemId: BigInt.from(99),
            trackId: BigInt.from(9),
            localLibraryTrackId: 42,
            localPath: 'old-name.ncm',
            localMetadata: TrackLite(
              id: 42,
              path: 'old-name.ncm',
              title: '知夏',
              artist: '兰音Reine',
              album: 'Album',
              durationMs: 123000,
            ),
          ),
        ],
        order: frb.Uint64List.fromList([BigInt.from(99)]),
        repeatMode: QueueRepeatMode.off,
        shuffle: false,
        revision: bridge.queue.revision + BigInt.one,
      );
      container.read(queueControllerProvider.notifier).applyBackend(snapshot);
      final item = container.read(queueControllerProvider).items.single;
      expect(item.displayTitle, '知夏');
      expect(item.artist, '兰音Reine');
      expect(item.album, 'Album');
      expect(item.durationMs, 123000);
      expect(item.id, 42);
    },
  );
}
