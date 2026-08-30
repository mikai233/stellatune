import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;
import 'package:path/path.dart' as p;
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/forms/schema_form.dart';
import 'package:stellatune/ui/widgets/track_list/models/track_list_models.dart';
import 'package:stellatune/ui/widgets/track_list/widgets/track_list_context_menu.dart';
import 'package:stellatune/ui/widgets/track_list/widgets/track_list_shared_widgets.dart';
import 'package:stellatune/ui/widgets/track_list/widgets/track_list_tile.dart';

class TrackList extends StatefulWidget {
  const TrackList({
    super.key,
    required this.bridge,
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
    this.onTranscodeSelected,
    this.blockedReasonByTrackId = const <int, String>{},
    this.onViewportRangeChanged,
  });

  final PlayerBridge bridge;
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
  final Future<void> Function(TrackLite track, EncoderTypeDescriptor encoder)?
  onTranscodeSelected;
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
  List<EncoderTypeDescriptor>? _encoderTypesCache;
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
              onReorderItem: (oldIndex, newIndex) async {
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
              scrollCacheExtent: ScrollCacheExtent.pixels(
                _deferHeavy ? 200 : 800,
              ),
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
        action: TrackListAction.transcode,
        label: l10n.menuTranscode,
        icon: Icons.transform_rounded,
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
      action: action,
      index: index,
      track: track,
      activateItems: activateItems,
      isBlocked: isBlocked,
      menuGlobalPosition: globalPosition,
    );
  }

  Future<void> _handleTrackAction({
    required TrackListAction action,
    required int index,
    required TrackLite track,
    required List<TrackLite> activateItems,
    required bool isBlocked,
    Offset? menuGlobalPosition,
  }) async {
    final context = this.context;
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
    if (action == TrackListAction.transcode) {
      final encoder = await _pickEncoder(
        anchorGlobalPosition: menuGlobalPosition,
      );
      if (encoder == null) return;

      final onTranscodeSelected = widget.onTranscodeSelected;
      if (onTranscodeSelected != null) {
        await onTranscodeSelected(track, encoder);
        return;
      }
      final params = await _editTranscodeParams(encoder);
      if (params == null) return;
      await _runDefaultTranscodeFlow(
        track: track,
        encoder: encoder,
        encoderConfigJson: params.encoderConfigJson,
        encoderOptionsJson: params.encoderOptionsJson,
      );
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

  Future<List<EncoderTypeDescriptor>> _listEncoderTypes() async {
    final cached = _encoderTypesCache;
    if (cached != null) {
      return cached;
    }
    final loaded = await widget.bridge.encoderListTypes();
    loaded.sort((a, b) {
      final byDisplayName = a.displayName.toLowerCase().compareTo(
        b.displayName.toLowerCase(),
      );
      if (byDisplayName != 0) return byDisplayName;
      final byPluginName = a.pluginName.toLowerCase().compareTo(
        b.pluginName.toLowerCase(),
      );
      if (byPluginName != 0) return byPluginName;
      final byTypeId = a.typeId.compareTo(b.typeId);
      if (byTypeId != 0) return byTypeId;
      return a.pluginId.compareTo(b.pluginId);
    });
    _encoderTypesCache = List<EncoderTypeDescriptor>.unmodifiable(loaded);
    return _encoderTypesCache!;
  }

  String _encoderMenuSubtitle(EncoderTypeDescriptor encoder) {
    return '${encoder.pluginName} (${encoder.pluginId}) · ${encoder.typeId}';
  }

  Future<EncoderTypeDescriptor?> _pickEncoder({
    Offset? anchorGlobalPosition,
  }) async {
    final context = this.context;
    List<EncoderTypeDescriptor> encoders;
    try {
      encoders = await _listEncoderTypes();
    } catch (error) {
      if (!context.mounted) return null;
      final l10n = AppLocalizations.of(context)!;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('${l10n.transcodeLoadEncodersFailed}: $error')),
      );
      return null;
    }
    if (!context.mounted) return null;
    final l10n = AppLocalizations.of(context)!;
    if (encoders.isEmpty) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.transcodeNoEncoders)));
      return null;
    }
    if (_isDesktopPlatform && anchorGlobalPosition != null) {
      return _showEncoderPickerMenu(
        context: context,
        globalPosition: anchorGlobalPosition,
        encoders: encoders,
      );
    }
    return _showEncoderPickerBottomSheet(context: context, encoders: encoders);
  }

  Future<EncoderTypeDescriptor?> _showEncoderPickerMenu({
    required BuildContext context,
    required Offset globalPosition,
    required List<EncoderTypeDescriptor> encoders,
  }) {
    final overlay = Overlay.of(context).context.findRenderObject();
    if (overlay is! RenderBox) {
      return Future<EncoderTypeDescriptor?>.value(null);
    }
    final position = RelativeRect.fromLTRB(
      globalPosition.dx,
      globalPosition.dy,
      overlay.size.width - globalPosition.dx,
      overlay.size.height - globalPosition.dy,
    );
    final theme = Theme.of(context);
    final subtitleStyle = theme.textTheme.bodySmall?.copyWith(
      color: theme.colorScheme.onSurfaceVariant,
    );
    return showMenu<EncoderTypeDescriptor>(
      context: context,
      position: position,
      constraints: const BoxConstraints(maxWidth: 420, maxHeight: 460),
      items: encoders
          .map(
            (encoder) => PopupMenuItem<EncoderTypeDescriptor>(
              value: encoder,
              child: Row(
                children: [
                  const Icon(Icons.file_upload_outlined, size: 18),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          encoder.displayName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                        const SizedBox(height: 2),
                        Text(
                          _encoderMenuSubtitle(encoder),
                          style: subtitleStyle,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          )
          .toList(growable: false),
    );
  }

  Future<EncoderTypeDescriptor?> _showEncoderPickerBottomSheet({
    required BuildContext context,
    required List<EncoderTypeDescriptor> encoders,
  }) {
    final l10n = AppLocalizations.of(context)!;
    final estimatedHeight = (encoders.length * 64.0 + 104.0)
        .clamp(220.0, 460.0)
        .toDouble();
    return showModalBottomSheet<EncoderTypeDescriptor>(
      context: context,
      showDragHandle: true,
      builder: (context) {
        final theme = Theme.of(context);
        return SafeArea(
          child: SizedBox(
            height: estimatedHeight,
            child: Column(
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: Text(
                      l10n.transcodeSelectEncoderTitle,
                      style: theme.textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ),
                ),
                Expanded(
                  child: ListView.separated(
                    itemCount: encoders.length,
                    separatorBuilder: (_, _) => const Divider(height: 1),
                    itemBuilder: (context, index) {
                      final encoder = encoders[index];
                      return ListTile(
                        leading: const Icon(Icons.file_upload_outlined),
                        title: Text(encoder.displayName),
                        subtitle: Text(_encoderMenuSubtitle(encoder)),
                        onTap: () => Navigator.of(context).pop(encoder),
                      );
                    },
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }

  Future<_TranscodeLaunchParams?> _editTranscodeParams(
    EncoderTypeDescriptor encoder,
  ) async {
    final context = this.context;
    final l10n = AppLocalizations.of(context)!;
    var configDraft = _normalizeJsonString(
      encoder.defaultConfigJson,
      fallbackJson: '{}',
    );
    final optionsController = TextEditingController();
    String? errorText;
    final result = await showDialog<_TranscodeLaunchParams>(
      context: context,
      builder: (dialogContext) {
        return StatefulBuilder(
          builder: (dialogContext, setState) {
            final theme = Theme.of(dialogContext);
            return AlertDialog(
              title: Text(l10n.transcodeParamsDialogTitle),
              content: SizedBox(
                width: 620,
                child: SingleChildScrollView(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        encoder.displayName,
                        style: theme.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      const SizedBox(height: 4),
                      Text(
                        _encoderMenuSubtitle(encoder),
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                      const SizedBox(height: 14),
                      SchemaForm(
                        schemaJson: encoder.configSchemaJson,
                        initialValueJson: configDraft,
                        fallbackLabel: l10n.transcodeParamsConfigLabel,
                        onChangedJson: (json) {
                          configDraft = json;
                        },
                      ),
                      const SizedBox(height: 10),
                      TextField(
                        controller: optionsController,
                        minLines: 2,
                        maxLines: 6,
                        decoration: InputDecoration(
                          border: const OutlineInputBorder(),
                          labelText: l10n.transcodeParamsOptionsLabel,
                          helperText: l10n.transcodeParamsOptionsHint,
                        ),
                      ),
                      if (errorText != null) ...[
                        const SizedBox(height: 10),
                        Text(
                          errorText!,
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.error,
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(dialogContext).pop(),
                  child: Text(l10n.cancel),
                ),
                FilledButton(
                  onPressed: () {
                    try {
                      final normalizedConfig = _normalizeJsonString(
                        configDraft,
                        fallbackJson: '{}',
                      );
                      final normalizedOptions = _normalizeOptionalJsonString(
                        optionsController.text,
                      );
                      Navigator.of(dialogContext).pop(
                        _TranscodeLaunchParams(
                          encoderConfigJson: normalizedConfig,
                          encoderOptionsJson: normalizedOptions,
                        ),
                      );
                    } catch (_) {
                      setState(() {
                        errorText = l10n.transcodeParamsInvalidJson;
                      });
                    }
                  },
                  child: Text(l10n.transcodeParamsConfirm),
                ),
              ],
            );
          },
        );
      },
    );
    optionsController.dispose();
    return result;
  }

  String _normalizeJsonString(String raw, {required String fallbackJson}) {
    final trimmed = raw.trim();
    final source = trimmed.isEmpty ? fallbackJson : trimmed;
    final decoded = jsonDecode(source);
    return jsonEncode(decoded);
  }

  String? _normalizeOptionalJsonString(String raw) {
    final trimmed = raw.trim();
    if (trimmed.isEmpty) return null;
    final decoded = jsonDecode(trimmed);
    return jsonEncode(decoded);
  }

  Future<void> _runDefaultTranscodeFlow({
    required TrackLite track,
    required EncoderTypeDescriptor encoder,
    required String encoderConfigJson,
    required String? encoderOptionsJson,
  }) async {
    final context = this.context;
    final l10n = AppLocalizations.of(context)!;
    final sourcePath = track.path.trim();
    if (sourcePath.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(l10n.transcodeStartFailed('source path is empty')),
        ),
      );
      return;
    }

    String? outputPath;
    try {
      final location = await getSaveLocation(
        suggestedName: _buildDefaultTranscodeFileName(track, encoder),
      );
      outputPath = location?.path;
    } catch (error) {
      if (!context.mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.transcodeStartFailed(error.toString()))),
      );
      return;
    }
    if (outputPath == null || outputPath.trim().isEmpty) {
      return;
    }
    outputPath = outputPath.trim();

    final taskId = _buildTranscodeTaskId(track, encoder);
    if (!context.mounted) return;
    final progressNotifier = ValueNotifier<TranscodeProgressEvent?>(null);
    final cancelingNotifier = ValueNotifier<bool>(false);
    var cancelRequested = false;
    Future<void> requestCancel() async {
      if (cancelRequested) return;
      cancelRequested = true;
      cancelingNotifier.value = true;
      try {
        await widget.bridge.transcodeCancel(taskId: taskId);
      } catch (error) {
        cancelRequested = false;
        cancelingNotifier.value = false;
        if (!context.mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.transcodeCancelFailed(error.toString()))),
        );
      }
    }

    final dialogFuture = showGeneralDialog<void>(
      context: context,
      barrierDismissible: false,
      barrierLabel: l10n.transcodeProgressDialogTitle,
      barrierColor: Colors.black.withValues(alpha: 0.38),
      transitionDuration: const Duration(milliseconds: 180),
      pageBuilder: (dialogContext, animation, secondaryAnimation) {
        return SafeArea(
          child: Center(
            child: _TranscodeProgressDialogCard(
              progressListenable: progressNotifier,
              cancelingListenable: cancelingNotifier,
              encoderName: encoder.displayName,
              sourceName: _trackDisplayName(track),
              onCancelPressed: requestCancel,
            ),
          ),
        );
      },
      transitionBuilder: (context, animation, secondaryAnimation, child) {
        final curved = CurvedAnimation(
          parent: animation,
          curve: Curves.easeOutCubic,
          reverseCurve: Curves.easeInCubic,
        );
        return FadeTransition(
          opacity: curved,
          child: ScaleTransition(
            scale: Tween<double>(begin: 0.94, end: 1.0).animate(curved),
            child: child,
          ),
        );
      },
    );

    var finalOutputPath = outputPath;
    String? failureMessage;
    var succeeded = false;
    var canceled = false;
    final done = Completer<void>();
    late final StreamSubscription<TranscodeProgressEvent> subscription;
    try {
      subscription = widget.bridge
          .transcodeTrackLocal(
            taskId: taskId,
            sourcePath: sourcePath,
            outputPath: outputPath,
            encoderPluginId: encoder.pluginId,
            encoderTypeId: encoder.typeId,
            encoderConfigJson: encoderConfigJson,
            encoderOptionsJson: encoderOptionsJson,
          )
          .listen(
            (event) {
              progressNotifier.value = event;
              final eventOutputPath = event.outputPath;
              if (eventOutputPath != null &&
                  eventOutputPath.trim().isNotEmpty) {
                finalOutputPath = eventOutputPath.trim();
              }
              final phase = event.phase.trim().toLowerCase();
              if (phase == 'completed') {
                succeeded = true;
              } else if (phase == 'canceled') {
                canceled = true;
              } else if (phase == 'failed') {
                final raw = event.message?.trim();
                failureMessage = (raw == null || raw.isEmpty)
                    ? l10n.error
                    : raw;
              } else {
                return;
              }
              if (!done.isCompleted) {
                done.complete();
              }
            },
            onError: (Object error, StackTrace stackTrace) {
              failureMessage = error.toString();
              if (!done.isCompleted) {
                done.complete();
              }
            },
            onDone: () {
              if (!done.isCompleted) {
                done.complete();
              }
            },
            cancelOnError: false,
          );
      await done.future;
      await subscription.cancel();

      if (!succeeded && !canceled && (failureMessage?.isEmpty ?? true)) {
        failureMessage = l10n.error;
      }
    } catch (error) {
      failureMessage = error.toString();
    } finally {
      if (context.mounted) {
        await Navigator.of(context, rootNavigator: true).maybePop();
      }
      await dialogFuture;
      progressNotifier.dispose();
      cancelingNotifier.dispose();
    }

    if (!context.mounted) return;
    final messenger = ScaffoldMessenger.of(context);
    messenger.showSnackBar(
      SnackBar(
        behavior: SnackBarBehavior.floating,
        content: Text(
          succeeded
              ? l10n.transcodeSucceededWithPath(finalOutputPath)
              : canceled
              ? l10n.transcodeCanceled
              : l10n.transcodeFailedWithError(failureMessage ?? l10n.error),
        ),
      ),
    );
  }

  String _buildTranscodeTaskId(TrackLite track, EncoderTypeDescriptor encoder) {
    final ts = DateTime.now().microsecondsSinceEpoch;
    final source =
        '${track.id}:${track.path}:${encoder.pluginId}:${encoder.typeId}';
    final fingerprint = source.hashCode.abs();
    return 'transcode_${ts}_$fingerprint';
  }

  String _trackDisplayName(TrackLite track) {
    final title = track.title?.trim();
    if (title != null && title.isNotEmpty) {
      return title;
    }
    final filename = p.basenameWithoutExtension(track.path).trim();
    if (filename.isNotEmpty) {
      return filename;
    }
    return 'Track';
  }

  String _buildDefaultTranscodeFileName(
    TrackLite track,
    EncoderTypeDescriptor encoder,
  ) {
    final extension = _inferEncoderExtension(encoder);
    final baseName = _sanitizeFileName(_trackDisplayName(track));
    return '$baseName.$extension';
  }

  String _inferEncoderExtension(EncoderTypeDescriptor encoder) {
    final normalized = encoder.typeId.toLowerCase();
    final segments = normalized
        .split(RegExp(r'[^a-z0-9]+'))
        .where((segment) => segment.isNotEmpty)
        .toList(growable: false);
    const ignored = <String>{'encoder', 'encode', 'audio', 'plugin'};
    for (final segment in segments.reversed) {
      if (ignored.contains(segment)) continue;
      if (segment.length >= 2 && segment.length <= 8) {
        return segment;
      }
    }
    return 'out';
  }

  String _sanitizeFileName(String raw) {
    final sanitized = raw.replaceAll(RegExp(r'[\\\\/:*?"<>|]'), '_').trim();
    if (sanitized.isEmpty) {
      return 'track';
    }
    return sanitized;
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

class _TranscodeProgressDialogCard extends StatelessWidget {
  const _TranscodeProgressDialogCard({
    required this.progressListenable,
    required this.cancelingListenable,
    required this.encoderName,
    required this.sourceName,
    required this.onCancelPressed,
  });

  final ValueNotifier<TranscodeProgressEvent?> progressListenable;
  final ValueNotifier<bool> cancelingListenable;
  final String encoderName;
  final String sourceName;
  final Future<void> Function() onCancelPressed;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context)!;
    final surfaceTop = colorScheme.surfaceContainerHighest.withValues(
      alpha: 0.98,
    );
    final surfaceBottom = colorScheme.surface.withValues(alpha: 0.98);
    final borderColor = colorScheme.outlineVariant.withValues(alpha: 0.42);

    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 560),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 20),
        child: Material(
          color: Colors.transparent,
          child: ValueListenableBuilder<TranscodeProgressEvent?>(
            valueListenable: progressListenable,
            builder: (context, event, child) {
              return ValueListenableBuilder<bool>(
                valueListenable: cancelingListenable,
                builder: (context, canceling, _) {
                  final phase = event?.phase.trim().toLowerCase() ?? 'started';
                  final processed = event?.processedFrames ?? BigInt.zero;
                  final total = event?.totalFrames;
                  final writtenBytes = event?.writtenBytes ?? BigInt.zero;
                  final elapsedMs = event?.elapsedMs;
                  final progress = _progressRatio(processed, total);
                  final statusColor = switch (phase) {
                    'failed' => colorScheme.error,
                    'canceled' => colorScheme.error,
                    'completed' => colorScheme.primary,
                    _ => colorScheme.primary,
                  };
                  final progressText = progress == null
                      ? '...'
                      : '${(progress * 100).clamp(0, 100).toStringAsFixed(1)}%';
                  final isTerminal =
                      phase == 'failed' ||
                      phase == 'completed' ||
                      phase == 'canceled';

                  return AnimatedContainer(
                    duration: const Duration(milliseconds: 220),
                    curve: Curves.easeOutCubic,
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(24),
                      border: Border.all(color: borderColor),
                      gradient: LinearGradient(
                        begin: Alignment.topCenter,
                        end: Alignment.bottomCenter,
                        colors: [surfaceTop, surfaceBottom],
                      ),
                      boxShadow: [
                        BoxShadow(
                          color: Colors.black.withValues(alpha: 0.18),
                          blurRadius: 28,
                          offset: const Offset(0, 12),
                        ),
                      ],
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Container(
                          width: double.infinity,
                          padding: const EdgeInsets.fromLTRB(20, 16, 20, 16),
                          decoration: BoxDecoration(
                            borderRadius: const BorderRadius.vertical(
                              top: Radius.circular(24),
                            ),
                            gradient: LinearGradient(
                              colors: [
                                colorScheme.primary.withValues(alpha: 0.20),
                                colorScheme.primary.withValues(alpha: 0.08),
                              ],
                            ),
                          ),
                          child: Row(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Container(
                                width: 42,
                                height: 42,
                                decoration: BoxDecoration(
                                  color: colorScheme.primary.withValues(
                                    alpha: 0.16,
                                  ),
                                  borderRadius: BorderRadius.circular(12),
                                ),
                                child: Icon(
                                  Icons.transform_rounded,
                                  color: colorScheme.primary,
                                ),
                              ),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      l10n.transcodeProgressDialogTitle,
                                      style: theme.textTheme.titleMedium
                                          ?.copyWith(
                                            fontWeight: FontWeight.w700,
                                          ),
                                    ),
                                    const SizedBox(height: 4),
                                    Text(
                                      l10n.transcodeProgressDialogSubtitle(
                                        encoderName,
                                      ),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: theme.textTheme.bodySmall
                                          ?.copyWith(
                                            color: colorScheme.onSurfaceVariant,
                                          ),
                                    ),
                                    const SizedBox(height: 2),
                                    Text(
                                      sourceName,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: theme.textTheme.bodySmall
                                          ?.copyWith(
                                            color: colorScheme.onSurfaceVariant,
                                          ),
                                    ),
                                  ],
                                ),
                              ),
                            ],
                          ),
                        ),
                        Padding(
                          padding: const EdgeInsets.fromLTRB(20, 16, 20, 20),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Row(
                                children: [
                                  Expanded(
                                    child: Text(
                                      _phaseLabel(l10n, phase),
                                      style: theme.textTheme.bodyMedium
                                          ?.copyWith(
                                            color: statusColor,
                                            fontWeight: FontWeight.w700,
                                          ),
                                    ),
                                  ),
                                  Text(
                                    progressText,
                                    style: theme.textTheme.bodyMedium?.copyWith(
                                      fontFeatures: const [
                                        FontFeature.tabularFigures(),
                                      ],
                                      fontWeight: FontWeight.w700,
                                    ),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 10),
                              ClipRRect(
                                borderRadius: BorderRadius.circular(999),
                                child: SizedBox(
                                  height: 10,
                                  child: progress == null
                                      ? LinearProgressIndicator(
                                          value: null,
                                          backgroundColor: colorScheme
                                              .surfaceContainerHighest,
                                          valueColor:
                                              AlwaysStoppedAnimation<Color>(
                                                colorScheme.primary,
                                              ),
                                        )
                                      : TweenAnimationBuilder<double>(
                                          tween: Tween<double>(
                                            begin: 0,
                                            end: progress,
                                          ),
                                          duration: const Duration(
                                            milliseconds: 260,
                                          ),
                                          curve: Curves.easeOutCubic,
                                          builder: (context, value, child) {
                                            return LinearProgressIndicator(
                                              value: value,
                                              backgroundColor: colorScheme
                                                  .surfaceContainerHighest,
                                              valueColor:
                                                  AlwaysStoppedAnimation<Color>(
                                                    statusColor,
                                                  ),
                                            );
                                          },
                                        ),
                                ),
                              ),
                              const SizedBox(height: 14),
                              Wrap(
                                spacing: 8,
                                runSpacing: 8,
                                children: [
                                  _TranscodeMetricChip(
                                    label: l10n.transcodeStatProcessed,
                                    value: _formatFrames(processed, total),
                                  ),
                                  _TranscodeMetricChip(
                                    label: l10n.transcodeStatWritten,
                                    value: _formatBytes(writtenBytes),
                                  ),
                                  _TranscodeMetricChip(
                                    label: l10n.transcodeStatElapsed,
                                    value: _formatElapsed(elapsedMs),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 14),
                              Align(
                                alignment: Alignment.centerRight,
                                child: TextButton.icon(
                                  onPressed: canceling || isTerminal
                                      ? null
                                      : () {
                                          unawaited(onCancelPressed());
                                        },
                                  icon: canceling
                                      ? const SizedBox(
                                          width: 14,
                                          height: 14,
                                          child: CircularProgressIndicator(
                                            strokeWidth: 2,
                                          ),
                                        )
                                      : const Icon(Icons.close_rounded),
                                  label: Text(
                                    canceling
                                        ? l10n.transcodeCanceling
                                        : l10n.transcodeCancel,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  );
                },
              );
            },
          ),
        ),
      ),
    );
  }

  String _phaseLabel(AppLocalizations l10n, String phase) {
    return switch (phase) {
      'failed' => l10n.transcodeStateFailed,
      'canceled' => l10n.transcodeStateCanceled,
      'completed' => l10n.transcodeStateCompleted,
      'progress' => l10n.transcodeStateProcessing,
      _ => l10n.transcodeStatePreparing,
    };
  }

  double? _progressRatio(BigInt processed, BigInt? total) {
    if (total == null || total <= BigInt.zero) return null;
    final capped = processed > total ? total : processed;
    final numerator = capped.toDouble();
    final denominator = total.toDouble();
    if (denominator <= 0) return null;
    return numerator / denominator;
  }

  String _formatFrames(BigInt processed, BigInt? total) {
    final processedText = _groupDigits(processed);
    if (total == null || total <= BigInt.zero) {
      return processedText;
    }
    return '$processedText / ${_groupDigits(total)}';
  }

  String _formatBytes(BigInt bytes) {
    final value = bytes < BigInt.zero ? BigInt.zero : bytes;
    final units = <String>['B', 'KB', 'MB', 'GB', 'TB'];
    var v = value.toDouble();
    var idx = 0;
    while (v >= 1024 && idx < units.length - 1) {
      v /= 1024;
      idx += 1;
    }
    final digits = v >= 100 ? 0 : (v >= 10 ? 1 : 2);
    return '${v.toStringAsFixed(digits)} ${units[idx]}';
  }

  String _formatElapsed(BigInt? elapsedMs) {
    final ms = elapsedMs?.toInt() ?? 0;
    final totalSeconds = (ms / 1000).floor().clamp(0, 24 * 60 * 60 * 99);
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }

  String _groupDigits(BigInt value) {
    final raw = value.toString();
    final buffer = StringBuffer();
    for (var i = 0; i < raw.length; i += 1) {
      final idxFromEnd = raw.length - i;
      buffer.write(raw[i]);
      if (idxFromEnd > 1 && idxFromEnd % 3 == 1) {
        buffer.write(',');
      }
    }
    return buffer.toString();
  }
}

class _TranscodeLaunchParams {
  const _TranscodeLaunchParams({
    required this.encoderConfigJson,
    required this.encoderOptionsJson,
  });

  final String encoderConfigJson;
  final String? encoderOptionsJson;
}

class _TranscodeMetricChip extends StatelessWidget {
  const _TranscodeMetricChip({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(999),
        color: colorScheme.surfaceContainerHighest.withValues(alpha: 0.72),
        border: Border.all(
          color: colorScheme.outlineVariant.withValues(alpha: 0.38),
        ),
      ),
      child: RichText(
        text: TextSpan(
          style: theme.textTheme.bodySmall?.copyWith(
            color: colorScheme.onSurface,
            fontFeatures: const [FontFeature.tabularFigures()],
          ),
          children: [
            TextSpan(
              text: '$label: ',
              style: const TextStyle(fontWeight: FontWeight.w600),
            ),
            TextSpan(text: value),
          ],
        ),
      ),
    );
  }
}
