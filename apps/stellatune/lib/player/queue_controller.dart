import 'dart:math';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/player/queue_models.dart';

final queueControllerProvider = NotifierProvider<QueueController, QueueState>(
  QueueController.new,
);

class QueueController extends Notifier<QueueState> {
  final Random _random = Random();

  @override
  QueueState build() => const QueueState.empty();

  void setQueue(List<QueueItem> items, {int startIndex = 0}) {
    if (items.isEmpty) {
      state = const QueueState.empty();
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
      orderPos: 0,
    );
  }

  void enqueue(List<QueueItem> items) {
    if (items.isEmpty) return;

    if (state.items.isEmpty) {
      setQueue(items, startIndex: 0);
      return;
    }

    final merged = [...state.items, ...items];
    final currentIndex = state.currentIndex ?? 0;

    // Rebuild order to include new items while keeping the current item at the front.
    final order = buildOrder(
      length: merged.length,
      startIndex: currentIndex.clamp(0, merged.length - 1),
      shuffle: state.shuffle,
      random: _random,
    );

    state = state.copyWith(
      items: merged,
      order: order,
      orderPos: 0,
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
    state = state.copyWith(currentIndex: index, order: order, orderPos: 0);
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

    if (state.repeatMode == RepeatMode.all && state.order.isNotEmpty) {
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
    state = state.copyWith(shuffle: shuffle, order: order, orderPos: 0);
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
    state = const QueueState.empty();
  }
}
