import 'dart:math';

import 'package:flutter/foundation.dart';

@immutable
class QueueItem {
  const QueueItem({
    required this.path,
    this.id,
    this.title,
    this.artist,
    this.album,
    this.durationMs,
  });

  final String path;
  final int? id;
  final String? title;
  final String? artist;
  final String? album;
  final int? durationMs;

  String get displayTitle => title?.trim().isNotEmpty == true
      ? title!.trim()
      : path.split(RegExp(r'[\\/]+')).last;
}

enum RepeatMode { off, all, one }

@immutable
class QueueState {
  const QueueState({
    required this.items,
    required this.currentIndex,
    required this.shuffle,
    required this.repeatMode,
    required this.order,
    required this.orderPos,
  });

  const QueueState.empty()
    : items = const [],
      currentIndex = null,
      shuffle = false,
      repeatMode = RepeatMode.off,
      order = const [],
      orderPos = 0;

  final List<QueueItem> items;
  final int? currentIndex;
  final bool shuffle;
  final RepeatMode repeatMode;
  final List<int> order;
  final int orderPos;

  QueueItem? get currentItem {
    final idx = currentIndex;
    if (idx == null || idx < 0 || idx >= items.length) return null;
    return items[idx];
  }

  QueueState copyWith({
    List<QueueItem>? items,
    int? currentIndex,
    bool? shuffle,
    RepeatMode? repeatMode,
    List<int>? order,
    int? orderPos,
  }) {
    return QueueState(
      items: items ?? this.items,
      currentIndex: currentIndex ?? this.currentIndex,
      shuffle: shuffle ?? this.shuffle,
      repeatMode: repeatMode ?? this.repeatMode,
      order: order ?? this.order,
      orderPos: orderPos ?? this.orderPos,
    );
  }
}

List<int> buildOrder({
  required int length,
  required int startIndex,
  required bool shuffle,
  Random? random,
}) {
  final indices = List<int>.generate(length, (i) => i);
  if (!shuffle) return indices;

  final r = random ?? Random();
  indices.remove(startIndex);
  indices.shuffle(r);
  return [startIndex, ...indices];
}
