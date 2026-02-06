import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/queue_models.dart';

/// Bottom playback bar with time, controls, progress slider, and volume.
class BottomPlaybackBar extends StatefulWidget {
  const BottomPlaybackBar({
    super.key,
    required this.positionMs,
    required this.durationMs,
    required this.isPlaying,
    required this.playMode,
    required this.volume,
    required this.onPlayPause,
    required this.onPrevious,
    required this.onNext,
    required this.onSeek,
    required this.onVolumeChanged,
    required this.onPlayModeChanged,
    required this.foregroundColor,
    this.currentPath,
    this.sampleRate,
  });

  final int positionMs;
  final int durationMs;
  final bool isPlaying;
  final PlayMode playMode;
  final double volume;
  final VoidCallback onPlayPause;
  final VoidCallback onPrevious;
  final VoidCallback onNext;
  final ValueChanged<int> onSeek;
  final ValueChanged<double> onVolumeChanged;
  final VoidCallback onPlayModeChanged;
  final Color foregroundColor;
  final String? currentPath;
  final int? sampleRate;

  @override
  State<BottomPlaybackBar> createState() => _BottomPlaybackBarState();
}

class _BottomPlaybackBarState extends State<BottomPlaybackBar> {
  String _formatMs(int ms) {
    final totalSeconds = ms ~/ 1000;
    final minutes = totalSeconds ~/ 60;
    final seconds = totalSeconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context)!;

    final playModeIcon = switch (widget.playMode) {
      PlayMode.sequential => Icons.arrow_forward,
      PlayMode.shuffle => Icons.shuffle,
      PlayMode.repeatAll => Icons.repeat,
      PlayMode.repeatOne => Icons.repeat_one,
    };

    final playModeTooltip = switch (widget.playMode) {
      PlayMode.sequential => l10n.playModeSequential,
      PlayMode.shuffle => l10n.playModeShuffle,
      PlayMode.repeatAll => l10n.playModeRepeatAll,
      PlayMode.repeatOne => l10n.playModeRepeatOne,
    };

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        // Progress bar
        DetailProgressBar(
          positionMs: widget.positionMs,
          durationMs: widget.durationMs,
          isPlaying: widget.isPlaying,
          onSeek: widget.onSeek,
          foregroundColor: widget.foregroundColor,
        ),
        // Controls row
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: Row(
            children: [
              // Left: Time display
              SizedBox(
                width: 60,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      _formatMs(widget.positionMs),
                      style: theme.textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w500,
                        color: widget.foregroundColor,
                      ),
                    ),
                    Text(
                      _formatMs(widget.durationMs),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: widget.foregroundColor.withValues(alpha: 0.6),
                      ),
                    ),
                  ],
                ),
              ),
              // Center: Playback controls
              Expanded(
                child: FittedBox(
                  fit: BoxFit.scaleDown,
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      IconButton(
                        icon: Icon(
                          Icons.equalizer,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 24,
                        onPressed: () {
                          // TODO: Equalizer
                        },
                      ),
                      // Volume button with popup
                      VolumeButton(
                        volume: widget.volume,
                        onChanged: widget.onVolumeChanged,
                        foregroundColor: widget.foregroundColor,
                      ),
                      const SizedBox(width: 8),
                      IconButton(
                        icon: Icon(
                          Icons.skip_previous,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 32,
                        tooltip: l10n.tooltipPrevious,
                        onPressed: widget.onPrevious,
                      ),
                      const SizedBox(width: 4),
                      IconButton(
                        icon: Icon(
                          widget.isPlaying ? Icons.pause : Icons.play_arrow,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 40,
                        tooltip: widget.isPlaying ? l10n.pause : l10n.play,
                        onPressed: widget.onPlayPause,
                      ),
                      const SizedBox(width: 4),
                      IconButton(
                        icon: Icon(
                          Icons.skip_next,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 32,
                        tooltip: l10n.tooltipNext,
                        onPressed: widget.onNext,
                      ),
                      const SizedBox(width: 8),
                      IconButton(
                        icon: Icon(playModeIcon, color: widget.foregroundColor),
                        iconSize: 24,
                        tooltip: playModeTooltip,
                        onPressed: widget.onPlayModeChanged,
                      ),
                      IconButton(
                        icon: Icon(Icons.menu, color: widget.foregroundColor),
                        iconSize: 24,
                        onPressed: () {
                          // TODO: Queue menu
                        },
                      ),
                    ],
                  ),
                ),
              ),
              // Right: Placeholder for additional controls
              const SizedBox(width: 60),
            ],
          ),
        ),
        const SizedBox(height: 8),
      ],
    );
  }
}

/// Volume button with popup slider.
class VolumeButton extends StatefulWidget {
  const VolumeButton({
    super.key,
    required this.volume,
    required this.onChanged,
    required this.foregroundColor,
  });

  final double volume;
  final ValueChanged<double> onChanged;
  final Color foregroundColor;

  @override
  State<VolumeButton> createState() => _VolumeButtonState();
}

class _VolumeButtonState extends State<VolumeButton> {
  OverlayEntry? _overlayEntry;
  final LayerLink _layerLink = LayerLink();

  @override
  void dispose() {
    _removeOverlay();
    super.dispose();
  }

  void _removeOverlay() {
    _overlayEntry?.remove();
    _overlayEntry = null;
  }

  void _toggleOverlay() {
    if (_overlayEntry != null) {
      _removeOverlay();
      return;
    }

    final overlay = Overlay.maybeOf(context);
    if (overlay == null) return;

    _overlayEntry = OverlayEntry(
      builder: (context) {
        final theme = Theme.of(context);
        return Stack(
          children: [
            // Tap outside to close
            Positioned.fill(
              child: GestureDetector(
                behavior: HitTestBehavior.translucent,
                onTap: _removeOverlay,
                child: Container(color: Colors.transparent),
              ),
            ),
            // Volume popup
            Positioned(
              width: 48,
              height: 160,
              child: CompositedTransformFollower(
                link: _layerLink,
                targetAnchor: Alignment.topCenter,
                followerAnchor: Alignment.bottomCenter,
                offset: const Offset(0, -8),
                child: Material(
                  elevation: 4,
                  borderRadius: BorderRadius.circular(12),
                  color: theme.colorScheme.surfaceContainerHigh,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(vertical: 12),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          '${(widget.volume * 100).round()}%',
                          style: theme.textTheme.labelSmall,
                        ),
                        Expanded(
                          child: RotatedBox(
                            quarterTurns: -1,
                            child: SliderTheme(
                              data: SliderThemeData(
                                trackHeight: 4,
                                thumbShape: const RoundSliderThumbShape(
                                  enabledThumbRadius: 6,
                                ),
                                overlayShape: const RoundSliderOverlayShape(
                                  overlayRadius: 12,
                                ),
                                activeTrackColor: theme.colorScheme.primary,
                                inactiveTrackColor: theme.colorScheme.primary
                                    .withValues(alpha: 0.2),
                                thumbColor: theme.colorScheme.primary,
                              ),
                              child: Slider(
                                value: widget.volume.clamp(0.0, 1.0),
                                onChanged: (v) {
                                  widget.onChanged(v);
                                  _overlayEntry?.markNeedsBuild();
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
          ],
        );
      },
    );
    overlay.insert(_overlayEntry!);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final iconData = widget.volume <= 0
        ? Icons.volume_off
        : widget.volume < 0.5
        ? Icons.volume_down
        : Icons.volume_up;

    return CompositedTransformTarget(
      link: _layerLink,
      child: IconButton(
        icon: Icon(iconData, color: widget.foregroundColor),
        iconSize: 24,
        tooltip: l10n.volume,
        onPressed: _toggleOverlay,
      ),
    );
  }
}

/// Custom progress bar with smooth animation, hover effect, and no thumb.
class DetailProgressBar extends StatefulWidget {
  const DetailProgressBar({
    super.key,
    required this.positionMs,
    required this.durationMs,
    required this.isPlaying,
    required this.onSeek,
    required this.foregroundColor,
  });

  final int positionMs;
  final int durationMs;
  final bool isPlaying;
  final ValueChanged<int> onSeek;
  final Color foregroundColor;

  @override
  State<DetailProgressBar> createState() => _DetailProgressBarState();
}

class _DetailProgressBarState extends State<DetailProgressBar>
    with TickerProviderStateMixin {
  late final AnimationController _controller;
  late final Ticker _ticker;
  bool _dragging = false;
  bool _hovering = false;
  DateTime? _baseAt;
  int _basePosMs = 0;

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
      if (d <= 0) return;
      final at = _baseAt;
      if (at == null) return;
      final elapsed = DateTime.now().difference(at).inMilliseconds;
      final pos = (_basePosMs + elapsed).clamp(0, d);
      _controller.value = (pos / d).clamp(0.0, 1.0);
    });
    _syncTicker();
  }

  @override
  void didUpdateWidget(covariant DetailProgressBar oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_dragging) return;

    final now = DateTime.now();
    final durationMs = widget.durationMs;

    if (oldWidget.positionMs != widget.positionMs ||
        oldWidget.durationMs != widget.durationMs ||
        oldWidget.isPlaying != widget.isPlaying) {
      _basePosMs = widget.positionMs;
      _baseAt = now;
    }

    _syncTicker();

    if (!_shouldTick) {
      _controller.animateTo(_targetValue(), curve: Curves.easeOutCubic);
    } else if (durationMs > 0) {
      final elapsed = _baseAt != null
          ? now.difference(_baseAt!).inMilliseconds
          : 0;
      final pos = (_basePosMs + elapsed).clamp(0, durationMs);
      _controller.value = (pos / durationMs).clamp(0.0, 1.0);
    }
  }

  bool get _shouldTick {
    if (_dragging) return false;
    if (widget.durationMs <= 0) return false;
    return widget.isPlaying;
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
    if (d <= 0) return 0;
    return (widget.positionMs / d).clamp(0.0, 1.0);
  }

  double _posToFraction(Offset local, double width) {
    if (width <= 0) return 0;
    return (local.dx / width).clamp(0.0, 1.0);
  }

  int _fractionToMs(double fraction) {
    final d = widget.durationMs;
    if (d <= 0) return 0;
    return (fraction * d).round().clamp(0, d);
  }

  bool get _emphasized => _hovering || _dragging;

  @override
  void dispose() {
    _ticker.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final d = widget.durationMs;

    final trackHeightTarget = _emphasized ? 6.0 : 4.0;
    const double barHeight = 16.0; // Fixed height to avoid layout shift

    return SizedBox(
      height: barHeight,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final w = constraints.maxWidth;

          return MouseRegion(
            cursor: d > 0 ? SystemMouseCursors.click : SystemMouseCursors.basic,
            onEnter: (_) => setState(() => _hovering = true),
            onExit: (_) => setState(() => _hovering = false),
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onTapDown: d > 0
                  ? (details) {
                      final frac = _posToFraction(details.localPosition, w);
                      final ms = _fractionToMs(frac);
                      _controller.stop();
                      _controller.value = frac;
                      _basePosMs = ms;
                      _baseAt = DateTime.now();
                      widget.onSeek(ms);
                    }
                  : null,
              onHorizontalDragStart: d > 0
                  ? (details) {
                      setState(() => _dragging = true);
                      _syncTicker();
                      final frac = _posToFraction(details.localPosition, w);
                      _controller.stop();
                      _controller.value = frac;
                    }
                  : null,
              onHorizontalDragUpdate: d > 0
                  ? (details) {
                      final frac = _posToFraction(details.localPosition, w);
                      _controller.value = frac;
                    }
                  : null,
              onHorizontalDragEnd: d > 0
                  ? (_) {
                      final ms = _fractionToMs(_controller.value);
                      setState(() => _dragging = false);
                      _basePosMs = ms;
                      _baseAt = DateTime.now();
                      _syncTicker();
                      widget.onSeek(ms);
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
              },
              child: TweenAnimationBuilder<double>(
                duration: const Duration(milliseconds: 140),
                curve: Curves.easeOutCubic,
                tween: Tween<double>(end: trackHeightTarget),
                builder: (context, trackH, _) {
                  return AnimatedBuilder(
                    animation: _controller,
                    builder: (context, _) {
                      return CustomPaint(
                        size: Size(w, barHeight),
                        painter: DetailProgressPainter(
                          progress: _controller.value,
                          trackHeight: trackH,
                          centerY: barHeight / 2,
                          trackColor: widget.foregroundColor.withValues(
                            alpha: 0.15,
                          ),
                          fillColor: widget.foregroundColor,
                        ),
                      );
                    },
                  );
                },
              ),
            ),
          );
        },
      ),
    );
  }
}

class DetailProgressPainter extends CustomPainter {
  const DetailProgressPainter({
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
  bool shouldRepaint(covariant DetailProgressPainter oldDelegate) {
    return oldDelegate.progress != progress ||
        oldDelegate.trackHeight != trackHeight ||
        oldDelegate.centerY != centerY ||
        oldDelegate.trackColor != trackColor ||
        oldDelegate.fillColor != fillColor;
  }
}
