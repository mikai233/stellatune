import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    as frb;
import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/player/playback_playability_utils.dart';

PlaybackQueue snapshot(
  List<int> ids, {
  int? current,
  int? requested,
  int revision = 1,
  List<int>? order,
}) => PlaybackQueue(
  items: [
    for (final id in ids)
      QueueEntry(itemId: BigInt.from(id), trackId: BigInt.from(42)),
  ],
  order: frb.Uint64List.fromList(order ?? ids),
  currentItemId: current == null ? null : BigInt.from(current),
  requestedItemId: requested == null ? null : BigInt.from(requested),
  repeatMode: QueueRepeatMode.off,
  shuffle: order != null,
  revision: BigInt.from(revision),
);

void main() {
  late Directory directory;
  late ProviderContainer container;
  setUp(() async {
    directory = await Directory.systemTemp.createTemp('stellatune_queue_test_');
    Hive.init(directory.path);
    await Hive.openBox('settings');
    container = ProviderContainer(
      overrides: [
        settingsStoreServiceProvider.overrideWithValue(SettingsStore()),
      ],
    );
  });
  tearDown(() async {
    container.dispose();
    await Hive.close();
    await directory.delete(recursive: true);
  });

  test(
    'duplicate tracks retain occurrence identity after removal and reordering',
    () {
      final controller = container.read(queueControllerProvider.notifier);
      controller.applyBackend(
        snapshot([10, 11, 12], current: 11),
        metadata: [
          QueueItem(
            itemId: BigInt.from(10),
            trackId: null,
            path: 'same',
            title: 'first',
          ),
          QueueItem(
            itemId: BigInt.from(11),
            trackId: null,
            path: 'same',
            title: 'second',
          ),
          QueueItem(
            itemId: BigInt.from(12),
            trackId: null,
            path: 'same',
            title: 'third',
          ),
        ],
      );
      controller.applyBackend(
        snapshot([11, 12], current: 11, order: [12, 11], revision: 2),
      );
      final queue = container.read(queueControllerProvider);
      expect(queue.currentItem?.itemId, BigInt.from(11));
      expect(queue.currentItem?.title, 'second');
      expect(queue.order, [1, 0]);
      expect(queue.items[1].title, 'third');
      expect(queue.items[1].isLocal, isFalse);
    },
  );

  test('a large queue projects occurrence order and the selected item', () {
    final ids = List.generate(10000, (index) => index + 1);
    final controller = container.read(queueControllerProvider.notifier);
    controller.applyBackend(
      snapshot(ids, current: 9501, order: ids.reversed.toList()),
    );
    final queue = container.read(queueControllerProvider);
    expect(queue.items.length, 10000);
    expect(queue.currentIndex, 9500);
    expect(queue.order, List.generate(10000, (index) => 9999 - index));
  });

  test('duplicate snapshot cannot undo a newer audible track observation', () {
    final controller = container.read(queueControllerProvider.notifier);
    controller.applyBackend(snapshot([10, 11], current: 10));
    controller.observeCurrent(BigInt.from(11));
    controller.applyBackend(snapshot([10, 11], current: 10));
    expect(
      container.read(queueControllerProvider).currentItem?.itemId,
      BigInt.from(11),
    );
  });

  test(
    'provider rebuild accepts the initial snapshot at the same revision',
    () {
      final initial = snapshot([10, 11], current: 11, revision: 8);
      container.read(queueControllerProvider.notifier).applyBackend(initial);
      container.invalidate(queueControllerProvider);
      container.read(queueControllerProvider.notifier).applyBackend(initial);
      expect(
        container.read(queueControllerProvider).currentItem?.itemId,
        BigInt.from(11),
      );
    },
  );

  test(
    'provider projection preserves plugin ownership for disable handling',
    () {
      final controller = container.read(queueControllerProvider.notifier);
      controller.applyBackend(
        PlaybackQueue(
          items: [
            QueueEntry(
              itemId: BigInt.one,
              trackId: BigInt.two,
              providerTrack: const QueueProviderTrack(
                providerId: 'account',
                providerKey: '42',
                pluginId: 'source-plugin',
                capabilityId: 'source',
              ),
            ),
          ],
          order: frb.Uint64List.fromList([1]),
          repeatMode: QueueRepeatMode.off,
          shuffle: false,
          revision: BigInt.one,
        ),
      );
      expect(
        PlaybackPlayabilityUtils.disabledPluginBlockReason(
          item: container.read(queueControllerProvider).items.single,
          disabledPluginIds: {'source-plugin'},
        ),
        'source_catalog_unavailable',
      );
    },
  );

  test('authoritative presentation can clear previously displayed fields', () {
    final controller = container.read(queueControllerProvider.notifier);
    PlaybackQueue withMetadata(TrackPresentation metadata, int revision) =>
        PlaybackQueue(
          items: [
            QueueEntry(
              itemId: BigInt.one,
              trackId: BigInt.two,
              metadata: metadata,
            ),
          ],
          order: frb.Uint64List.fromList([1]),
          repeatMode: QueueRepeatMode.off,
          shuffle: false,
          revision: BigInt.from(revision),
        );
    controller.applyBackend(
      withMetadata(
        const TrackPresentation(
          title: 'Old',
          artist: 'Old artist',
          album: 'Old album',
          cover: TrackCover(
            kind: TrackCoverKind.url,
            value: 'https://example.test/cover.jpg',
          ),
        ),
        1,
      ),
    );
    controller.applyBackend(
      withMetadata(const TrackPresentation(title: 'Updated'), 2),
    );
    final item = container.read(queueControllerProvider).items.single;
    expect(item.title, 'Updated');
    expect(item.artist, isNull);
    expect(item.album, isNull);
    expect(item.cover, isNull);
  });

  test(
    'external replacement clears a stale source but append preserves it',
    () async {
      final controller = container.read(queueControllerProvider.notifier);
      const source = QueueSource(
        type: QueueSourceType.folder,
        folderPath: 'music',
      );
      controller.applyBackend(
        snapshot([10], current: 10),
        source: source,
        replaceSource: true,
      );
      controller.applyBackend(snapshot([10, 11], current: 10, revision: 2));
      expect(container.read(queueControllerProvider).source, source);
      controller.applyBackend(snapshot([20], current: 20, revision: 3));
      expect(container.read(queueControllerProvider).source, isNull);
      await Future<void>.delayed(Duration.zero);
      expect(container.read(settingsStoreProvider).queueSource, isNull);
    },
  );

  test('metadata is matched by identity even when a concurrent edit changes ordering', () {
    final controller = container.read(queueControllerProvider.notifier);
    controller.applyBackend(
      PlaybackQueue(
        items: [
          QueueEntry(itemId: BigInt.from(20), trackId: BigInt.two),
          QueueEntry(itemId: BigInt.from(10), trackId: BigInt.one),
        ],
        order: frb.Uint64List.fromList([20, 10]),
        repeatMode: QueueRepeatMode.off,
        shuffle: false,
        revision: BigInt.one,
      ),
      metadata: [
        QueueItem(trackId: BigInt.one, path: '', title: 'First track'),
        QueueItem(trackId: BigInt.two, path: '', title: 'Second track'),
      ],
    );
    expect(
      container.read(queueControllerProvider).items.map((item) => item.title),
      ['Second track', 'First track'],
    );
  });

  test('requested target does not move the audible cursor and stale snapshots are ignored', () {
    final controller = container.read(queueControllerProvider.notifier);
    controller.applyBackend(
      snapshot([10, 11], current: 10, requested: 11, revision: 3),
    );
    expect(
      container.read(queueControllerProvider).currentItem?.itemId,
      BigInt.from(10),
    );
    controller.observeCurrent(BigInt.from(11));
    controller.applyBackend(snapshot([10, 11], current: 10, revision: 2));
    expect(
      container.read(queueControllerProvider).currentItem?.itemId,
      BigInt.from(11),
    );
  });

  test('removing the current occurrence does not select a different copy of the song', () {
    final controller = container.read(queueControllerProvider.notifier);
    controller.applyBackend(snapshot([10, 11], current: 10));
    controller.applyBackend(snapshot([11], revision: 2));
    expect(container.read(queueControllerProvider).currentItem, isNull);
  });
}
