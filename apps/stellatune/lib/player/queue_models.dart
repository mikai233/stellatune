import 'dart:math';
import 'package:flutter/foundation.dart';

enum QueueCoverKind { url, file, data }

@immutable
class ProviderQueueTrack {
  const ProviderQueueTrack({
    required this.providerId,
    required this.pluginId,
    required this.typeId,
    required this.configJson,
    required this.providerKey,
    this.pathHint = '',
    this.sourcePluginId,
    this.decoderPluginId,
  });

  final String providerId;
  final String pluginId;
  final String typeId;
  final String configJson;
  final String providerKey;
  final String pathHint;
  final String? sourcePluginId;
  final String? decoderPluginId;
}

@immutable
class QueueCover {
  const QueueCover({required this.kind, required this.value, this.mime});

  final QueueCoverKind kind;
  final String value;
  final String? mime;
}

@immutable
class QueueItem {
  const QueueItem({
    required this.trackId,
    required this.path,
    this.providerTrack,
    this.id,
    this.title,
    this.artist,
    this.album,
    this.durationMs,
    this.cover,
  });

  /// Stable application TrackId. Provider rows receive it lazily at play/queue time.
  final BigInt? trackId;
  final String path;
  final ProviderQueueTrack? providerTrack;
  final int? id;
  final String? title;
  final String? artist;
  final String? album;
  final int? durationMs;
  final QueueCover? cover;

  String get stableTrackKey => trackId?.toString() ??
      '${providerTrack?.providerId ?? "local"}:${providerTrack?.providerKey ?? id ?? path}';

  String get displayTitle {
    final explicit = title?.trim() ?? '';
    if (explicit.isNotEmpty) return explicit;
    final fallback = path.trim().isNotEmpty
        ? path
        : providerTrack?.providerKey ?? trackId?.toString() ?? '';
    return fallback.split(RegExp(r'[\\/]+')).last;
  }
}

enum RepeatMode { off, all, one }

enum PlayMode { sequential, shuffle, repeatAll, repeatOne }

enum QueueSourceType { all, folder, playlist }

@immutable
class QueueSource {
  const QueueSource({
    required this.type,
    this.folderPath,
    this.includeSubfolders = false,
    this.playlistId,
    this.label,
  });

  final QueueSourceType type;
  final String? folderPath;
  final bool includeSubfolders;
  final int? playlistId;
  final String? label;

  Map<String, dynamic> toJson() => {
    'type': type.name,
    'folderPath': folderPath,
    'includeSubfolders': includeSubfolders,
    'playlistId': playlistId,
    'label': label,
  };

  factory QueueSource.fromJson(Map<String, dynamic> json) {
    return QueueSource(
      type: QueueSourceType.values.firstWhere(
        (e) => e.name == json['type'],
        orElse: () => QueueSourceType.all,
      ),
      folderPath: json['folderPath'] as String?,
      includeSubfolders: json['includeSubfolders'] as bool? ?? false,
      playlistId: json['playlistId'] as int?,
      label: json['label'] as String?,
    );
  }
}

@immutable
class QueueState {
  const QueueState({
    required this.items,
    required this.currentIndex,
    required this.shuffle,
    required this.repeatMode,
    required this.order,
    required this.orderPos,
    required this.source,
  });

  const QueueState.empty()
    : items = const [],
      currentIndex = null,
      shuffle = false,
      repeatMode = RepeatMode.off,
      order = const [],
      orderPos = 0,
      source = null;

  final List<QueueItem> items;
  final int? currentIndex;
  final bool shuffle;
  final RepeatMode repeatMode;
  final List<int> order;
  final int orderPos;
  final QueueSource? source;

  String? get sourceLabel => source?.label;

  PlayMode get playMode {
    if (repeatMode == RepeatMode.one) return PlayMode.repeatOne;
    if (repeatMode == RepeatMode.all) return PlayMode.repeatAll;
    if (shuffle) return PlayMode.shuffle;
    return PlayMode.sequential;
  }

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
    Object? source = _queueSentinel,
  }) {
    return QueueState(
      items: items ?? this.items,
      currentIndex: currentIndex ?? this.currentIndex,
      shuffle: shuffle ?? this.shuffle,
      repeatMode: repeatMode ?? this.repeatMode,
      order: order ?? this.order,
      orderPos: orderPos ?? this.orderPos,
      source: identical(source, _queueSentinel)
          ? this.source
          : source as QueueSource?,
    );
  }
}

const Object _queueSentinel = Object();

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
