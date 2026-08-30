import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/widgets/queue_cover_image.dart';
import 'package:stellatune/ui/widgets/queue_source_info_card.dart';
import 'package:stellatune/ui/widgets/stellatune_search_field.dart';
import 'package:stellatune/ui/widgets/track_list.dart';

class PlaylistTracksPane extends StatelessWidget {
  const PlaylistTracksPane({
    super.key,
    required this.bridge,
    required this.searchController,
    required this.queueSourceLabel,
    required this.selectedLabel,
    required this.playlists,
    required this.selectedPlaylistId,
    required this.results,
    required this.likedTrackIds,
    required this.coverDir,
    required this.onSearchChanged,
    required this.onActivate,
    required this.onEnqueue,
    required this.onSetLiked,
    required this.onAddToPlaylist,
    required this.onRemoveFromPlaylist,
    required this.blockedReasonByTrackId,
    required this.onViewportRangeChanged,
    this.onMoveInCurrentPlaylist,
    this.onBatchAddToPlaylist,
    this.onBatchRemoveFromCurrentPlaylist,
  });

  final PlayerBridge bridge;
  final TextEditingController searchController;
  final String queueSourceLabel;
  final String selectedLabel;
  final List<PlaylistLite> playlists;
  final int? selectedPlaylistId;
  final List<TrackLite> results;
  final Set<int> likedTrackIds;
  final String coverDir;
  final ValueChanged<String> onSearchChanged;
  final Future<void> Function(int index, List<TrackLite> items) onActivate;
  final Future<void> Function(TrackLite track) onEnqueue;
  final Future<void> Function(TrackLite track, bool liked) onSetLiked;
  final Future<void> Function(TrackLite track, int playlistId) onAddToPlaylist;
  final Future<void> Function(TrackLite track, int playlistId)
  onRemoveFromPlaylist;
  final Map<int, String> blockedReasonByTrackId;
  final void Function(int startIndex, int endIndex) onViewportRangeChanged;
  final Future<void> Function(TrackLite track, int newIndex)?
  onMoveInCurrentPlaylist;
  final Future<void> Function(List<TrackLite> tracks, int playlistId)?
  onBatchAddToPlaylist;
  final Future<void> Function(List<TrackLite> tracks, int playlistId)?
  onBatchRemoveFromCurrentPlaylist;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        StellatuneSearchField(
          controller: searchController,
          onChanged: onSearchChanged,
        ),
        const SizedBox(height: 12),
        QueueSourceInfoCard(queueSourceLabel: queueSourceLabel),
        const SizedBox(height: 12),
        Text(
          selectedLabel,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: theme.textTheme.titleMedium?.copyWith(
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 12),
        Expanded(
          child: TrackList(
            bridge: bridge,
            coverDir: coverDir,
            items: results,
            likedTrackIds: likedTrackIds,
            playlists: playlists,
            currentPlaylistId: selectedPlaylistId,
            onActivate: onActivate,
            onEnqueue: onEnqueue,
            onSetLiked: onSetLiked,
            onAddToPlaylist: onAddToPlaylist,
            onRemoveFromPlaylist: onRemoveFromPlaylist,
            onMoveInCurrentPlaylist: onMoveInCurrentPlaylist,
            onBatchAddToPlaylist: onBatchAddToPlaylist,
            onBatchRemoveFromCurrentPlaylist: onBatchRemoveFromCurrentPlaylist,
            blockedReasonByTrackId: blockedReasonByTrackId,
            onViewportRangeChanged: onViewportRangeChanged,
          ),
        ),
      ],
    );
  }
}

class PluginPlaylistTracksPane extends StatefulWidget {
  const PluginPlaylistTracksPane({
    super.key,
    required this.searchController,
    required this.queueSourceLabel,
    required this.selectedLabel,
    required this.sourceLabel,
    required this.tracks,
    required this.loading,
    required this.loadingMore,
    required this.hasMore,
    required this.filterActive,
    required this.error,
    required this.onSearchChanged,
    required this.onLoadMore,
    required this.onActivate,
    required this.onEnqueue,
  });

  final TextEditingController searchController;
  final String queueSourceLabel;
  final String selectedLabel;
  final String sourceLabel;
  final List<QueueItem> tracks;
  final bool loading;
  final bool loadingMore;
  final bool hasMore;
  final bool filterActive;
  final String? error;
  final ValueChanged<String> onSearchChanged;
  final Future<void> Function() onLoadMore;
  final Future<void> Function(int index, List<QueueItem> items) onActivate;
  final Future<void> Function(QueueItem item) onEnqueue;

  @override
  State<PluginPlaylistTracksPane> createState() =>
      _PluginPlaylistTracksPaneState();
}

class _PluginPlaylistTracksPaneState extends State<PluginPlaylistTracksPane> {
  static const double _loadMoreThreshold = 320;
  static const double _itemExtent = 72;
  final ScrollController _scrollController = ScrollController();
  Future<void>? _pendingLoadMore;
  Timer? _settleTimer;
  bool _deferHeavy = false;
  double _lastPixels = 0.0;
  int _lastMicros = 0;
  String? _pressedTrackKey;

  @override
  void initState() {
    super.initState();
    _scrollController.addListener(_onScroll);
    _schedulePrefetchCheck();
  }

  @override
  void didUpdateWidget(covariant PluginPlaylistTracksPane oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.tracks.length != widget.tracks.length ||
        oldWidget.hasMore != widget.hasMore ||
        oldWidget.loading != widget.loading ||
        oldWidget.loadingMore != widget.loadingMore ||
        oldWidget.filterActive != widget.filterActive) {
      _schedulePrefetchCheck();
    }
  }

  @override
  void dispose() {
    _settleTimer?.cancel();
    _scrollController.removeListener(_onScroll);
    _scrollController.dispose();
    super.dispose();
  }

  bool get _canLoadMore {
    return !widget.filterActive &&
        widget.hasMore &&
        !widget.loading &&
        !widget.loadingMore &&
        _pendingLoadMore == null;
  }

  void _onScroll() {
    if (!_scrollController.hasClients || !_canLoadMore) return;
    if (_scrollController.position.extentAfter <= _loadMoreThreshold) {
      _triggerLoadMore();
    }
  }

  void _schedulePrefetchCheck() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scrollController.hasClients || !_canLoadMore) return;
      if (_scrollController.position.maxScrollExtent <= 0) {
        _triggerLoadMore();
      }
    });
  }

  void _triggerLoadMore() {
    if (!_canLoadMore) return;
    final pending = widget.onLoadMore();
    _pendingLoadMore = pending;
    pending.whenComplete(() {
      if (!mounted || !identical(_pendingLoadMore, pending)) return;
      _pendingLoadMore = null;
      _schedulePrefetchCheck();
    });
  }

  bool _onScrollNotification(ScrollNotification n) {
    final nowMicros = DateTime.now().microsecondsSinceEpoch;
    final pixels = n.metrics.pixels;
    final dtMicros = _lastMicros == 0 ? 0 : (nowMicros - _lastMicros);
    final deltaPx = (pixels - _lastPixels).abs();
    final dtMs = dtMicros / 1000.0;
    final speed = dtMs <= 0 ? 0.0 : (deltaPx / dtMs); // px/ms
    _lastMicros = nowMicros;
    _lastPixels = pixels;

    final viewport = n.metrics.viewportDimension;
    final isFast = deltaPx > viewport * 0.60 || speed > 5.0; // ~5000 px/s
    if (isFast && !_deferHeavy) {
      setState(() => _deferHeavy = true);
    }

    _settleTimer?.cancel();
    _settleTimer = Timer(const Duration(milliseconds: 160), () {
      if (!mounted || !_deferHeavy) return;
      setState(() => _deferHeavy = false);
    });
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        StellatuneSearchField(
          controller: widget.searchController,
          onChanged: widget.onSearchChanged,
        ),
        const SizedBox(height: 12),
        QueueSourceInfoCard(queueSourceLabel: widget.queueSourceLabel),
        const SizedBox(height: 12),
        const SizedBox(height: 6),
        Expanded(
          child: widget.loading
              ? const Center(child: CircularProgressIndicator())
              : (widget.error != null && widget.error!.trim().isNotEmpty)
              ? Center(
                  child: Text(
                    widget.error!,
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: theme.colorScheme.error,
                    ),
                  ),
                )
              : widget.tracks.isEmpty
              ? Center(child: Text(l10n.noResultsHint))
              : Stack(
                  children: [
                    NotificationListener<ScrollNotification>(
                      onNotification: _onScrollNotification,
                      child: ListView.builder(
                        controller: _scrollController,
                        itemExtent: _itemExtent,
                        scrollCacheExtent: ScrollCacheExtent.pixels(
                          _deferHeavy ? 180 : 760,
                        ),
                        itemCount: widget.tracks.length,
                        itemBuilder: (context, index) {
                          final item = widget.tracks[index];
                          final trackKey = item.stableTrackKey;
                          final title = item.title?.trim().isNotEmpty == true
                              ? item.title!.trim()
                              : item.track.trackId;
                          final artist = item.artist?.trim() ?? '';
                          final album = item.album?.trim() ?? '';
                          final subtitle = artist.isEmpty
                              ? album
                              : (album.isEmpty ? artist : '$artist · $album');
                          final rowBg = theme
                              .colorScheme
                              .surfaceContainerHighest
                              .withValues(alpha: 0.28);
                          final pressed = _pressedTrackKey == trackKey;
                          return Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 2,
                              vertical: 4,
                            ),
                            child: Listener(
                              onPointerDown: (_) {
                                if (_pressedTrackKey == trackKey) return;
                                setState(() => _pressedTrackKey = trackKey);
                              },
                              onPointerUp: (_) {
                                if (_pressedTrackKey != trackKey) return;
                                setState(() => _pressedTrackKey = null);
                              },
                              onPointerCancel: (_) {
                                if (_pressedTrackKey != trackKey) return;
                                setState(() => _pressedTrackKey = null);
                              },
                              child: AnimatedScale(
                                duration: const Duration(milliseconds: 90),
                                curve: Curves.easeOutCubic,
                                scale: pressed ? 0.995 : 1.0,
                                child: AnimatedContainer(
                                  duration: const Duration(milliseconds: 180),
                                  curve: Curves.easeOutCubic,
                                  decoration: BoxDecoration(
                                    color: rowBg,
                                    borderRadius: BorderRadius.circular(14),
                                  ),
                                  child: Material(
                                    type: MaterialType.transparency,
                                    shape: RoundedRectangleBorder(
                                      borderRadius: BorderRadius.circular(14),
                                    ),
                                    clipBehavior: Clip.antiAlias,
                                    child: ListTile(
                                      dense: true,
                                      hoverColor: theme
                                          .colorScheme
                                          .surfaceContainerHighest
                                          .withValues(alpha: 0.42),
                                      shape: RoundedRectangleBorder(
                                        borderRadius: BorderRadius.circular(14),
                                      ),
                                      contentPadding:
                                          const EdgeInsets.symmetric(
                                            horizontal: 12,
                                          ),
                                      leading: _PluginTrackCover(
                                        cover: item.cover,
                                        deferHeavy: _deferHeavy,
                                      ),
                                      title: Text(
                                        title,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                      subtitle: _deferHeavy || subtitle.isEmpty
                                          ? null
                                          : Text(
                                              subtitle,
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                            ),
                                      onTap: () => widget.onActivate(
                                        index,
                                        widget.tracks,
                                      ),
                                      trailing: _deferHeavy
                                          ? const SizedBox(width: 72)
                                          : Row(
                                              mainAxisSize: MainAxisSize.min,
                                              children: [
                                                _PluginDurationText(
                                                  ms: item.durationMs?.toInt(),
                                                ),
                                                const SizedBox(width: 4),
                                                IconButton(
                                                  tooltip: l10n.menuEnqueue,
                                                  onPressed: () =>
                                                      widget.onEnqueue(item),
                                                  icon: const Icon(
                                                    Icons.add_to_queue_outlined,
                                                  ),
                                                ),
                                              ],
                                            ),
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          );
                        },
                      ),
                    ),
                    if (widget.loadingMore)
                      Positioned(
                        left: 0,
                        right: 0,
                        bottom: 8,
                        child: Center(
                          child: Container(
                            padding: const EdgeInsets.all(8),
                            decoration: BoxDecoration(
                              color: theme.colorScheme.surface.withValues(
                                alpha: 0.92,
                              ),
                              borderRadius: BorderRadius.circular(999),
                              border: Border.all(
                                color: theme.colorScheme.onSurface.withValues(
                                  alpha: 0.10,
                                ),
                              ),
                            ),
                            child: const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
        ),
      ],
    );
  }
}

class _PluginTrackCover extends StatefulWidget {
  const _PluginTrackCover({this.cover, required this.deferHeavy});

  final QueueCover? cover;
  final bool deferHeavy;

  @override
  State<_PluginTrackCover> createState() => _PluginTrackCoverState();
}

class _PluginTrackCoverState extends State<_PluginTrackCover> {
  static const Duration _lazyDelay = Duration(milliseconds: 90);
  Timer? _timer;
  bool _showCover = false;
  String? _coverToken;

  @override
  void initState() {
    super.initState();
    if (widget.deferHeavy) {
      _showCover = false;
      return;
    }
    _refreshCoverVisibility(initial: true);
  }

  @override
  void didUpdateWidget(covariant _PluginTrackCover oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.deferHeavy) {
      _timer?.cancel();
      if (_showCover) {
        setState(() => _showCover = false);
      }
      return;
    }
    if (oldWidget.deferHeavy && !widget.deferHeavy) {
      _refreshCoverVisibility();
      return;
    }
    if (_coverIdentity(oldWidget.cover) != _coverIdentity(widget.cover)) {
      _refreshCoverVisibility();
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  String? _coverIdentity(QueueCover? cover) {
    if (cover == null) return null;
    return '${cover.kind.name}:${cover.value}';
  }

  void _refreshCoverVisibility({bool initial = false}) {
    _timer?.cancel();
    final token = _coverIdentity(widget.cover);
    _coverToken = token;
    if (token == null) {
      if (initial) {
        _showCover = false;
      } else {
        setState(() => _showCover = false);
      }
      return;
    }
    if (initial) {
      _showCover = false;
    } else {
      setState(() => _showCover = false);
    }
    _timer = Timer(_lazyDelay, () {
      if (!mounted || _coverToken != token) return;
      setState(() => _showCover = true);
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final placeholder = Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        color: theme.colorScheme.primary.withValues(alpha: 0.12),
        border: Border.all(
          color: theme.colorScheme.primary.withValues(alpha: 0.22),
        ),
      ),
      child: Icon(
        Icons.music_note_rounded,
        size: 18,
        color: theme.colorScheme.primary,
      ),
    );

    if (widget.deferHeavy || !_showCover) return placeholder;
    return QueueCoverImage(
      cover: widget.cover,
      placeholder: placeholder,
      size: 40,
    );
  }
}

class _PluginDurationText extends StatelessWidget {
  const _PluginDurationText({required this.ms});

  final int? ms;

  @override
  Widget build(BuildContext context) {
    final value = ms;
    if (value == null || value <= 0) {
      return const SizedBox(width: 40);
    }
    final totalSeconds = (value / 1000).floor();
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    final text =
        '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
    return SizedBox(
      width: 40,
      child: Text(
        text,
        textAlign: TextAlign.right,
        style: Theme.of(context).textTheme.bodySmall,
      ),
    );
  }
}
