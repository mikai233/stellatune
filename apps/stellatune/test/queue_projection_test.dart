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
        metadata: const [
          QueueItem(trackId: null, path: 'same', title: 'first'),
          QueueItem(trackId: null, path: 'same', title: 'second'),
          QueueItem(trackId: null, path: 'same', title: 'third'),
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
