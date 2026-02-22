import 'dart:async';
import 'dart:io';

import 'package:animations/animations.dart';
import 'package:flutter/gestures.dart';
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
    this.blockedReasonByTrackId = const <int, String>{},
    this.onViewportRangeChanged,
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
  final Map<int, String> blockedReasonByTrackId;
  final void Function(int startIndex, int endIndex)? onViewportRangeChanged;

  @override
  State<TrackList> createState() => _TrackListState();
}

class _TrackListState extends State<TrackList> {
  static const _rowAnimDuration = Duration(milliseconds: 220);
  static const _rowAnimCurve = Cubic(0.22, 1.0, 0.36, 1.0);
  static const _itemExtent = 72.0;

  final ScrollController _controller = ScrollController();
  final GlobalKey _listViewportKey = GlobalKey();
  Timer? _settleTimer;

  bool _deferHeavy = false;
  double _lastPixels = 0.0;
  int _lastMicros = 0;
  int _fastScrollTickStreak = 0;
  double _desktopWheelBurstPx = 0.0;
  int _desktopWheelBurstTicks = 0;
  int _desktopWheelBurstStartMicros = 0;
  int _desktopWheelBurstLastMicros = 0;
  final Set<int> _selectedTrackIds = <int>{};
  int? _hoveredTrackId;
  int? _pressedTrackId;
  int _lastViewportStart = -1;
  int _lastViewportEnd = -1;
  bool _globalPointerRouteAttached = false;
  bool _desktopTrackMenuVisible = false;
  _PendingTrackMenuRequest? _pendingTrackMenuRequest;
  bool get _isDesktopPlatform =>
      Platform.isWindows || Platform.isLinux || Platform.isMacOS;

  @override
  void didUpdateWidget(covariant TrackList oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.items, widget.items)) {
      _lastViewportStart = -1;
      _lastViewportEnd = -1;
    }
  }

  @override
  void dispose() {
    _detachGlobalPointerRoute();
    _settleTimer?.cancel();
    _controller.dispose();
    super.dispose();
  }

  void _attachGlobalPointerRoute() {
    if (_globalPointerRouteAttached) return;
    GestureBinding.instance.pointerRouter.addGlobalRoute(
      _handleGlobalPointerEvent,
    );
    _globalPointerRouteAttached = true;
  }

  void _detachGlobalPointerRoute() {
    if (!_globalPointerRouteAttached) return;
    GestureBinding.instance.pointerRouter.removeGlobalRoute(
      _handleGlobalPointerEvent,
    );
    _globalPointerRouteAttached = false;
  }

  void _handleGlobalPointerEvent(PointerEvent event) {
    if (!_isDesktopPlatform || !_desktopTrackMenuVisible) return;
    if (event is! PointerDownEvent || event.buttons != kSecondaryMouseButton) {
      return;
    }
    final request = _resolveTrackMenuRequest(event.position);
    if (request == null) return;
    _pendingTrackMenuRequest = request;
    Navigator.of(context).maybePop();
  }

  _PendingTrackMenuRequest? _resolveTrackMenuRequest(Offset globalPosition) {
    if (!_controller.hasClients) {
      return null;
    }
    final render = _listViewportKey.currentContext?.findRenderObject();
    if (render is! RenderBox) {
      return null;
    }
    final local = render.globalToLocal(globalPosition);
    if (local.dx < 0 ||
        local.dy < 0 ||
        local.dx > render.size.width ||
        local.dy > render.size.height) {
      return null;
    }
    final scrollOffset = _controller.position.pixels;
    final index = ((scrollOffset + local.dy) / _itemExtent).floor();
    if (index < 0 || index >= widget.items.length) {
      return null;
    }
    final track = widget.items[index];
    final isBlocked = widget.blockedReasonByTrackId.containsKey(
      track.id.toInt(),
    );
    return _PendingTrackMenuRequest(
      globalPosition: globalPosition,
      index: index,
      track: track,
      isBlocked: isBlocked,
    );
  }

  void _emitViewportRange(ScrollMetrics metrics) {
    final onViewportRangeChanged = widget.onViewportRangeChanged;
    if (onViewportRangeChanged == null || widget.items.isEmpty) return;
    final maxIndex = widget.items.length - 1;
    var start = (metrics.pixels / _itemExtent).floor();
    var end =
        ((metrics.pixels + metrics.viewportDimension) / _itemExtent).ceil() - 1;
    start = start.clamp(0, maxIndex).toInt();
    end = end.clamp(0, maxIndex).toInt();
    if (end < start) {
      end = start;
    }
    if (start == _lastViewportStart && end == _lastViewportEnd) {
      return;
    }
    _lastViewportStart = start;
    _lastViewportEnd = end;
    onViewportRangeChanged(start, end);
  }

  bool _onScrollNotification(ScrollNotification n) {
    _emitViewportRange(n.metrics);
    if (n is! ScrollUpdateNotification) {
      if (n is ScrollEndNotification) {
        // Do not clear defer state immediately on end events. Some desktop wheel
        // devices emit update/end in quick pairs, and immediate reset makes the
        // placeholder practically invisible. Let settle timer handle exit.
      }
      return false;
    }

    final nowMicros = DateTime.now().microsecondsSinceEpoch;
    final pixels = n.metrics.pixels;
    final deltaPx = n.scrollDelta?.abs() ?? (pixels - _lastPixels).abs();
    final dtMicros = _lastMicros == 0 ? 0 : (nowMicros - _lastMicros);
    final dtMs = dtMicros / 1000.0;
    final speed = (dtMs <= 0) ? 0.0 : (deltaPx / dtMs); // px/ms

    _lastMicros = nowMicros;
    _lastPixels = pixels;

    // Treat big jumps (typical when dragging the scrollbar thumb) as "fast scrolling"
    // and temporarily render lighter rows. This keeps scroll thumb tracking snappy,
    // while content fills in shortly after the user stops.
    final viewport = n.metrics.viewportDimension;
    final isDesktopUpdate = _isDesktopPlatform;
    final isBigJump = deltaPx > viewport * 0.85;
    final isFastTick = deltaPx > viewport * 0.45 || speed > 6.5;
    final isVeryFast = speed > 8.0;

    if (isDesktopUpdate) {
      final sinceLastWheelMs = _desktopWheelBurstLastMicros == 0
          ? 0.0
          : (nowMicros - _desktopWheelBurstLastMicros) / 1000.0;
      if (sinceLastWheelMs > 180 || _desktopWheelBurstTicks == 0) {
        _desktopWheelBurstPx = 0;
        _desktopWheelBurstTicks = 0;
        _desktopWheelBurstStartMicros = nowMicros;
      }
      _desktopWheelBurstLastMicros = nowMicros;
      _desktopWheelBurstPx += deltaPx;
      _desktopWheelBurstTicks += 1;

      final burstDurationMs = _desktopWheelBurstStartMicros == 0
          ? 0.0
          : (nowMicros - _desktopWheelBurstStartMicros) / 1000.0;
      final isDesktopVeryFastTick =
          deltaPx > _itemExtent * 10.0 ||
          (deltaPx > _itemExtent * 6.0 && speed > 5.2);
      final isDesktopBurstFast =
          (burstDurationMs <= 220 &&
              _desktopWheelBurstTicks >= 2 &&
              _desktopWheelBurstPx > _itemExtent * 12.0) ||
          (burstDurationMs <= 420 &&
              _desktopWheelBurstTicks >= 3 &&
              _desktopWheelBurstPx > _itemExtent * 10.0);

      if (isDesktopVeryFastTick || isDesktopBurstFast) {
        _fastScrollTickStreak = 2;
      } else if (_fastScrollTickStreak > 0) {
        _fastScrollTickStreak -= 1;
      }
    } else if (isBigJump || isVeryFast) {
      _desktopWheelBurstPx = 0;
      _desktopWheelBurstTicks = 0;
      _desktopWheelBurstStartMicros = 0;
      _desktopWheelBurstLastMicros = 0;
      _fastScrollTickStreak = 2;
    } else if (isFastTick) {
      _desktopWheelBurstPx = 0;
      _desktopWheelBurstTicks = 0;
      _desktopWheelBurstStartMicros = 0;
      _desktopWheelBurstLastMicros = 0;
      _fastScrollTickStreak += 1;
    } else if (_fastScrollTickStreak > 0) {
      _desktopWheelBurstPx = 0;
      _desktopWheelBurstTicks = 0;
      _desktopWheelBurstStartMicros = 0;
      _desktopWheelBurstLastMicros = 0;
      _fastScrollTickStreak -= 1;
    }

    final shouldDeferHeavy = isDesktopUpdate
        ? _fastScrollTickStreak >= 2
        : _fastScrollTickStreak >= 2;
    if (shouldDeferHeavy && !_deferHeavy) {
      setState(() => _deferHeavy = true);
    }

    _settleTimer?.cancel();
    final settleDelay = isDesktopUpdate
        ? const Duration(milliseconds: 240)
        : const Duration(milliseconds: 140);
    _settleTimer = Timer(settleDelay, () {
      if (!mounted) return;
      _fastScrollTickStreak = 0;
      _desktopWheelBurstPx = 0;
      _desktopWheelBurstTicks = 0;
      _desktopWheelBurstStartMicros = 0;
      _desktopWheelBurstLastMicros = 0;
      if (_deferHeavy) {
        setState(() => _deferHeavy = false);
      }
    });

    return false;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (widget.items.isEmpty) {
      return Center(child: Text(l10n.noResultsHint));
    }

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      if (_controller.hasClients) {
        _emitViewportRange(_controller.position);
        return;
      }
      if (widget.items.isEmpty) return;
      if (_lastViewportStart == 0 && _lastViewportEnd >= 0) return;
      final fallbackEnd = (widget.items.length - 1).clamp(0, 11).toInt();
      _lastViewportStart = 0;
      _lastViewportEnd = fallbackEnd;
      widget.onViewportRangeChanged?.call(0, fallbackEnd);
    });

    final isSelectionMode = _selectedTrackIds.isNotEmpty;
    final canReorderCurrentPlaylist =
        widget.currentPlaylistId != null &&
        widget.onMoveInCurrentPlaylist != null &&
        !isSelectionMode;
    final body = canReorderCurrentPlaylist
        ? NotificationListener<ScrollNotification>(
            onNotification: _onScrollNotification,
            child: ReorderableListView.builder(
              scrollController: _controller,
              buildDefaultDragHandles: false,
              itemCount: widget.items.length,
              itemExtent: _itemExtent,
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
                    deferHeavy: _deferHeavy,
                    selectionMode: false,
                  ),
                );
              },
            ),
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
                  itemExtent: _itemExtent,
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

    final listBody = KeyedSubtree(key: _listViewportKey, child: body);
    if (!isSelectionMode) {
      return listBody;
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
        Expanded(child: listBody),
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
    final blockedReason = widget.blockedReasonByTrackId[trackId];
    final isBlocked = blockedReason != null;
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
              child: GestureDetector(
                onSecondaryTapDown: !_isDesktopPlatform || deferHeavy
                    ? null
                    : (details) => _showTrackActionMenu(
                        globalPosition: details.globalPosition,
                        index: i,
                        track: t,
                        isBlocked: isBlocked,
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
                    style: isBlocked
                        ? theme.textTheme.bodyLarge?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          )
                        : null,
                  ),
                  subtitle: deferHeavy
                      ? const _SubtitlePlaceholder()
                      : Row(
                          children: [
                            AudioFormatBadge(path: t.path),
                            Expanded(
                              child: Text(
                                line2.isNotEmpty ? line2 : t.path,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: isBlocked
                                    ? theme.textTheme.bodyMedium?.copyWith(
                                        color:
                                            theme.colorScheme.onSurfaceVariant,
                                      )
                                    : null,
                              ),
                            ),
                          ],
                        ),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (isBlocked)
                        Padding(
                          padding: const EdgeInsets.only(right: 4),
                          child: Tooltip(
                            message: blockedReason,
                            child: Icon(
                              Icons.block,
                              size: 18,
                              color: theme.colorScheme.error,
                            ),
                          ),
                        ),
                      if (selectionMode)
                        Checkbox(
                          value: selected,
                          onChanged: (_) => _toggleSelected(trackId),
                        ),
                      if (deferHeavy && !selectionMode) ...[
                        const _TrackTrailingPlaceholder(),
                      ] else ...[
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
                        if (!_isDesktopPlatform) ...[
                          const SizedBox(width: 8),
                          PopupMenuButton<_TrackAction>(
                            onSelected: (action) => _handleTrackAction(
                              context: context,
                              action: action,
                              index: i,
                              track: t,
                              isBlocked: isBlocked,
                            ),
                            itemBuilder: (context) =>
                                _buildTrackActionMenuItems(context, isBlocked),
                          ),
                        ],
                      ],
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
                    if (isBlocked) return;
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
      ),
    );
  }

  List<_TrackActionSpec> _buildTrackActionSpecs(
    BuildContext context,
    bool isBlocked,
  ) {
    final l10n = AppLocalizations.of(context)!;
    return <_TrackActionSpec>[
      _TrackActionSpec(
        action: _TrackAction.play,
        label: l10n.menuPlay,
        icon: Icons.play_arrow_rounded,
        enabled: !isBlocked,
      ),
      _TrackActionSpec(
        action: _TrackAction.enqueue,
        label: l10n.menuEnqueue,
        icon: Icons.queue_music_rounded,
        enabled: !isBlocked,
      ),
      _TrackActionSpec(
        action: _TrackAction.addToPlaylist,
        label: l10n.menuAddToPlaylist,
        icon: Icons.playlist_add_rounded,
      ),
      if (widget.currentPlaylistId != null)
        _TrackActionSpec(
          action: _TrackAction.removeFromCurrentPlaylist,
          label: l10n.menuRemoveFromCurrentPlaylist,
          icon: Icons.remove_circle_outline_rounded,
          showDividerBefore: true,
        ),
    ];
  }

  List<PopupMenuEntry<_TrackAction>> _buildTrackActionMenuItems(
    BuildContext context,
    bool isBlocked,
  ) {
    final items = _buildTrackActionSpecs(context, isBlocked);
    return items
        .map((item) {
          return PopupMenuItem<_TrackAction>(
            value: item.action,
            enabled: item.enabled,
            child: Row(
              children: [
                Icon(item.icon, size: 18),
                const SizedBox(width: 10),
                Expanded(child: Text(item.label)),
              ],
            ),
          );
        })
        .toList(growable: false);
  }

  Future<_TrackAction?> _showAnimatedTrackActionMenu({
    required Offset globalPosition,
    required bool isBlocked,
  }) {
    final context = this.context;
    final render = Overlay.of(context).context.findRenderObject();
    if (render is! RenderBox) {
      return Future<_TrackAction?>.value(null);
    }
    final menuItems = _buildTrackActionSpecs(context, isBlocked);
    const menuWidth = 250.0;
    const safePadding = 8.0;
    const rowHeight = 42.0;
    final dividerCount = menuItems
        .where((item) => item.showDividerBefore)
        .length;
    final menuHeight = menuItems.length * rowHeight + dividerCount * 9 + 14;
    final left = globalPosition.dx.clamp(
      safePadding,
      render.size.width - menuWidth - safePadding,
    );
    final top = globalPosition.dy.clamp(
      safePadding,
      render.size.height - menuHeight - safePadding,
    );

    return showGeneralDialog<_TrackAction>(
      context: context,
      barrierDismissible: true,
      barrierLabel: MaterialLocalizations.of(context).modalBarrierDismissLabel,
      barrierColor: Colors.transparent,
      transitionDuration: const Duration(milliseconds: 160),
      pageBuilder: (dialogContext, animation, secondaryAnimation) {
        final menuAnimation = CurvedAnimation(
          parent: animation,
          curve: Curves.easeOutCubic,
          reverseCurve: Curves.easeInCubic,
        );
        return Stack(
          children: [
            Positioned.fill(
              child: GestureDetector(
                behavior: HitTestBehavior.translucent,
                onTap: () => Navigator.of(dialogContext).pop(),
                onSecondaryTapDown: (_) => Navigator.of(dialogContext).pop(),
              ),
            ),
            Positioned(
              left: left.toDouble(),
              top: top.toDouble(),
              width: menuWidth,
              child: _TrackContextMenuCard(
                animation: menuAnimation,
                items: menuItems,
                onSelected: (action) => Navigator.of(dialogContext).pop(action),
              ),
            ),
          ],
        );
      },
    );
  }

  Future<void> _showTrackActionMenu({
    required Offset globalPosition,
    required int index,
    required TrackLite track,
    required bool isBlocked,
  }) async {
    if (!mounted) return;
    _desktopTrackMenuVisible = true;
    _attachGlobalPointerRoute();
    final action = await _showAnimatedTrackActionMenu(
      globalPosition: globalPosition,
      isBlocked: isBlocked,
    );
    _desktopTrackMenuVisible = false;
    _detachGlobalPointerRoute();
    final pendingRequest = _pendingTrackMenuRequest;
    _pendingTrackMenuRequest = null;
    if (pendingRequest != null) {
      if (!mounted) return;
      unawaited(
        _showTrackActionMenu(
          globalPosition: pendingRequest.globalPosition,
          index: pendingRequest.index,
          track: pendingRequest.track,
          isBlocked: pendingRequest.isBlocked,
        ),
      );
      return;
    }
    if (action == null) return;
    if (!mounted) return;
    await _handleTrackAction(
      context: context,
      action: action,
      index: index,
      track: track,
      isBlocked: isBlocked,
    );
  }

  Future<void> _handleTrackAction({
    required BuildContext context,
    required _TrackAction action,
    required int index,
    required TrackLite track,
    required bool isBlocked,
  }) async {
    if (action == _TrackAction.enqueue) {
      if (isBlocked) return;
      await widget.onEnqueue(track);
      return;
    }
    if (action == _TrackAction.play) {
      if (isBlocked) return;
      await widget.onActivate(index, widget.items);
      return;
    }
    if (action == _TrackAction.addToPlaylist) {
      final playlistId = await _pickPlaylistId(context);
      if (playlistId != null) {
        await widget.onAddToPlaylist(track, playlistId);
      }
      return;
    }
    final playlistId = widget.currentPlaylistId;
    if (playlistId != null) {
      await widget.onRemoveFromPlaylist(track, playlistId);
    }
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

class _TrackActionSpec {
  const _TrackActionSpec({
    required this.action,
    required this.label,
    required this.icon,
    this.enabled = true,
    this.showDividerBefore = false,
  });

  final _TrackAction action;
  final String label;
  final IconData icon;
  final bool enabled;
  final bool showDividerBefore;
}

class _TrackContextMenuCard extends StatelessWidget {
  const _TrackContextMenuCard({
    required this.animation,
    required this.items,
    required this.onSelected,
  });

  final Animation<double> animation;
  final List<_TrackActionSpec> items;
  final ValueChanged<_TrackAction> onSelected;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final iconColor = theme.colorScheme.onSurfaceVariant;
    final menuColor = Color.alphaBlend(
      theme.colorScheme.primary.withValues(alpha: 0.025),
      theme.colorScheme.surface,
    );
    final enabledTextStyle = theme.textTheme.bodyMedium;
    final disabledTextStyle = enabledTextStyle?.copyWith(
      color: theme.colorScheme.onSurface.withValues(alpha: 0.42),
    );
    final menuAnimation = animation;
    final baseMenu = Material(
      elevation: 14,
      shadowColor: Colors.black.withValues(alpha: 0.18),
      color: menuColor,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(
          color: theme.colorScheme.outlineVariant.withValues(alpha: 0.42),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final item in items) ...[
              if (item.showDividerBefore)
                Divider(
                  height: 9,
                  thickness: 0.8,
                  color: theme.colorScheme.outlineVariant.withValues(
                    alpha: 0.60,
                  ),
                ),
              InkWell(
                onTap: item.enabled ? () => onSelected(item.action) : null,
                hoverColor: theme.colorScheme.primary.withValues(alpha: 0.08),
                highlightColor: theme.colorScheme.primary.withValues(
                  alpha: 0.12,
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 10,
                  ),
                  child: Row(
                    children: [
                      Icon(
                        item.icon,
                        size: 19,
                        color: item.enabled
                            ? iconColor
                            : iconColor.withValues(alpha: 0.36),
                      ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Text(
                          item.label,
                          style: item.enabled
                              ? enabledTextStyle
                              : disabledTextStyle,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
    );

    return FadeScaleTransition(
      animation: menuAnimation,
      child: AnimatedBuilder(
        animation: menuAnimation,
        child: baseMenu,
        builder: (context, child) {
          final dy = (1 - menuAnimation.value) * 8;
          return Transform.translate(offset: Offset(0, dy), child: child);
        },
      ),
    );
  }
}

class _PendingTrackMenuRequest {
  _PendingTrackMenuRequest({
    required this.globalPosition,
    required this.index,
    required this.track,
    required this.isBlocked,
  });

  final Offset globalPosition;
  final int index;
  final TrackLite track;
  final bool isBlocked;
}

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

class _SubtitlePlaceholder extends StatelessWidget {
  const _SubtitlePlaceholder();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      children: [
        Container(
          width: 46,
          height: 16,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(6),
            color: theme.colorScheme.surfaceContainerHighest,
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Container(
            height: 12,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(6),
              color: theme.colorScheme.surfaceContainerHighest,
            ),
          ),
        ),
      ],
    );
  }
}

class _TrackTrailingPlaceholder extends StatelessWidget {
  const _TrackTrailingPlaceholder();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final blockColor = theme.colorScheme.surfaceContainerHighest;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 18,
          height: 18,
          decoration: BoxDecoration(shape: BoxShape.circle, color: blockColor),
        ),
        const SizedBox(width: 10),
        Container(
          width: 36,
          height: 12,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(6),
            color: blockColor,
          ),
        ),
      ],
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
