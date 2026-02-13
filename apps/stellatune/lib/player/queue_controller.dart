import 'dart:async';
import 'dart:math';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/player/queue_models.dart';

final queueControllerProvider = NotifierProvider<QueueController, QueueState>(
  QueueController.new,
);

class QueueController extends Notifier<QueueState> {
  final Random _random = Random();

  int _orderPosFor(List<int> order, int currentIndex) {
    final pos = order.indexOf(currentIndex);
    return pos >= 0 ? pos : 0;
  }

  @override
  QueueState build() {
    final settings = ref.read(settingsStoreProvider);
    final mode = settings.playMode;
    final shuffle = mode == PlayMode.shuffle;
    final repeatMode = switch (mode) {
      PlayMode.sequential => RepeatMode.off,
      PlayMode.shuffle => RepeatMode.off,
      PlayMode.repeatAll => RepeatMode.all,
      PlayMode.repeatOne => RepeatMode.one,
    };
    final source = settings.queueSource;
    return const QueueState.empty().copyWith(
      shuffle: shuffle,
      repeatMode: repeatMode,
      source: source,
    );
  }

  void setQueue(
    List<QueueItem> items, {
    int startIndex = 0,
    QueueSource? source,
  }) {
    if (items.isEmpty) {
      state = const QueueState.empty().copyWith(
        shuffle: state.shuffle,
        repeatMode: state.repeatMode,
        source: null,
      );
      unawaited(ref.read(settingsStoreProvider).setQueueSource(null));
      return;
    }

    final idx = startIndex.clamp(0, items.length - 1);
    final order = buildOrder(
      length: items.length,
      startIndex: idx,
      shuffle: state.shuffle,
      random: _random,
    );

    state = state.copyWith(
      items: List.of(items),
      currentIndex: idx,
      order: order,
      orderPos: _orderPosFor(order, idx),
      source: source,
    );
    unawaited(ref.read(settingsStoreProvider).setQueueSource(source));
  }

  void enqueue(List<QueueItem> items) {
    if (items.isEmpty) return;

    if (state.items.isEmpty) {
      setQueue(items, startIndex: 0);
      return;
    }

    final merged = [...state.items, ...items];
    final currentIndex = state.currentIndex ?? 0;

    // Rebuild order after enqueue while preserving the currently selected item.
    final order = buildOrder(
      length: merged.length,
      startIndex: currentIndex.clamp(0, merged.length - 1),
      shuffle: state.shuffle,
      random: _random,
    );

    state = state.copyWith(
      items: merged,
      order: order,
      orderPos: _orderPosFor(order, currentIndex),
      currentIndex: currentIndex,
    );
  }

  void selectIndex(int index) {
    if (index < 0 || index >= state.items.length) return;
    final order = buildOrder(
      length: state.items.length,
      startIndex: index,
      shuffle: state.shuffle,
      random: _random,
    );
    state = state.copyWith(
      currentIndex: index,
      order: order,
      orderPos: _orderPosFor(order, index),
    );
  }

  QueueItem? next({bool fromAuto = false}) {
    final current = state.currentItem;
    if (current == null) return null;

    if (state.repeatMode == RepeatMode.one) {
      return current;
    }

    if (state.orderPos + 1 < state.order.length) {
      final newPos = state.orderPos + 1;
      final newIndex = state.order[newPos];
      state = state.copyWith(currentIndex: newIndex, orderPos: newPos);
      return state.currentItem;
    }

    // End of order.
    if (state.repeatMode == RepeatMode.all) {
      final startIndex = state.shuffle ? state.order.last : 0;
      final order = buildOrder(
        length: state.items.length,
        startIndex: startIndex.clamp(0, state.items.length - 1),
        shuffle: state.shuffle,
        random: _random,
      );
      state = state.copyWith(currentIndex: order[0], order: order, orderPos: 0);
      return state.currentItem;
    }

    // repeat off
    return null;
  }

  QueueItem? previous() {
    final current = state.currentItem;
    if (current == null) return null;

    if (state.repeatMode == RepeatMode.one) {
      return current;
    }

    if (state.orderPos > 0) {
      final newPos = state.orderPos - 1;
      final newIndex = state.order[newPos];
      state = state.copyWith(currentIndex: newIndex, orderPos: newPos);
      return state.currentItem;
    }

    if (state.order.isNotEmpty) {
      final newPos = state.order.length - 1;
      final newIndex = state.order[newPos];
      state = state.copyWith(currentIndex: newIndex, orderPos: newPos);
      return state.currentItem;
    }

    return null;
  }

  void toggleShuffle() {
    final shuffle = !state.shuffle;
    final currentIndex = state.currentIndex;
    if (currentIndex == null || state.items.isEmpty) {
      state = state.copyWith(shuffle: shuffle);
      return;
    }

    final order = buildOrder(
      length: state.items.length,
      startIndex: currentIndex,
      shuffle: shuffle,
      random: _random,
    );
    state = state.copyWith(
      shuffle: shuffle,
      order: order,
      orderPos: _orderPosFor(order, currentIndex),
    );
  }

  void cyclePlayMode() {
    final next = switch (state.playMode) {
      PlayMode.sequential => PlayMode.shuffle,
      PlayMode.shuffle => PlayMode.repeatAll,
      PlayMode.repeatAll => PlayMode.repeatOne,
      PlayMode.repeatOne => PlayMode.sequential,
    };
    setPlayMode(next);
  }

  void setPlayMode(PlayMode mode) {
    unawaited(ref.read(settingsStoreProvider).setPlayMode(mode));
    final desiredShuffle = mode == PlayMode.shuffle;
    final desiredRepeat = switch (mode) {
      PlayMode.sequential => RepeatMode.off,
      PlayMode.shuffle => RepeatMode.off,
      PlayMode.repeatAll => RepeatMode.all,
      PlayMode.repeatOne => RepeatMode.one,
    };

    final currentIndex = state.currentIndex;
    if (currentIndex == null || state.items.isEmpty) {
      state = state.copyWith(
        shuffle: desiredShuffle,
        repeatMode: desiredRepeat,
      );
      return;
    }

    final order = buildOrder(
      length: state.items.length,
      startIndex: currentIndex,
      shuffle: desiredShuffle,
      random: _random,
    );

    state = state.copyWith(
      shuffle: desiredShuffle,
      repeatMode: desiredRepeat,
      order: order,
      orderPos: _orderPosFor(order, currentIndex),
    );
  }

  void cycleRepeatMode() {
    final next = switch (state.repeatMode) {
      RepeatMode.off => RepeatMode.all,
      RepeatMode.all => RepeatMode.one,
      RepeatMode.one => RepeatMode.off,
    };
    state = state.copyWith(repeatMode: next);
  }

  void clear() {
    state = const QueueState.empty().copyWith(
      shuffle: state.shuffle,
      repeatMode: state.repeatMode,
      source: null,
    );
    unawaited(ref.read(settingsStoreProvider).setQueueSource(null));
  }

  int removeIndices(Set<int> indices) {
    if (indices.isEmpty || state.items.isEmpty) return 0;

    final valid = indices
        .where((i) => i >= 0 && i < state.items.length)
        .toSet();
    if (valid.isEmpty) return 0;

    final oldItems = state.items;
    final oldCurrent = state.currentIndex ?? -1;
    final oldCurrentItem = state.currentItem;

    final nextItems = <QueueItem>[
      for (var i = 0; i < oldItems.length; i++)
        if (!valid.contains(i)) oldItems[i],
    ];

    final removed = oldItems.length - nextItems.length;
    if (removed <= 0) return 0;

    if (nextItems.isEmpty) {
      state = const QueueState.empty().copyWith(
        shuffle: state.shuffle,
        repeatMode: state.repeatMode,
        source: null,
      );
      unawaited(ref.read(settingsStoreProvider).setQueueSource(null));
      return removed;
    }

    var nextCurrent = -1;
    if (oldCurrentItem != null) {
      final keepKey = oldCurrentItem.stableTrackKey;
      nextCurrent = nextItems.indexWhere((it) => it.stableTrackKey == keepKey);
    }
    if (nextCurrent < 0) {
      for (var i = oldCurrent + 1; i < oldItems.length; i++) {
        if (!valid.contains(i)) {
          final key = oldItems[i].stableTrackKey;
          nextCurrent = nextItems.indexWhere((it) => it.stableTrackKey == key);
          if (nextCurrent >= 0) break;
        }
      }
    }
    if (nextCurrent < 0) {
      for (var i = oldCurrent - 1; i >= 0; i--) {
        if (!valid.contains(i)) {
          final key = oldItems[i].stableTrackKey;
          nextCurrent = nextItems.indexWhere((it) => it.stableTrackKey == key);
          if (nextCurrent >= 0) break;
        }
      }
    }
    if (nextCurrent < 0) {
      nextCurrent = 0;
    }

    final order = buildOrder(
      length: nextItems.length,
      startIndex: nextCurrent,
      shuffle: state.shuffle,
      random: _random,
    );
    state = state.copyWith(
      items: nextItems,
      currentIndex: nextCurrent,
      order: order,
      orderPos: _orderPosFor(order, nextCurrent),
    );
    return removed;
  }
}
