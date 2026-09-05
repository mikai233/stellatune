import 'dart:async';
import 'dart:math';

import 'package:stellatune/app/logging.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/player/queue_models.dart';

final queueControllerProvider = NotifierProvider<QueueController, QueueState>(
  QueueController.new,
);

class QueueController extends Notifier<QueueState> {
  final Random _random = Random();
  BigInt _backendRevision = BigInt.zero;
  bool get _remote =>
      ref.read(dlnaSelectedRendererProvider)?.avTransportControlUrl != null;

  void applyBackend(
    PlaybackQueue snapshot, {
    List<QueueItem>? metadata,
    QueueSource? source,
    bool replaceSource = false,
    bool preserveCurrent = false,
  }) {
    if (snapshot.revision < _backendRevision) return;
    _backendRevision = snapshot.revision;
    final existing = {
      for (final item in state.items)
        if (item.itemId != null) item.itemId!: item,
    };
    final items = <QueueItem>[];
    for (var i = 0; i < snapshot.items.length; i++) {
      final entry = snapshot.items[i];
      final old = existing[entry.itemId];
      final supplied = metadata != null && i < metadata.length
          ? metadata[i]
          : null;
      final localMetadata = entry.localMetadata;
      items.add(
        QueueItem(
          itemId: entry.itemId,
          trackId: entry.trackId,
          local: entry.localLibraryTrackId != null,
          path: entry.localPath ?? supplied?.path ?? old?.path ?? '',
          providerTrack: supplied?.providerTrack ?? old?.providerTrack,
          id: entry.localLibraryTrackId?.toInt() ?? supplied?.id ?? old?.id,
          title: localMetadata?.title ?? supplied?.title ?? old?.title,
          artist: localMetadata?.artist ?? supplied?.artist ?? old?.artist,
          album: localMetadata?.album ?? supplied?.album ?? old?.album,
          durationMs:
              localMetadata?.durationMs?.toInt() ??
              supplied?.durationMs ??
              old?.durationMs,
          cover: supplied?.cover ?? old?.cover,
        ),
      );
    }
    final current = preserveCurrent
        ? state.currentItem?.itemId
        : snapshot.currentItemId;
    final index = items.indexWhere((item) => item.itemId == current);
    final indices = {
      for (var i = 0; i < items.length; i++) items[i].itemId!: i,
    };
    final order = [
      for (final id in snapshot.order)
        if (indices.containsKey(id)) indices[id]!,
    ];
    state = QueueState(
      items: items,
      currentIndex: index < 0 ? null : index,
      shuffle: snapshot.shuffle,
      repeatMode: switch (snapshot.repeatMode) {
        QueueRepeatMode.off => RepeatMode.off,
        QueueRepeatMode.all => RepeatMode.all,
        QueueRepeatMode.one => RepeatMode.one,
      },
      order: order,
      orderPos: order.indexOf(index).clamp(0, order.length),
      source: replaceSource ? source : (source ?? state.source),
    );
  }

  void observeCurrent(BigInt itemId) {
    final index = state.items.indexWhere((item) => item.itemId == itemId);
    if (index < 0) return;
    state = state.copyWith(
      currentIndex: index,
      orderPos: state.order.indexOf(index).clamp(0, state.order.length),
    );
  }

  Future<void> _setNativeMode(PlayMode mode) async {
    try {
      final repeat = switch (mode) {
        PlayMode.repeatOne => QueueRepeatMode.one,
        PlayMode.repeatAll => QueueRepeatMode.all,
        _ => QueueRepeatMode.off,
      };
      final snapshot = await ref
          .read(playerBridgeProvider)
          .setQueueMode(repeat, mode == PlayMode.shuffle);
      applyBackend(snapshot, preserveCurrent: true);
    } catch (error) {
      ref.read(loggerProvider).w('set queue mode failed: $error');
    }
  }

  int _orderPosFor(List<int> order, int currentIndex) {
    final pos = order.indexOf(currentIndex);
    return pos >= 0 ? pos : 0;
  }

  QueueItem? _moveToOrderPosition(List<int> order, int orderPos) {
    if (orderPos < 0 || orderPos >= order.length) return null;
    final nextIndex = order[orderPos];
    if (nextIndex < 0 || nextIndex >= state.items.length) return null;
    state = state.copyWith(
      currentIndex: nextIndex,
      order: order,
      orderPos: orderPos,
    );
    return state.currentItem;
  }

  QueueItem? _wrapNext() {
    if (state.items.isEmpty) return null;

    if (state.shuffle) {
      final currentIndex = state.currentIndex;
      if (currentIndex == null ||
          currentIndex < 0 ||
          currentIndex >= state.items.length) {
        return null;
      }

      final order = buildOrder(
        length: state.items.length,
        startIndex: currentIndex,
        shuffle: true,
        random: _random,
      );
      final nextPos = order.length > 1 ? 1 : 0;
      return _moveToOrderPosition(order, nextPos);
    }

    return _moveToOrderPosition(state.order, 0);
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
      unawaited(ref.read(settingsStoreProvider.notifier).setQueueSource(null));
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
    unawaited(ref.read(settingsStoreProvider.notifier).setQueueSource(source));
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

    // End of order: all modes except repeat-one wrap around.
    return _wrapNext();
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
    if (!_remote) {
      setPlayMode(state.shuffle ? PlayMode.sequential : PlayMode.shuffle);
      return;
    }
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
    unawaited(ref.read(settingsStoreProvider.notifier).setPlayMode(mode));
    if (!_remote) {
      unawaited(_setNativeMode(mode));
      return;
    }
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
    if (!_remote) {
      cyclePlayMode();
      return;
    }
    final next = switch (state.repeatMode) {
      RepeatMode.off => RepeatMode.all,
      RepeatMode.all => RepeatMode.one,
      RepeatMode.one => RepeatMode.off,
    };
    state = state.copyWith(repeatMode: next);
  }

  void clear() {
    if (!_remote) {
      unawaited(
        ref.read(playerBridgeProvider).replaceQueue([]).then((snapshot) {
          applyBackend(snapshot);
        }),
      );
      return;
    }
    state = const QueueState.empty().copyWith(
      shuffle: state.shuffle,
      repeatMode: state.repeatMode,
      source: null,
    );
    unawaited(ref.read(settingsStoreProvider.notifier).setQueueSource(null));
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
      unawaited(ref.read(settingsStoreProvider.notifier).setQueueSource(null));
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
