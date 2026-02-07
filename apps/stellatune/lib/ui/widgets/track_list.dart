import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/audio_format_badge.dart';

class TrackList extends StatefulWidget {
  const TrackList({
    super.key,
    required this.coverDir,
    required this.items,
    required this.likedTrackIds,
    required this.playlists,
    required this.currentPlaylistId,
    required this.onActivate,
    required this.onEnqueue,
    required this.onSetLiked,
    required this.onAddToPlaylist,
    required this.onRemoveFromPlaylist,
    this.onMoveInCurrentPlaylist,
    this.onBatchAddToPlaylist,
    this.onBatchRemoveFromCurrentPlaylist,
  });

  final String coverDir;
  final List<TrackLite> items;
  final Set<int> likedTrackIds;
  final List<PlaylistLite> playlists;
  final int? currentPlaylistId;
  final Future<void> Function(int index, List<TrackLite> items) onActivate;
  final Future<void> Function(TrackLite track) onEnqueue;
  final Future<void> Function(TrackLite track, bool liked) onSetLiked;
  final Future<void> Function(TrackLite track, int playlistId) onAddToPlaylist;
  final Future<void> Function(TrackLite track, int playlistId)
  onRemoveFromPlaylist;
  final Future<void> Function(TrackLite track, int newIndex)?
  onMoveInCurrentPlaylist;
  final Future<void> Function(List<TrackLite> tracks, int playlistId)?
  onBatchAddToPlaylist;
  final Future<void> Function(List<TrackLite> tracks, int playlistId)?
  onBatchRemoveFromCurrentPlaylist;

  @override
  State<TrackList> createState() => _TrackListState();
}

class _TrackListState extends State<TrackList> {
  static const _rowAnimDuration = Duration(milliseconds: 220);
  static const _rowAnimCurve = Cubic(0.22, 1.0, 0.36, 1.0);

  final ScrollController _controller = ScrollController();
  Timer? _settleTimer;

  bool _deferHeavy = false;
  double _lastPixels = 0.0;
  int _lastMicros = 0;
  final Set<int> _selectedTrackIds = <int>{};
  int? _hoveredTrackId;
  int? _pressedTrackId;

  @override
  void dispose() {
    _settleTimer?.cancel();
    _controller.dispose();
    super.dispose();
  }

  bool _onScrollNotification(ScrollNotification n) {
    final nowMicros = DateTime.now().microsecondsSinceEpoch;
    final pixels = n.metrics.pixels;

    final dtMicros = _lastMicros == 0 ? 0 : (nowMicros - _lastMicros);
    final deltaPx = (pixels - _lastPixels).abs();
    final dtMs = dtMicros / 1000.0;
    final speed = (dtMs <= 0) ? 0.0 : (deltaPx / dtMs); // px/ms

    _lastMicros = nowMicros;
    _lastPixels = pixels;

    // Treat big jumps (typical when dragging the scrollbar thumb) as "fast scrolling"
    // and temporarily render lighter rows. This keeps scroll thumb tracking snappy,
    // while content fills in shortly after the user stops.
    final viewport = n.metrics.viewportDimension;
    final isFast = deltaPx > viewport * 0.6 || speed > 5.0; // ~5000 px/s

    if (isFast && !_deferHeavy) {
      setState(() => _deferHeavy = true);
    }

    _settleTimer?.cancel();
    _settleTimer = Timer(const Duration(milliseconds: 160), () {
      if (!mounted) return;
      if (_deferHeavy) setState(() => _deferHeavy = false);
    });

    return false;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (widget.items.isEmpty) {
      return Center(child: Text(l10n.noResultsHint));
    }

    final isSelectionMode = _selectedTrackIds.isNotEmpty;
    final canReorderCurrentPlaylist =
        widget.currentPlaylistId != null &&
        widget.onMoveInCurrentPlaylist != null &&
        !isSelectionMode;
    final body = canReorderCurrentPlaylist
        ? ReorderableListView.builder(
            buildDefaultDragHandles: false,
            itemCount: widget.items.length,
            itemExtent: 72,
            onReorder: (oldIndex, newIndex) async {
              if (newIndex > oldIndex) {
                newIndex -= 1;
              }
              if (oldIndex == newIndex) return;
              final t = widget.items[oldIndex];
              await widget.onMoveInCurrentPlaylist!(t, newIndex);
            },
            itemBuilder: (context, i) {
              final t = widget.items[i];
              return KeyedSubtree(
                key: ValueKey('playlist-track-${t.id}-$i'),
                child: _buildTrackTile(
                  context,
                  l10n,
                  i,
                  t,
                  reorderIndex: i,
                  deferHeavy: false,
                  selectionMode: false,
                ),
              );
            },
          )
        : NotificationListener<ScrollNotification>(
            onNotification: _onScrollNotification,
            child: CustomScrollView(
              controller: _controller,
              // Smaller cache while scrubbing a long list keeps rebuild work low; once settled, allow
              // more cache for normal wheel/trackpad scrolling.
              cacheExtent: _deferHeavy ? 200 : 800,
              slivers: [
                SliverFixedExtentList(
                  itemExtent: 72,
                  delegate: SliverChildBuilderDelegate((context, i) {
                    final t = widget.items[i];
                    return _buildTrackTile(
                      context,
                      l10n,
                      i,
                      t,
                      deferHeavy: _deferHeavy,
                      selectionMode: isSelectionMode,
                    );
                  }, childCount: widget.items.length),
                ),
              ],
            ),
          );

    if (!isSelectionMode) {
      return body;
    }

    return Column(
      children: [
        _SelectionBar(
          selectedCount: _selectedTrackIds.length,
          allCount: widget.items.length,
          onCancel: () => setState(() => _selectedTrackIds.clear()),
          onSelectAll: _selectedTrackIds.length == widget.items.length
              ? null
              : () => setState(() {
                  _selectedTrackIds
                    ..clear()
                    ..addAll(widget.items.map((t) => t.id.toInt()));
                }),
          onAddToPlaylist: _onBatchAddToPlaylist,
          onRemoveFromCurrentPlaylist: widget.currentPlaylistId == null
              ? null
              : _onBatchRemoveFromCurrentPlaylist,
        ),
        Expanded(child: body),
      ],
    );
  }

  Widget _buildTrackTile(
    BuildContext context,
    AppLocalizations l10n,
    int i,
    TrackLite t, {
    int? reorderIndex,
    required bool deferHeavy,
    required bool selectionMode,
  }) {
    final title = (t.title ?? '').trim();
    final artist = (t.artist ?? '').trim();
    final album = (t.album ?? '').trim();
    final isLiked = widget.likedTrackIds.contains(t.id.toInt());

    final line1 = title.isNotEmpty ? title : _basename(t.path);
    final line2 = [artist, album].where((s) => s.isNotEmpty).join(' • ');
    final coverPath = '${widget.coverDir}${Platform.pathSeparator}${t.id}';

    final trackId = t.id.toInt();
    final selected = _selectedTrackIds.contains(trackId);
    final theme = Theme.of(context);
    final hovered = _hoveredTrackId == trackId;
    final pressed = _pressedTrackId == trackId;
    final rowBg = selected
        ? theme.colorScheme.secondaryContainer.withValues(alpha: 0.52)
        : hovered
        ? theme.colorScheme.surfaceContainerHigh.withValues(alpha: 0.42)
        : Colors.transparent;
    final rowBorderColor = selected
        ? theme.colorScheme.secondary.withValues(alpha: 0.34)
        : theme.colorScheme.onSurface.withValues(alpha: hovered ? 0.12 : 0.08);
    final rowShadow = hovered
        ? [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.045),
              blurRadius: 8,
              offset: const Offset(0, 2),
            ),
          ]
        : const <BoxShadow>[];

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 1.5),
      child: MouseRegion(
        onEnter: (_) {
          if (_hoveredTrackId == trackId) return;
          setState(() => _hoveredTrackId = trackId);
        },
        onExit: (_) {
          if (_hoveredTrackId != trackId) return;
          setState(() => _hoveredTrackId = null);
        },
        child: Listener(
          onPointerDown: (_) {
            if (_pressedTrackId == trackId) return;
            setState(() => _pressedTrackId = trackId);
          },
          onPointerUp: (_) {
            if (_pressedTrackId != trackId) return;
            setState(() => _pressedTrackId = null);
          },
          onPointerCancel: (_) {
            if (_pressedTrackId != trackId) return;
            setState(() => _pressedTrackId = null);
          },
          child: AnimatedScale(
            duration: const Duration(milliseconds: 90),
            curve: Curves.easeOutCubic,
            scale: pressed ? 0.995 : 1.0,
            child: AnimatedContainer(
              duration: _rowAnimDuration,
              curve: _rowAnimCurve,
              decoration: BoxDecoration(
                color: rowBg,
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: rowBorderColor, width: 0.8),
                boxShadow: rowShadow,
              ),
              child: ListTile(
                dense: true,
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(12),
                ),
                leading: deferHeavy
                    ? const _CoverPlaceholder()
                    : _CoverThumb(path: coverPath),
                title: Text(
                  line1,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                subtitle: deferHeavy
                    ? null
                    : Row(
                        children: [
                          AudioFormatBadge(path: t.path),
                          Expanded(
                            child: Text(
                              line2.isNotEmpty ? line2 : t.path,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        ],
                      ),
                trailing: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (selectionMode)
                      Checkbox(
                        value: selected,
                        onChanged: (_) => _toggleSelected(trackId),
                      ),
                    IconButton(
                      tooltip: isLiked
                          ? l10n.likedRemoveTooltip
                          : l10n.likedAddTooltip,
                      onPressed: () => widget.onSetLiked(t, !isLiked),
                      icon: Icon(
                        isLiked ? Icons.favorite : Icons.favorite_border,
                        color: isLiked ? theme.colorScheme.error : null,
                      ),
                    ),
                    _DurationText(ms: t.durationMs?.toInt()),
                    const SizedBox(width: 8),
                    if (!deferHeavy)
                      PopupMenuButton<_TrackAction>(
                        onSelected: (action) async {
                          if (action == _TrackAction.enqueue) {
                            await widget.onEnqueue(t);
                            return;
                          }
                          if (action == _TrackAction.play) {
                            await widget.onActivate(i, widget.items);
                            return;
                          }
                          if (action == _TrackAction.addToPlaylist) {
                            final playlistId = await _pickPlaylistId(context);
                            if (playlistId != null) {
                              await widget.onAddToPlaylist(t, playlistId);
                            }
                            return;
                          }
                          final playlistId = widget.currentPlaylistId;
                          if (playlistId != null) {
                            await widget.onRemoveFromPlaylist(t, playlistId);
                          }
                        },
                        itemBuilder: (context) => [
                          PopupMenuItem(
                            value: _TrackAction.play,
                            child: Text(l10n.menuPlay),
                          ),
                          PopupMenuItem(
                            value: _TrackAction.enqueue,
                            child: Text(l10n.menuEnqueue),
                          ),
                          PopupMenuItem(
                            value: _TrackAction.addToPlaylist,
                            child: Text(l10n.menuAddToPlaylist),
                          ),
                          if (widget.currentPlaylistId != null)
                            PopupMenuItem(
                              value: _TrackAction.removeFromCurrentPlaylist,
                              child: Text(l10n.menuRemoveFromCurrentPlaylist),
                            ),
                        ],
                      ),
                    if (reorderIndex != null)
                      ReorderableDragStartListener(
                        index: reorderIndex,
                        child: const Padding(
                          padding: EdgeInsets.only(left: 4),
                          child: Icon(Icons.drag_handle),
                        ),
                      ),
                  ],
                ),
                onTap: () {
                  if (selectionMode) {
                    _toggleSelected(trackId);
                    return;
                  }
                  widget.onActivate(i, widget.items);
                },
                onLongPress: widget.currentPlaylistId == null
                    ? null
                    : () => _toggleSelected(trackId),
              ),
            ),
          ),
        ),
      ),
    );
  }

  void _toggleSelected(int trackId) {
    setState(() {
      if (!_selectedTrackIds.remove(trackId)) {
        _selectedTrackIds.add(trackId);
      }
    });
  }

  Future<void> _onBatchAddToPlaylist() async {
    final playlistId = await _pickPlaylistId(context);
    if (playlistId == null) return;
    final tracks = _selectedTracks();
    if (tracks.isEmpty) return;

    final handler = widget.onBatchAddToPlaylist;
    if (handler != null) {
      await handler(tracks, playlistId);
    } else {
      for (final t in tracks) {
        await widget.onAddToPlaylist(t, playlistId);
      }
    }
    if (!mounted) return;
    setState(() => _selectedTrackIds.clear());
  }

  Future<void> _onBatchRemoveFromCurrentPlaylist() async {
    final playlistId = widget.currentPlaylistId;
    if (playlistId == null) return;
    final tracks = _selectedTracks();
    if (tracks.isEmpty) return;

    final handler = widget.onBatchRemoveFromCurrentPlaylist;
    if (handler != null) {
      await handler(tracks, playlistId);
    } else {
      for (final t in tracks) {
        await widget.onRemoveFromPlaylist(t, playlistId);
      }
    }
    if (!mounted) return;
    setState(() => _selectedTrackIds.clear());
  }

  List<TrackLite> _selectedTracks() {
    if (_selectedTrackIds.isEmpty) return const <TrackLite>[];
    return widget.items
        .where((t) => _selectedTrackIds.contains(t.id.toInt()))
        .toList();
  }

  Future<int?> _pickPlaylistId(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return showModalBottomSheet<int>(
      context: context,
      showDragHandle: true,
      builder: (context) {
        final playlists = widget.playlists;
        if (playlists.isEmpty) {
          return SizedBox(
            height: 120,
            child: Center(child: Text(l10n.playlistEmptyHint)),
          );
        }
        return SafeArea(
          child: ListView.builder(
            itemCount: playlists.length,
            itemBuilder: (context, index) {
              final p = playlists[index];
              return ListTile(
                leading: Icon(
                  p.systemKey == 'liked' ? Icons.favorite : Icons.playlist_play,
                ),
                title: Text(_playlistDisplayName(l10n, p)),
                onTap: () => Navigator.of(context).pop(p.id.toInt()),
              );
            },
          ),
        );
      },
    );
  }

  String _playlistDisplayName(AppLocalizations l10n, PlaylistLite playlist) {
    if (playlist.systemKey == 'liked') {
      return l10n.likedPlaylistName;
    }
    return playlist.name;
  }

  static String _basename(String path) {
    final parts = path.split(RegExp(r'[\\/]+'));
    return parts.isEmpty ? path : parts.last;
  }
}

enum _TrackAction { play, enqueue, addToPlaylist, removeFromCurrentPlaylist }

class _SelectionBar extends StatelessWidget {
  const _SelectionBar({
    required this.selectedCount,
    required this.allCount,
    required this.onCancel,
    required this.onSelectAll,
    required this.onAddToPlaylist,
    required this.onRemoveFromCurrentPlaylist,
  });

  final int selectedCount;
  final int allCount;
  final VoidCallback onCancel;
  final VoidCallback? onSelectAll;
  final Future<void> Function() onAddToPlaylist;
  final Future<void> Function()? onRemoveFromCurrentPlaylist;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Material(
      color: Theme.of(context).colorScheme.surfaceContainerHigh,
      child: SizedBox(
        height: 52,
        child: Row(
          children: [
            const SizedBox(width: 8),
            Text(l10n.playlistSelectionCount(selectedCount)),
            const SizedBox(width: 8),
            TextButton(
              onPressed: onSelectAll,
              child: Text(
                selectedCount >= allCount
                    ? l10n.playlistAllSelected
                    : l10n.playlistSelectAll,
              ),
            ),
            const Spacer(),
            TextButton(
              onPressed: onAddToPlaylist,
              child: Text(l10n.playlistBatchAddToPlaylist),
            ),
            if (onRemoveFromCurrentPlaylist != null)
              TextButton(
                onPressed: onRemoveFromCurrentPlaylist,
                child: Text(l10n.playlistBatchRemoveFromCurrent),
              ),
            TextButton(onPressed: onCancel, child: Text(l10n.cancel)),
            const SizedBox(width: 4),
          ],
        ),
      ),
    );
  }
}

class _CoverPlaceholder extends StatelessWidget {
  const _CoverPlaceholder();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        color: theme.colorScheme.surfaceContainerHighest,
      ),
      child: Icon(Icons.music_note, color: theme.colorScheme.onSurfaceVariant),
    );
  }
}

class _CoverThumb extends StatelessWidget {
  const _CoverThumb({required this.path});

  final String path;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final placeholder = Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        color: theme.colorScheme.primary.withValues(alpha: 0.10),
        border: Border.all(
          color: theme.colorScheme.primary.withValues(alpha: 0.15),
        ),
      ),
      child: Icon(Icons.music_note, color: theme.colorScheme.primary),
    );

    final provider = ResizeImage(
      FileImage(File(path)),
      width: 80,
      height: 80,
      allowUpscaling: false,
    );

    return ClipRRect(
      borderRadius: BorderRadius.circular(8),
      child: Image(
        image: provider,
        width: 40,
        height: 40,
        fit: BoxFit.cover,
        filterQuality: FilterQuality.low,
        gaplessPlayback: true,
        errorBuilder: (context, error, stackTrace) => placeholder,
      ),
    );
  }
}

class _DurationText extends StatelessWidget {
  const _DurationText({required this.ms});

  final int? ms;

  @override
  Widget build(BuildContext context) {
    final v = ms;
    if (v == null || v <= 0) return const SizedBox.shrink();
    final totalSeconds = (v / 1000).floor();
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    return Text(
      '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}',
      style: Theme.of(context).textTheme.bodySmall,
    );
  }
}
