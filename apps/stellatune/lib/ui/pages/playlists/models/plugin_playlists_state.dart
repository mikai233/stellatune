import 'package:flutter/foundation.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/playlists/models/playlists_data_models.dart';

@immutable
class PluginPlaylistsState {
  const PluginPlaylistsState({
    this.entries = const [],
    this.refreshing = false,
    this.listError,
    this.selection,
  });

  final List<PluginPlaylistEntry> entries;
  final bool refreshing;
  final String? listError;
  final PluginPlaylistSelection? selection;

  PluginPlaylistsState copyWith({
    List<PluginPlaylistEntry>? entries,
    bool? refreshing,
    Object? listError = _unchanged,
    Object? selection = _unchanged,
  }) => PluginPlaylistsState(
    entries: entries ?? this.entries,
    refreshing: refreshing ?? this.refreshing,
    listError: identical(listError, _unchanged)
        ? this.listError
        : listError as String?,
    selection: identical(selection, _unchanged)
        ? this.selection
        : selection as PluginPlaylistSelection?,
  );
}

@immutable
class PluginPlaylistSelection {
  const PluginPlaylistSelection({
    required this.entry,
    this.tracks = const [],
    this.nextOffset = 0,
    this.nextCursor,
    this.hasMore = false,
    this.loading = false,
    this.loadingMore = false,
    this.error,
  });

  final PluginPlaylistEntry entry;
  final List<QueueItem> tracks;
  final int nextOffset;
  final String? nextCursor;
  final bool hasMore;
  final bool loading;
  final bool loadingMore;
  final String? error;

  List<QueueItem> filter(String query) {
    final normalized = query.trim().toLowerCase();
    if (normalized.isEmpty) return tracks;
    return tracks.where((item) {
      return (item.title ?? '').toLowerCase().contains(normalized) ||
          (item.artist ?? '').toLowerCase().contains(normalized) ||
          (item.album ?? '').toLowerCase().contains(normalized);
    }).toList();
  }

  PluginPlaylistSelection copyWith({
    PluginPlaylistEntry? entry,
    List<QueueItem>? tracks,
    int? nextOffset,
    Object? nextCursor = _unchanged,
    bool? hasMore,
    bool? loading,
    bool? loadingMore,
    Object? error = _unchanged,
  }) => PluginPlaylistSelection(
    entry: entry ?? this.entry,
    tracks: tracks ?? this.tracks,
    nextOffset: nextOffset ?? this.nextOffset,
    nextCursor: identical(nextCursor, _unchanged)
        ? this.nextCursor
        : nextCursor as String?,
    hasMore: hasMore ?? this.hasMore,
    loading: loading ?? this.loading,
    loadingMore: loadingMore ?? this.loadingMore,
    error: identical(error, _unchanged) ? this.error : error as String?,
  );
}

const _unchanged = Object();
