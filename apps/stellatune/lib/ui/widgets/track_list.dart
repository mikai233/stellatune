import 'dart:async';
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/track_list/models/track_list_models.dart';
import 'package:stellatune/ui/widgets/track_list/widgets/track_list_context_menu.dart';
import 'package:stellatune/ui/widgets/track_list/widgets/track_list_shared_widgets.dart';
import 'package:stellatune/ui/widgets/track_list/widgets/track_list_tile.dart';

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
  List<TrackLite>? _reorderViewItems;
  String? _pendingReorderFingerprint;
  final Set<int> _selectedTrackIds = <int>{};
  int? _pressedTrackId;
  int _lastViewportStart = -1;
  int _lastViewportEnd = -1;
  bool _globalPointerRouteAttached = false;
  bool _desktopTrackMenuVisible = false;
  PendingTrackMenuRequest? _pendingTrackMenuRequest;
  bool get _isDesktopPlatform =>
      Platform.isWindows || Platform.isLinux || Platform.isMacOS;

  @override
  void didUpdateWidget(covariant TrackList oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.items, widget.items)) {
      _lastViewportStart = -1;
      _lastViewportEnd = -1;
    }
    if (widget.currentPlaylistId == null ||
        widget.onMoveInCurrentPlaylist == null) {
      _reorderViewItems = null;
      _pendingReorderFingerprint = null;
      return;
    }
    final local = _reorderViewItems;
    if (local == null) {
      return;
    }
    final currentFp = _trackOrderFingerprint(widget.items);
    final localFp = _trackOrderFingerprint(local);
    final pendingFp = _pendingReorderFingerprint;
    if (pendingFp == null) {
      _reorderViewItems = null;
      return;
    }
    if (currentFp == pendingFp) {
      _reorderViewItems = null;
      _pendingReorderFingerprint = null;
      return;
    }
    if (currentFp != localFp) {
      _reorderViewItems = null;
      _pendingReorderFingerprint = null;
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

  PendingTrackMenuRequest? _resolveTrackMenuRequest(Offset globalPosition) {
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
    final items = _effectiveTrackItems();
    if (index >= items.length) {
      return null;
    }
    final track = items[index];
    final isBlocked = widget.blockedReasonByTrackId.containsKey(
      track.id.toInt(),
    );
    return PendingTrackMenuRequest(
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

  List<TrackLite> _effectiveTrackItems() => _reorderViewItems ?? widget.items;

  String _trackOrderFingerprint(List<TrackLite> items) {
    return items.map((t) => '${t.id}|${t.path}').join('\u0001');
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
    final reorderItems = canReorderCurrentPlaylist
        ? _effectiveTrackItems()
        : widget.items;
    final body = canReorderCurrentPlaylist
        ? NotificationListener<ScrollNotification>(
            onNotification: _onScrollNotification,
            child: ReorderableListView.builder(
              scrollController: _controller,
              buildDefaultDragHandles: false,
              itemCount: reorderItems.length,
              itemExtent: _itemExtent,
              onReorder: (oldIndex, newIndex) async {
                if (newIndex > oldIndex) {
                  newIndex -= 1;
                }
                if (oldIndex == newIndex) return;
                final next = List<TrackLite>.from(reorderItems);
                final moved = next.removeAt(oldIndex);
                next.insert(newIndex, moved);
                setState(() {
                  _reorderViewItems = next;
                  _pendingReorderFingerprint = _trackOrderFingerprint(next);
                });
                try {
                  await widget.onMoveInCurrentPlaylist!(moved, newIndex);
                } catch (_) {
                  if (!mounted) return;
                  setState(() {
                    _reorderViewItems = null;
                    _pendingReorderFingerprint = null;
                  });
                  rethrow;
                }
              },
              itemBuilder: (context, i) {
                final t = reorderItems[i];
                return KeyedSubtree(
                  key: ObjectKey(t),
                  child: _buildTrackTile(
                    context,
                    l10n,
                    i,
                    t,
                    activateItems: reorderItems,
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
                      activateItems: widget.items,
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
        TrackListSelectionBar(
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
    required List<TrackLite> activateItems,
    int? reorderIndex,
    required bool deferHeavy,
    required bool selectionMode,
  }) {
    final trackId = t.id.toInt();
    final blockedReason = widget.blockedReasonByTrackId[trackId];
    final isBlocked = blockedReason != null;
    final selected = _selectedTrackIds.contains(trackId);
    final pressed = _pressedTrackId == trackId;
    final isLiked = widget.likedTrackIds.contains(trackId);

    return TrackListTile(
      l10n: l10n,
      index: i,
      track: t,
      coverDir: widget.coverDir,
      deferHeavy: deferHeavy,
      selectionMode: selectionMode,
      selected: selected,
      pressed: pressed,
      isLiked: isLiked,
      isBlocked: isBlocked,
      isDesktopPlatform: _isDesktopPlatform,
      blockedReason: blockedReason,
      reorderIndex: reorderIndex,
      onPressedDown: () {
        if (_pressedTrackId == trackId) return;
        setState(() => _pressedTrackId = trackId);
      },
      onPressedUp: () {
        if (_pressedTrackId != trackId) return;
        setState(() => _pressedTrackId = null);
      },
      onPressedCancel: () {
        if (_pressedTrackId != trackId) return;
        setState(() => _pressedTrackId = null);
      },
      onToggleLike: () => widget.onSetLiked(t, !isLiked),
      onToggleSelected: () => _toggleSelected(trackId),
      onTapTrack: () {
        if (selectionMode) {
          _toggleSelected(trackId);
          return;
        }
        if (isBlocked) return;
        widget.onActivate(i, activateItems);
      },
      onTrackAction: (action) => _handleTrackAction(
        context: context,
        action: action,
        index: i,
        track: t,
        activateItems: activateItems,
        isBlocked: isBlocked,
      ),
      buildTrackActionMenuItems: (context) =>
          _buildTrackActionMenuItems(context, isBlocked),
      onDesktopContextMenuRequested: !_isDesktopPlatform
          ? null
          : (globalPosition) => _showTrackActionMenu(
              globalPosition: globalPosition,
              index: i,
              track: t,
              activateItems: activateItems,
              isBlocked: isBlocked,
            ),
      onLongPressSelect: widget.currentPlaylistId == null
          ? null
          : () => _toggleSelected(trackId),
    );
  }

  List<TrackListActionSpec> _buildTrackActionSpecs(
    BuildContext context,
    bool isBlocked,
  ) {
    final l10n = AppLocalizations.of(context)!;
    return <TrackListActionSpec>[
      TrackListActionSpec(
        action: TrackListAction.play,
        label: l10n.menuPlay,
        icon: Icons.play_arrow_rounded,
        enabled: !isBlocked,
      ),
      TrackListActionSpec(
        action: TrackListAction.enqueue,
        label: l10n.menuEnqueue,
        icon: Icons.queue_music_rounded,
        enabled: !isBlocked,
      ),
      TrackListActionSpec(
        action: TrackListAction.addToPlaylist,
        label: l10n.menuAddToPlaylist,
        icon: Icons.playlist_add_rounded,
      ),
      if (widget.currentPlaylistId != null)
        TrackListActionSpec(
          action: TrackListAction.removeFromCurrentPlaylist,
          label: l10n.menuRemoveFromCurrentPlaylist,
          icon: Icons.remove_circle_outline_rounded,
          showDividerBefore: true,
        ),
    ];
  }

  List<PopupMenuEntry<TrackListAction>> _buildTrackActionMenuItems(
    BuildContext context,
    bool isBlocked,
  ) {
    final items = _buildTrackActionSpecs(context, isBlocked);
    return buildTrackListActionMenuItems(items);
  }

  Future<TrackListAction?> _showAnimatedTrackActionMenu({
    required Offset globalPosition,
    required bool isBlocked,
  }) {
    final context = this.context;
    final render = Overlay.of(context).context.findRenderObject();
    if (render is! RenderBox) {
      return Future<TrackListAction?>.value(null);
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

    return showGeneralDialog<TrackListAction>(
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
              child: TrackListContextMenuCard(
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
    required List<TrackLite> activateItems,
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
          activateItems: _effectiveTrackItems(),
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
      activateItems: activateItems,
      isBlocked: isBlocked,
    );
  }

  Future<void> _handleTrackAction({
    required BuildContext context,
    required TrackListAction action,
    required int index,
    required TrackLite track,
    required List<TrackLite> activateItems,
    required bool isBlocked,
  }) async {
    if (action == TrackListAction.enqueue) {
      if (isBlocked) return;
      await widget.onEnqueue(track);
      return;
    }
    if (action == TrackListAction.play) {
      if (isBlocked) return;
      await widget.onActivate(index, activateItems);
      return;
    }
    if (action == TrackListAction.addToPlaylist) {
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
}
