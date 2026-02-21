import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:animations/animations.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/scheduler.dart';

import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/queue_models.dart';

class NowPlayingCommon {
  static String formatMs(int ms) {
    final totalSeconds = (ms / 1000).floor();
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }
}

class NowPlayingCover extends StatelessWidget {
  const NowPlayingCover({
    super.key,
    required this.coverDir,
    required this.trackId,
    this.cover,
    required this.primaryColor,
    required this.onTap,
  });

  final String coverDir;
  final int? trackId;
  final QueueCover? cover;
  final Color primaryColor;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final placeholder = Container(
      width: 48,
      height: 48,
      decoration: BoxDecoration(
        color: primaryColor.withValues(alpha: 0.12),
        border: Border.all(color: primaryColor.withValues(alpha: 0.18)),
      ),
      child: Icon(Icons.music_note, color: primaryColor),
    );

    if (trackId == null) {
      return MouseRegion(
        cursor: onTap != null
            ? SystemMouseCursors.click
            : SystemMouseCursors.basic,
        child: GestureDetector(
          onTap: onTap,
          child: _buildCoverByRef(placeholder),
        ),
      );
    }

    final coverPath = '$coverDir${Platform.pathSeparator}$trackId';
    final provider = ResizeImage(
      FileImage(File(coverPath)),
      width: 96,
      height: 96,
      allowUpscaling: false,
    );

    return MouseRegion(
      cursor: onTap != null
          ? SystemMouseCursors.click
          : SystemMouseCursors.basic,
      child: GestureDetector(
        onTap: onTap,
        child: Image(
          image: provider,
          width: 48,
          height: 48,
          fit: BoxFit.cover,
          gaplessPlayback: true,
          errorBuilder: (context, error, stackTrace) =>
              _buildCoverByRef(placeholder),
        ),
      ),
    );
  }

  Widget _buildCoverByRef(Widget placeholder) {
    final ref = cover;
    if (ref == null) return placeholder;
    switch (ref.kind) {
      case QueueCoverKind.url:
        return ClipRRect(
          borderRadius: BorderRadius.circular(2),
          child: Image.network(
            ref.value,
            width: 48,
            height: 48,
            fit: BoxFit.cover,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
      case QueueCoverKind.file:
        return ClipRRect(
          borderRadius: BorderRadius.circular(2),
          child: Image.file(
            File(ref.value),
            width: 48,
            height: 48,
            fit: BoxFit.cover,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
      case QueueCoverKind.data:
        final bytes = _decodeCoverBytes(ref.value);
        if (bytes == null) return placeholder;
        return ClipRRect(
          borderRadius: BorderRadius.circular(2),
          child: Image.memory(
            bytes,
            width: 48,
            height: 48,
            fit: BoxFit.cover,
            gaplessPlayback: true,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
    }
  }

  Uint8List? _decodeCoverBytes(String raw) {
    final text = raw.trim();
    if (text.isEmpty) return null;
    final data = () {
      if (text.startsWith('data:')) {
        final comma = text.indexOf(',');
        if (comma <= 0 || comma >= text.length - 1) return '';
        return text.substring(comma + 1);
      }
      return text;
    }();
    if (data.isEmpty) return null;
    try {
      return base64Decode(data);
    } catch (_) {
      return null;
    }
  }
}

class NowPlayingProgressBar extends StatefulWidget {
  const NowPlayingProgressBar({
    super.key,
    required this.durationMs,
    required this.positionMs,
    required this.enabled,
    required this.audioStarted,
    required this.playerState,
    required this.onSeekMs,
    this.foregroundColor,
    this.trackHeight = 3.0,
    this.activeTrackHeight = 6.0,
    this.barHeight = 6.0,
    this.activeBarHeight = 14.0,
    this.thumbRadius = 0.0,
    this.showTooltip = true,
  });

  final int? durationMs;
  final int positionMs;
  final bool enabled;
  final bool audioStarted;
  final PlayerState playerState;
  final ValueChanged<int> onSeekMs;
  final Color? foregroundColor;
  final double trackHeight;
  final double activeTrackHeight;
  final double barHeight;
  final double activeBarHeight;
  final double thumbRadius;
  final bool showTooltip;

  @override
  State<NowPlayingProgressBar> createState() => _NowPlayingProgressBarState();
}

class _NowPlayingProgressBarState extends State<NowPlayingProgressBar>
    with TickerProviderStateMixin {
  late final AnimationController _controller;
  late final Ticker _ticker;
  final LayerLink _tooltipLink = LayerLink();
  OverlayEntry? _tooltipEntry;
  bool _tooltipRebuildScheduled = false;
  double _lastLayoutWidth = 0;
  double _lastLayoutHeight = 0;
  Size? _tooltipSize;
  int? _pendingSeekMs;
  DateTime? _pendingSeekAt;
  int? _pendingSeekFromMs;
  bool _dragging = false;
  bool _hovering = false;
  DateTime? _baseAt;
  int _basePosMs = 0;

  int _predictedPosMsAt(DateTime now, int durationMs) {
    if (durationMs <= 0) return widget.positionMs;
    final at = _baseAt;
    if (at != null) {
      final elapsed = now.difference(at).inMilliseconds;
      return (_basePosMs + elapsed).clamp(0, durationMs);
    }
    return (_controller.value * durationMs).round().clamp(0, durationMs);
  }

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      lowerBound: 0,
      upperBound: 1,
      duration: const Duration(milliseconds: 180),
    );
    _controller.value = _targetValue();
    _basePosMs = widget.positionMs;
    _baseAt = DateTime.now();
    _ticker = createTicker((_) {
      if (!_shouldTick) return;
      final d = widget.durationMs;
      if (d == null || d <= 0) return;
      final at = _baseAt;
      if (at == null) return;
      final elapsed = DateTime.now().difference(at).inMilliseconds;
      final pos = (_basePosMs + elapsed).clamp(0, d);
      _controller.value = (pos / d).clamp(0.0, 1.0);
      _requestTooltipRebuild();
    });
    _syncTicker();
  }

  @override
  void didUpdateWidget(covariant NowPlayingProgressBar oldWidget) {
    super.didUpdateWidget(oldWidget);
    final wasPlaying = oldWidget.playerState == PlayerState.playing;
    final isPlaying = widget.playerState == PlayerState.playing;

    final durationChanged = oldWidget.durationMs != widget.durationMs;
    final posChanged = oldWidget.positionMs != widget.positionMs;
    final enabledChanged = oldWidget.enabled != widget.enabled;
    final likelyTrackSwitch =
        posChanged &&
        oldWidget.positionMs > 1500 &&
        widget.positionMs <= 400 &&
        !(_dragging);

    final now = DateTime.now();
    final durationMs = widget.durationMs ?? 0;
    final prevPredictedMs = _predictedPosMsAt(now, durationMs);
    final pendingMs = _pendingSeekMs;
    final pendingAt = _pendingSeekAt;
    final pendingFromMs = _pendingSeekFromMs;
    final pendingAgeMs = pendingAt == null
        ? null
        : now.difference(pendingAt).inMilliseconds;
    final pendingExpired = pendingAgeMs != null && pendingAgeMs > 5000;
    final pendingClose =
        pendingMs != null && (widget.positionMs - pendingMs).abs() <= 600;
    final pendingForward =
        pendingMs != null && pendingFromMs != null && pendingMs > pendingFromMs;
    final pendingBackward =
        pendingMs != null && pendingFromMs != null && pendingMs < pendingFromMs;
    final pendingCaughtUp = () {
      if (pendingMs == null || pendingFromMs == null) return false;
      if (pendingForward) return widget.positionMs >= pendingMs - 200;
      if (pendingBackward) return widget.positionMs <= pendingMs + 200;
      return pendingClose;
    }();

    if (widget.playerState == PlayerState.stopped || likelyTrackSwitch) {
      _pendingSeekMs = null;
      _pendingSeekAt = null;
      _pendingSeekFromMs = null;
    } else if (pendingMs != null &&
        pendingAt != null &&
        pendingFromMs != null) {
      if ((pendingClose && pendingCaughtUp) || pendingExpired) {
        _pendingSeekMs = null;
        _pendingSeekAt = null;
        _pendingSeekFromMs = null;
      }
    }

    // Avoid "seek jitter": while a pending seek is active, ignore backend position
    // updates that are still on the old side of the seek.
    final shouldAcceptPosUpdate = () {
      final currentPendingMs = _pendingSeekMs;
      final currentPendingAt = _pendingSeekAt;
      final currentPendingFromMs = _pendingSeekFromMs;
      if (currentPendingMs == null ||
          currentPendingAt == null ||
          currentPendingFromMs == null) {
        return true;
      }
      final ageMs = now.difference(currentPendingAt).inMilliseconds;
      if (ageMs > 5000) return true;

      if (currentPendingMs > currentPendingFromMs) {
        // Forward seek: ignore positions that are still far behind the target.
        if (widget.positionMs < currentPendingMs - 200) return false;
      } else if (currentPendingMs < currentPendingFromMs) {
        // Backward seek: ignore positions that are still far ahead of the target.
        if (widget.positionMs > currentPendingMs + 200) return false;
      }

      return true;
    }();

    final effectivePosMsForBase = () {
      final d = durationMs;
      final p = _pendingSeekMs;
      final pAt = _pendingSeekAt;
      final pFrom = _pendingSeekFromMs;
      if (p != null && pAt != null && pFrom != null && !shouldAcceptPosUpdate) {
        if (d > 0 && widget.playerState == PlayerState.playing) {
          final elapsed = now.difference(pAt).inMilliseconds;
          return (p + elapsed).clamp(0, d);
        }
        return d > 0 ? p.clamp(0, d) : p;
      }
      return d > 0 ? widget.positionMs.clamp(0, d) : widget.positionMs;
    }();

    if (durationChanged ||
        enabledChanged ||
        wasPlaying != isPlaying ||
        (posChanged && shouldAcceptPosUpdate)) {
      _basePosMs = effectivePosMsForBase;
      if (durationMs > 0 && widget.playerState == PlayerState.playing) {
        // Preserve visual continuity even if the backend reports a slightly earlier
        // position (common right after seek or due to coarse position updates).
        final deltaMs = prevPredictedMs - _basePosMs;
        if (deltaMs > 0 && deltaMs <= 1200) {
          _baseAt = now.subtract(Duration(milliseconds: deltaMs));
        } else {
          _baseAt = now;
        }
      } else {
        _baseAt = now;
      }
    }

    _syncTicker();

    if (_dragging) return;

    if (!_shouldTick) {
      _controller.animateTo(_targetValue(), curve: Curves.easeOutCubic);
      return;
    }

    if (durationMs > 0 && _baseAt != null) {
      final pos = _predictedPosMsAt(now, durationMs);
      _controller.value = (pos / durationMs).clamp(0.0, 1.0);
    } else {
      _controller.value = _targetValue();
    }
    _requestTooltipRebuild();
  }

  bool get _shouldTick {
    final d = widget.durationMs;
    if (_dragging) return false;
    if (!widget.enabled || !widget.audioStarted || d == null || d <= 0)
      return false;
    return widget.playerState == PlayerState.playing;
  }

  void _syncTicker() {
    if (_shouldTick) {
      if (!_ticker.isActive) _ticker.start();
    } else {
      if (_ticker.isActive) _ticker.stop();
    }
  }

  double _targetValue() {
    final d = widget.durationMs;
    if (!widget.enabled || !widget.audioStarted || d == null || d <= 0)
      return 0;
    final pendingMs = _pendingSeekMs;
    final pendingAt = _pendingSeekAt;
    final pendingFromMs = _pendingSeekFromMs;
    if (pendingMs != null &&
        pendingAt != null &&
        pendingFromMs != null &&
        !(() {
          // If pending is active and the backend is still on the old side, keep
          // using the optimistic seek position to avoid a backwards jump.
          if (pendingMs > pendingFromMs) {
            return widget.positionMs >= pendingMs - 200;
          }
          if (pendingMs < pendingFromMs) {
            return widget.positionMs <= pendingMs + 200;
          }
          return (widget.positionMs - pendingMs).abs() <= 600;
        }())) {
      if (widget.playerState == PlayerState.playing) {
        final elapsed = DateTime.now().difference(pendingAt).inMilliseconds;
        return ((pendingMs + elapsed) / d).clamp(0.0, 1.0);
      }
      return (pendingMs / d).clamp(0.0, 1.0);
    }
    return (widget.positionMs / d).clamp(0.0, 1.0);
  }

  double _posToFraction(Offset local, double width) {
    if (width <= 0) return 0;
    return (local.dx / width).clamp(0.0, 1.0);
  }

  int _fractionToMs(double fraction) {
    final d = widget.durationMs ?? 0;
    if (d <= 0) return 0;
    return (fraction * d).round().clamp(0, d);
  }

  void _registerPendingSeek(int ms) {
    final d = widget.durationMs ?? 0;
    _pendingSeekMs = ms;
    _pendingSeekAt = DateTime.now();
    _pendingSeekFromMs = widget.positionMs;
    _basePosMs = ms;
    _baseAt = _pendingSeekAt;
    if (d > 0) {
      _controller.value = (ms / d).clamp(0.0, 1.0);
    }
    _syncTicker();
    _requestTooltipRebuild();
  }

  bool get _emphasized => _hovering || _dragging;

  @override
  void dispose() {
    _hideTooltipOverlay();
    _ticker.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _showTooltipOverlay() {
    if (_tooltipEntry != null) return;
    final overlay = Overlay.maybeOf(context, rootOverlay: true);
    if (overlay == null) return;
    _tooltipEntry = OverlayEntry(
      builder: (context) {
        final theme = Theme.of(context);
        final d = widget.durationMs ?? 0;
        final progress = _controller.value.clamp(0.0, 1.0);
        final posMs = d > 0 ? (progress * d).round().clamp(0, d) : 0;
        final text = d > 0
            ? '${NowPlayingCommon.formatMs(posMs)} / ${NowPlayingCommon.formatMs(d)}'
            : NowPlayingCommon.formatMs(widget.positionMs);

        // Center tooltip at the latest progress position, but keep it within the bar bounds.
        const fallbackTooltipWidth = 120.0;
        final barW = _lastLayoutWidth;
        final tipW = (_tooltipSize?.width ?? fallbackTooltipWidth).clamp(
          0.0,
          barW > 0 ? barW : fallbackTooltipWidth,
        );
        final centerX = progress * barW;
        final maxLeft = (barW - tipW).clamp(0.0, barW);
        final left = (centerX - tipW / 2).clamp(0.0, maxLeft);

        return Positioned.fill(
          child: CompositedTransformFollower(
            link: _tooltipLink,
            targetAnchor: Alignment.topLeft,
            followerAnchor: Alignment.topLeft,
            showWhenUnlinked: false,
            child: IgnorePointer(
              child: SizedBox(
                width: _lastLayoutWidth,
                height: _lastLayoutHeight,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    Positioned(
                      left: left,
                      top: -34,
                      child: MeasureSize(
                        onChange: (size) {
                          final old = _tooltipSize;
                          if (old == size) return;
                          _tooltipSize = size;
                          _requestTooltipRebuild();
                        },
                        child: Material(
                          elevation: 2,
                          color: theme.colorScheme.inverseSurface.withValues(
                            alpha: 0.92,
                          ),
                          borderRadius: BorderRadius.circular(8),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 6,
                            ),
                            child: DefaultTextStyle(
                              style: theme.textTheme.bodySmall!.copyWith(
                                color: theme.colorScheme.onInverseSurface,
                              ),
                              child: Text(text),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
    overlay.insert(_tooltipEntry!);
    _requestTooltipRebuild();
  }

  void _hideTooltipOverlay() {
    _tooltipEntry?.remove();
    _tooltipEntry = null;
    _tooltipRebuildScheduled = false;
  }

  void _requestTooltipRebuild() {
    if (_tooltipEntry == null) return;
    final phase = SchedulerBinding.instance.schedulerPhase;
    final safeToMarkNow =
        phase != SchedulerPhase.persistentCallbacks &&
        phase != SchedulerPhase.postFrameCallbacks;
    if (safeToMarkNow) {
      _tooltipEntry?.markNeedsBuild();
      return;
    }
    if (_tooltipRebuildScheduled) return;
    _tooltipRebuildScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _tooltipRebuildScheduled = false;
      _tooltipEntry?.markNeedsBuild();
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final d = widget.durationMs ?? 0;
    final showIndeterminate =
        widget.enabled && d <= 0 && widget.playerState == PlayerState.buffering;

    final trackHeightTarget = _emphasized
        ? widget.activeTrackHeight
        : widget.trackHeight;
    final barHeightTarget = _emphasized
        ? widget.activeBarHeight
        : widget.barHeight;
    // Always center vertically relative to the fixed bar height if possible,
    // or just assume center is half of barHeight.
    final baseCenterY = widget.barHeight / 2;

    final fillColor = widget.foregroundColor ?? theme.colorScheme.primary;
    final trackColor =
        (widget.foregroundColor?.withValues(alpha: 0.15)) ??
        theme.colorScheme.onSurface.withValues(alpha: 0.10);

    return AnimatedContainer(
      duration: const Duration(milliseconds: 140),
      curve: Curves.easeOutCubic,
      height: barHeightTarget,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final w = constraints.maxWidth;
          _lastLayoutWidth = w;
          _lastLayoutHeight = barHeightTarget;
          final tooltip = d > 0
              ? '${NowPlayingCommon.formatMs(widget.positionMs)} / ${NowPlayingCommon.formatMs(d)}'
              : NowPlayingCommon.formatMs(widget.positionMs);

          Widget bar;
          if (showIndeterminate) {
            final topPad = (baseCenterY - trackHeightTarget / 2).clamp(
              0.0,
              barHeightTarget,
            );
            bar = Padding(
              padding: EdgeInsets.only(top: topPad),
              child: LinearProgressIndicator(
                minHeight: trackHeightTarget,
                value: null,
                backgroundColor: trackColor,
                color: fillColor,
              ),
            );
          } else {
            bar = TweenAnimationBuilder<double>(
              duration: const Duration(milliseconds: 140),
              curve: Curves.easeOutCubic,
              tween: Tween<double>(end: trackHeightTarget),
              builder: (context, trackH, _) {
                return AnimatedBuilder(
                  animation: _controller,
                  builder: (context, _) {
                    return CustomPaint(
                      size: Size(w, barHeightTarget),
                      painter: ProgressPainter(
                        progress: _controller.value,
                        trackHeight: trackH,
                        centerY: baseCenterY,
                        trackColor: trackColor,
                        fillColor: fillColor,
                      ),
                    );
                  },
                );
              },
            );
          }

          // Keep `tooltip` computed here so we can use it for a11y/diagnostics if needed.
          // Actual tooltip UI is a custom overlay so it can track the progress position.
          return CompositedTransformTarget(
            link: _tooltipLink,
            child: MouseRegion(
              cursor: widget.enabled && d > 0
                  ? SystemMouseCursors.click
                  : SystemMouseCursors.basic,
              onEnter: (_) {
                setState(() => _hovering = true);
                if (widget.showTooltip) _showTooltipOverlay();
              },
              onExit: (_) {
                setState(() => _hovering = false);
                if (!_dragging) _hideTooltipOverlay();
              },
              child: GestureDetector(
                behavior: HitTestBehavior.translucent,
                onTapDown: widget.enabled && d > 0
                    ? (details) {
                        if (widget.showTooltip) _showTooltipOverlay();
                        final frac = _posToFraction(details.localPosition, w);
                        final ms = _fractionToMs(frac);
                        _controller.stop();
                        _controller.value = frac;
                        _registerPendingSeek(ms);
                        widget.onSeekMs(ms);
                      }
                    : null,
                onHorizontalDragStart: widget.enabled && d > 0
                    ? (details) {
                        if (widget.showTooltip) _showTooltipOverlay();
                        setState(() => _dragging = true);
                        _syncTicker();
                        final frac = _posToFraction(details.localPosition, w);
                        _controller.stop();
                        _controller.value = frac;
                        _requestTooltipRebuild();
                      }
                    : null,
                onHorizontalDragUpdate: widget.enabled && d > 0
                    ? (details) {
                        final frac = _posToFraction(details.localPosition, w);
                        _controller.value = frac;
                        _requestTooltipRebuild();
                      }
                    : null,
                onHorizontalDragEnd: widget.enabled && d > 0
                    ? (_) {
                        final ms = _fractionToMs(_controller.value);
                        setState(() => _dragging = false);
                        _registerPendingSeek(ms);
                        if (!_hovering) _hideTooltipOverlay();
                        widget.onSeekMs(ms);
                      }
                    : null,
                onHorizontalDragCancel: () {
                  if (!_dragging) return;
                  setState(() => _dragging = false);
                  _syncTicker();
                  _controller.animateTo(
                    _targetValue(),
                    curve: Curves.easeOutCubic,
                  );
                  _requestTooltipRebuild();
                  if (!_hovering) _hideTooltipOverlay();
                },
                child: Semantics(label: tooltip, child: bar),
              ),
            ),
          );
        },
      ),
    );
  }
}

class ProgressPainter extends CustomPainter {
  const ProgressPainter({
    required this.progress,
    required this.trackHeight,
    required this.centerY,
    required this.trackColor,
    required this.fillColor,
  });

  final double progress;
  final double trackHeight;
  final double centerY;
  final Color trackColor;
  final Color fillColor;

  @override
  void paint(Canvas canvas, Size size) {
    final trackH = trackHeight;
    final maxY = (size.height - trackH).clamp(0.0, size.height);
    final y = (centerY - trackH / 2).clamp(0.0, maxY);
    final trackRect = Rect.fromLTWH(0, y, size.width, trackH);
    canvas.drawRect(trackRect, Paint()..color = trackColor);

    final w = (size.width * progress).clamp(0.0, size.width);
    if (w <= 0) return;
    final fillRect = Rect.fromLTWH(0, y, w, trackH);
    canvas.drawRect(fillRect, Paint()..color = fillColor);
  }

  @override
  bool shouldRepaint(covariant ProgressPainter oldDelegate) {
    return oldDelegate.progress != progress ||
        oldDelegate.trackHeight != trackHeight ||
        oldDelegate.centerY != centerY ||
        oldDelegate.trackColor != trackColor ||
        oldDelegate.fillColor != fillColor;
  }
}

class MeasureSize extends SingleChildRenderObjectWidget {
  const MeasureSize({super.key, required this.onChange, required super.child});

  final ValueChanged<Size> onChange;

  @override
  RenderObject createRenderObject(BuildContext context) {
    return RenderMeasureSize(onChange);
  }

  @override
  void updateRenderObject(
    BuildContext context,
    covariant RenderMeasureSize renderObject,
  ) {
    renderObject.onChange = onChange;
  }
}

class RenderMeasureSize extends RenderProxyBox {
  RenderMeasureSize(this.onChange);

  ValueChanged<Size> onChange;
  Size? _oldSize;

  @override
  void performLayout() {
    super.performLayout();
    final newSize = child?.size;
    if (newSize == null) return;
    if (_oldSize == newSize) return;
    _oldSize = newSize;
    WidgetsBinding.instance.addPostFrameCallback((_) => onChange(newSize));
  }
}

class VolumePopupButton extends StatefulWidget {
  const VolumePopupButton({
    super.key,
    required this.volume,
    required this.onChanged,
    this.foregroundColor,
    this.iconSize = 24.0,
    this.enableHover = false,
    this.onToggleMute,
  });

  final double volume;
  final ValueChanged<double> onChanged;
  final Color? foregroundColor;
  final double iconSize;
  final bool enableHover;
  final VoidCallback? onToggleMute;

  @override
  State<VolumePopupButton> createState() => _VolumePopupButtonState();
}

class _VolumePopupButtonState extends State<VolumePopupButton>
    with SingleTickerProviderStateMixin {
  static const _animationDuration = Duration(milliseconds: 200);
  static const _hideDelay = Duration(milliseconds: 120);
  static const _popupWidth = 56.0;
  static const _popupHeight = 180.0;

  final LayerLink _link = LayerLink();
  OverlayEntry? _entry;
  Timer? _hideTimer;
  late final AnimationController _animationController;

  bool _hoverAnchor = false;
  bool _hoverPopup = false;
  bool _dragging = false;

  double? _overrideVolume;

  @override
  void initState() {
    super.initState();
    _animationController = AnimationController(
      vsync: this,
      duration: _animationDuration,
    );
  }

  @override
  void didUpdateWidget(covariant VolumePopupButton oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_entry != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _entry?.markNeedsBuild();
      });
    }
    if (_overrideVolume != null) {
      final v = widget.volume.clamp(0.0, 1.0);
      if ((v - _overrideVolume!).abs() <= 0.0001) {
        _overrideVolume = null;
      }
    }
  }

  @override
  void dispose() {
    _hideTimer?.cancel();
    _removeOverlay();
    _animationController.dispose();
    super.dispose();
  }

  double get _volume => (_overrideVolume ?? widget.volume).clamp(0.0, 1.0);

  void _markOverlayNeedsBuild() {
    _entry?.markNeedsBuild();
  }

  void _showOverlay() {
    if (_entry != null) {
      if (!_animationController.isCompleted) {
        _animationController.forward();
      }
      return;
    }
    _animationController.forward(from: 0.0);
    final overlay = Overlay.maybeOf(context, rootOverlay: true);
    if (overlay == null) return;

    _entry = OverlayEntry(
      builder: (context) {
        final theme = Theme.of(context);
        final volume = _volume;
        final percent = (volume * 100).round();

        return Stack(
          children: [
            if (!widget.enableHover)
              Positioned.fill(
                child: GestureDetector(
                  behavior: HitTestBehavior.translucent,
                  onTap: _removeOverlay,
                  child: Container(color: Colors.transparent),
                ),
              ),
            Positioned(
              left: 0,
              top: 0,
              child: CompositedTransformFollower(
                link: _link,
                targetAnchor: Alignment.topCenter,
                followerAnchor: Alignment.bottomCenter,
                offset: const Offset(0, -10),
                showWhenUnlinked: false,
                child: MouseRegion(
                  onEnter: (_) {
                    if (widget.enableHover) {
                      _hideTimer?.cancel();
                      _hoverPopup = true;
                    }
                  },
                  onExit: (_) {
                    if (widget.enableHover) {
                      _hoverPopup = false;
                      _scheduleHideIfNeeded();
                    }
                  },
                  child: FadeScaleTransition(
                    animation: _animationController,
                    child: Material(
                      elevation: 6,
                      borderRadius: BorderRadius.circular(12),
                      color: theme.colorScheme.surfaceContainerHigh,
                      child: SizedBox(
                        width: _popupWidth,
                        height: _popupHeight,
                        child: Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 10,
                            vertical: 10,
                          ),
                          child: Column(
                            children: [
                              Text(
                                '$percent',
                                style: theme.textTheme.labelLarge,
                              ),
                              const SizedBox(height: 10),
                              Expanded(
                                child: RotatedBox(
                                  quarterTurns: -1,
                                  child: SliderTheme(
                                    data: SliderTheme.of(context).copyWith(
                                      trackHeight: 3,
                                      overlayShape:
                                          SliderComponentShape.noOverlay,
                                      activeTrackColor:
                                          theme.colorScheme.primary,
                                      thumbColor: theme.colorScheme.primary,
                                    ),
                                    child: TweenAnimationBuilder<double>(
                                      tween: Tween<double>(
                                        begin: volume,
                                        end: volume,
                                      ),
                                      duration: _dragging
                                          ? Duration.zero
                                          : const Duration(milliseconds: 200),
                                      curve: Curves.easeOutCubic,
                                      builder: (context, value, child) {
                                        return Slider(
                                          value: value,
                                          onChangeStart: (_) {
                                            _dragging = true;
                                            _hideTimer?.cancel();
                                          },
                                          onChangeEnd: (_) {
                                            _dragging = false;
                                            _scheduleHideIfNeeded();
                                          },
                                          onChanged: (v) {
                                            _overrideVolume = v;
                                            _markOverlayNeedsBuild();
                                            widget.onChanged(v);
                                          },
                                        );
                                      },
                                    ),
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ],
        );
      },
    );

    overlay.insert(_entry!);
  }

  void _removeOverlay() {
    _entry?.remove();
    _entry = null;
    _animationController.reset();
  }

  void _hideOverlay() {
    if (_entry == null) return;
    _animationController.reverse().then((_) {
      if (!mounted) return;
      // Only remove if we didn't start showing it again during dismissal
      if (_animationController.status == AnimationStatus.dismissed) {
        _removeOverlay();
      }
    });
  }

  void _scheduleHideIfNeeded() {
    _hideTimer?.cancel();
    _hideTimer = Timer(_hideDelay, () {
      if (!mounted) return;
      if (_hoverAnchor || _hoverPopup || _dragging) return;
      _hideOverlay();
    });
  }

  void _onPressed() {
    if (widget.enableHover) {
      widget.onToggleMute?.call();
      // Keep popup UI responsive
      final muted = widget.volume <= 0.0;
      _overrideVolume = muted ? null : 0.0;
      _markOverlayNeedsBuild();
    } else {
      if (_entry != null) {
        _hideOverlay();
      } else {
        _showOverlay();
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final muted = widget.volume <= 0.0;
    final iconData = muted
        ? Icons.volume_off
        : widget.volume < 0.5
        ? Icons.volume_down
        : Icons.volume_up;

    final color =
        widget.foregroundColor ??
        (muted && widget.enableHover ? null : theme.colorScheme.primary);

    return CompositedTransformTarget(
      link: _link,
      child: MouseRegion(
        onEnter: (_) {
          if (widget.enableHover) {
            _hideTimer?.cancel();
            _hoverAnchor = true;
            _showOverlay();
          }
        },
        onExit: (_) {
          if (widget.enableHover) {
            _hoverAnchor = false;
            _scheduleHideIfNeeded();
          }
        },
        child: IconButton(
          tooltip: null,
          icon: Icon(iconData),
          iconSize: widget.iconSize,
          color: color,
          onPressed: _onPressed,
        ),
      ),
    );
  }
}
