import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    as frb;
import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

class ControlledBridge implements PlayerBridge {
  final eventStream = StreamController<Event>.broadcast(sync: true);
  final selections = <BigInt, Completer<bool>>{};
  Completer<void>? retainGate;
  int stopCalls = 0;
  int replaceCalls = 0;
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
  Future<void> setVolume(
    double value, {
    required int seq,
    required int rampMs,
  }) async {}
  @override
  Future<List<String>> decoderSupportedExtensions() async => ['mp3'];
  @override
  Future<PlaybackSnapshot> playbackSnapshot() async =>
      PlaybackSnapshot(state: PlayerState.stopped, positionMs: 0);
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
  Future<void> stop() async {
    stopCalls++;
  }

  @override
  Future<TrackDecodeInfo?> currentTrackInfo() async => null;
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
    await Hive.close();
    await directory.delete(recursive: true);
  });

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
      final failed = controller.playIndex(1);
      await until(() => bridge.selections.containsKey(BigInt.two));
      bridge.selections[BigInt.two]!.completeError(
        StateError('current request failed'),
      );
      await failed;
      expect(bridge.stopCalls, 1);
      expect(
        container.read(playbackControllerProvider).lastError,
        contains('current request failed'),
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
