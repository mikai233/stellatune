import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

class NowPlayingBar extends ConsumerWidget {
  const NowPlayingBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final playback = ref.watch(playbackControllerProvider);
    final queue = ref.watch(queueControllerProvider);
    final selectedRenderer = ref.watch(dlnaSelectedRendererProvider);

    final currentTitle = queue.currentItem?.displayTitle ?? l10n.nowPlayingNone;
    final currentSubtitle = playback.currentPath ?? '';
    final playModeLabel = switch (queue.playMode) {
      PlayMode.sequential => l10n.playModeSequential,
      PlayMode.shuffle => l10n.playModeShuffle,
      PlayMode.repeatAll => l10n.playModeRepeatAll,
      PlayMode.repeatOne => l10n.playModeRepeatOne,
    };

    final isPlaying =
        playback.playerState == PlayerState.playing ||
        playback.playerState == PlayerState.buffering;

    return Material(
      elevation: 2,
      color: theme.colorScheme.surfaceContainer,
      child: SizedBox(
        height: 72,
        child: LayoutBuilder(
          builder: (context, constraints) {
            final showInlineVolume = constraints.maxWidth >= 920;

            return Stack(
              children: [
                Row(
                  children: [
                    const SizedBox(width: 12),
                    _CoverPlaceholder(color: theme.colorScheme.primary),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            currentTitle,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          if (currentSubtitle.isNotEmpty)
                            Text(
                              currentSubtitle,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.bodySmall,
                            ),
                        ],
                      ),
                    ),
                    Text(
                      _formatMs(playback.positionMs),
                      style: theme.textTheme.titleMedium,
                    ),
                    if (playback.lastError != null) ...[
                      const SizedBox(width: 8),
                      IconButton(
                        tooltip: playback.lastError!,
                        onPressed: () {
                          final msg = playback.lastError;
                          if (msg == null) return;
                          ScaffoldMessenger.of(
                            context,
                          ).showSnackBar(SnackBar(content: Text(msg)));
                        },
                        icon: Icon(
                          Icons.error_outline,
                          color: theme.colorScheme.error,
                        ),
                      ),
                    ],
                    const SizedBox(width: 16),
                    IconButton(
                      tooltip: l10n.tooltipPrevious,
                      onPressed: () => ref
                          .read(playbackControllerProvider.notifier)
                          .previous(),
                      icon: const Icon(Icons.skip_previous),
                    ),
                    IconButton(
                      tooltip: isPlaying ? l10n.pause : l10n.play,
                      onPressed: () => isPlaying
                          ? ref
                                .read(playbackControllerProvider.notifier)
                                .pause()
                          : ref
                                .read(playbackControllerProvider.notifier)
                                .play(),
                      icon: Icon(isPlaying ? Icons.pause : Icons.play_arrow),
                    ),
                    IconButton(
                      tooltip: l10n.stop,
                      onPressed: () =>
                          ref.read(playbackControllerProvider.notifier).stop(),
                      icon: const Icon(Icons.stop),
                    ),
                    IconButton(
                      tooltip: l10n.tooltipNext,
                      onPressed: () =>
                          ref.read(playbackControllerProvider.notifier).next(),
                      icon: const Icon(Icons.skip_next),
                    ),
                    const SizedBox(width: 8),
                    if (showInlineVolume)
                      _VolumeControlInline(
                        volume: playback.volume,
                        onChanged: (v) => ref
                            .read(playbackControllerProvider.notifier)
                            .setVolume(v),
                        tooltip: l10n.tooltipVolume,
                      )
                    else
                      IconButton(
                        tooltip: l10n.tooltipVolume,
                        icon: const Icon(Icons.volume_up),
                        onPressed: () async {
                          final controller = ref.read(
                            playbackControllerProvider.notifier,
                          );
                          var v = ref.read(playbackControllerProvider).volume;
                          await showDialog<void>(
                            context: context,
                            builder: (context) => AlertDialog(
                              title: Text(l10n.volume),
                              content: StatefulBuilder(
                                builder: (context, setState) {
                                  return Row(
                                    children: [
                                      const Icon(Icons.volume_up),
                                      const SizedBox(width: 12),
                                      Expanded(
                                        child: Slider(
                                          value: v,
                                          onChanged: (nv) {
                                            setState(() => v = nv);
                                            controller.setVolume(nv);
                                          },
                                        ),
                                      ),
                                    ],
                                  );
                                },
                              ),
                              actions: [
                                TextButton(
                                  onPressed: () => Navigator.of(context).pop(),
                                  child: Text(
                                    MaterialLocalizations.of(
                                      context,
                                    ).closeButtonLabel,
                                  ),
                                ),
                              ],
                            ),
                          );
                        },
                      ),
                    IconButton(
                      tooltip: playModeLabel,
                      onPressed: () => ref
                          .read(queueControllerProvider.notifier)
                          .cyclePlayMode(),
                      icon: Icon(
                        switch (queue.playMode) {
                          PlayMode.sequential => Icons.playlist_play,
                          PlayMode.shuffle => Icons.shuffle,
                          PlayMode.repeatAll => Icons.repeat,
                          PlayMode.repeatOne => Icons.repeat_one,
                        },
                        color: queue.playMode == PlayMode.sequential
                            ? null
                            : theme.colorScheme.primary,
                      ),
                    ),
                    IconButton(
                      tooltip: selectedRenderer == null
                          ? 'DLNA'
                          : 'DLNA: ${selectedRenderer.friendlyName}',
                      onPressed: () async {
                        final chosen = await showDialog<_DlnaActionResult>(
                          context: context,
                          builder: (context) =>
                              _DlnaDialog(selected: selectedRenderer),
                        );
                        if (chosen == null) return;

                        if (chosen.applySelection) {
                          ref
                                  .read(dlnaSelectedRendererProvider.notifier)
                                  .state =
                              chosen.selected;
                        }

                        final message = chosen.message;
                        if (message != null && context.mounted) {
                          ScaffoldMessenger.of(
                            context,
                          ).showSnackBar(SnackBar(content: Text(message)));
                        }
                      },
                      icon: Icon(
                        Icons.cast,
                        color: selectedRenderer == null
                            ? null
                            : theme.colorScheme.primary,
                      ),
                    ),
                    const SizedBox(width: 12),
                  ],
                ),
                Positioned(
                  left: 0,
                  right: 0,
                  top: 0,
                  child: _NowPlayingProgressBar(
                    durationMs: queue.currentItem?.durationMs,
                    positionMs: playback.positionMs,
                    enabled:
                        queue.currentItem != null &&
                        playback.currentPath != null &&
                        playback.currentPath!.isNotEmpty,
                    playerState: playback.playerState,
                    onSeekMs: (ms) => ref
                        .read(playbackControllerProvider.notifier)
                        .seekMs(ms),
                  ),
                ),
              ],
            );
          },
        ),
      ),
    );
  }

  static String _formatMs(int ms) {
    final totalSeconds = (ms / 1000).floor();
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }
}

class _NowPlayingProgressBar extends StatefulWidget {
  const _NowPlayingProgressBar({
    required this.durationMs,
    required this.positionMs,
    required this.enabled,
    required this.playerState,
    required this.onSeekMs,
  });

  final int? durationMs;
  final int positionMs;
  final bool enabled;
  final PlayerState playerState;
  final ValueChanged<int> onSeekMs;

  @override
  State<_NowPlayingProgressBar> createState() => _NowPlayingProgressBarState();
}

class _NowPlayingProgressBarState extends State<_NowPlayingProgressBar>
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
  void didUpdateWidget(covariant _NowPlayingProgressBar oldWidget) {
    super.didUpdateWidget(oldWidget);
    final wasPlaying =
        oldWidget.playerState == PlayerState.playing ||
        oldWidget.playerState == PlayerState.buffering;
    final isPlaying =
        widget.playerState == PlayerState.playing ||
        widget.playerState == PlayerState.buffering;

    final durationChanged = oldWidget.durationMs != widget.durationMs;
    final posChanged = oldWidget.positionMs != widget.positionMs;
    final enabledChanged = oldWidget.enabled != widget.enabled;

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

    if (widget.playerState == PlayerState.stopped) {
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
        if (d > 0 &&
            (widget.playerState == PlayerState.playing ||
                widget.playerState == PlayerState.buffering)) {
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
      if (durationMs > 0 &&
          (widget.playerState == PlayerState.playing ||
              widget.playerState == PlayerState.buffering)) {
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
    if (!widget.enabled || d == null || d <= 0) return false;
    return widget.playerState == PlayerState.playing ||
        widget.playerState == PlayerState.buffering;
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
    if (!widget.enabled || d == null || d <= 0) return 0;
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
      if (widget.playerState == PlayerState.playing ||
          widget.playerState == PlayerState.buffering) {
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
            ? '${NowPlayingBar._formatMs(posMs)} / ${NowPlayingBar._formatMs(d)}'
            : NowPlayingBar._formatMs(widget.positionMs);

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
                      child: _MeasureSize(
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

    final trackHeightTarget = _emphasized ? 6.0 : 3.0;
    final barHeightTarget = _emphasized ? 14.0 : 6.0;
    const baseCenterY = 3.0;

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
              ? '${NowPlayingBar._formatMs(widget.positionMs)} / ${NowPlayingBar._formatMs(d)}'
              : NowPlayingBar._formatMs(widget.positionMs);

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
                backgroundColor: theme.colorScheme.onSurface.withValues(
                  alpha: 0.08,
                ),
                color: theme.colorScheme.primary,
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
                      painter: _ProgressPainter(
                        progress: _controller.value,
                        trackHeight: trackH,
                        centerY: baseCenterY,
                        trackColor: theme.colorScheme.onSurface.withValues(
                          alpha: 0.10,
                        ),
                        fillColor: theme.colorScheme.primary,
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
                _showTooltipOverlay();
              },
              onExit: (_) {
                setState(() => _hovering = false);
                if (!_dragging) _hideTooltipOverlay();
              },
              child: GestureDetector(
                behavior: HitTestBehavior.translucent,
                onTapDown: widget.enabled && d > 0
                    ? (details) {
                        _showTooltipOverlay();
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
                        _showTooltipOverlay();
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

class _ProgressPainter extends CustomPainter {
  const _ProgressPainter({
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
    final r = RRect.fromRectAndRadius(
      Rect.fromLTWH(0, y, size.width, trackH),
      const Radius.circular(999),
    );
    canvas.drawRRect(r, Paint()..color = trackColor);

    final w = (size.width * progress).clamp(0.0, size.width);
    if (w <= 0) return;
    final fr = RRect.fromRectAndRadius(
      Rect.fromLTWH(0, y, w, trackH),
      const Radius.circular(999),
    );
    canvas.drawRRect(fr, Paint()..color = fillColor);
  }

  @override
  bool shouldRepaint(covariant _ProgressPainter oldDelegate) {
    return oldDelegate.progress != progress ||
        oldDelegate.trackHeight != trackHeight ||
        oldDelegate.centerY != centerY ||
        oldDelegate.trackColor != trackColor ||
        oldDelegate.fillColor != fillColor;
  }
}

class _MeasureSize extends SingleChildRenderObjectWidget {
  const _MeasureSize({required this.onChange, required super.child});

  final ValueChanged<Size> onChange;

  @override
  RenderObject createRenderObject(BuildContext context) {
    return _RenderMeasureSize(onChange);
  }

  @override
  void updateRenderObject(
    BuildContext context,
    covariant _RenderMeasureSize renderObject,
  ) {
    renderObject.onChange = onChange;
  }
}

class _RenderMeasureSize extends RenderProxyBox {
  _RenderMeasureSize(this.onChange);

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

class _CoverPlaceholder extends StatelessWidget {
  const _CoverPlaceholder({required this.color});

  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 48,
      height: 48,
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: color.withValues(alpha: 0.18)),
      ),
      child: Icon(Icons.music_note, color: color),
    );
  }
}

class _VolumeControlInline extends StatelessWidget {
  const _VolumeControlInline({
    required this.volume,
    required this.onChanged,
    required this.tooltip,
  });

  final double volume;
  final ValueChanged<double> onChanged;
  final String tooltip;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Tooltip(message: tooltip, child: const Icon(Icons.volume_up)),
        SizedBox(
          width: 120,
          child: SliderTheme(
            data: SliderTheme.of(context).copyWith(
              trackHeight: 2,
              overlayShape: SliderComponentShape.noOverlay,
            ),
            child: Slider(value: volume.clamp(0.0, 1.0), onChanged: onChanged),
          ),
        ),
        const SizedBox(width: 8),
      ],
    );
  }
}

class _DlnaActionResult {
  const _DlnaActionResult({
    this.applySelection = false,
    this.selected,
    this.message,
  });

  final bool applySelection;
  final DlnaRenderer? selected;
  final String? message;
}

class _DlnaDialog extends StatefulWidget {
  const _DlnaDialog({required this.selected});

  final DlnaRenderer? selected;

  @override
  State<_DlnaDialog> createState() => _DlnaDialogState();
}

class _DlnaDialogState extends State<_DlnaDialog> {
  late Future<List<DlnaRenderer>> _future;
  DlnaRenderer? _selected;

  @override
  void initState() {
    super.initState();
    _selected = widget.selected;
    _future = const DlnaBridge().discoverRenderers(
      timeout: const Duration(milliseconds: 1200),
    );
  }

  void _refresh() {
    setState(() {
      _future = const DlnaBridge().discoverRenderers(
        timeout: const Duration(milliseconds: 1200),
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    final okLabel = MaterialLocalizations.of(context).okButtonLabel;
    final cancelLabel = MaterialLocalizations.of(context).cancelButtonLabel;
    final screenH = MediaQuery.sizeOf(context).height;
    final listHeight = (screenH * 0.45).clamp(260.0, 420.0);

    return Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(24, 20, 24, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Row(
                children: [
                  Icon(Icons.cast, color: theme.colorScheme.primary),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text('DLNA', style: theme.textTheme.titleLarge),
                  ),
                  IconButton(
                    tooltip: '刷新',
                    onPressed: _refresh,
                    icon: const Icon(Icons.refresh),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  '输出设备',
                  style: theme.textTheme.labelMedium?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
              const SizedBox(height: 12),
              SizedBox(
                height: listHeight,
                child: FutureBuilder<List<DlnaRenderer>>(
                  future: _future,
                  builder: (context, snapshot) {
                    final data = snapshot.data;
                    if (snapshot.connectionState != ConnectionState.done) {
                      return const Center(child: CircularProgressIndicator());
                    }
                    if (snapshot.hasError) {
                      return _DlnaEmptyState(
                        icon: Icons.error_outline,
                        title: '发现失败',
                        subtitle: '${snapshot.error}',
                        onRetry: _refresh,
                      );
                    }

                    final devices = data ?? const [];
                    if (devices.isEmpty) {
                      return _DlnaEmptyState(
                        icon: Icons.wifi_off,
                        title: '未发现 DLNA 设备',
                        subtitle: '请确保设备与本机在同一局域网内，并关闭可能拦截组播的代理/VPN。',
                        onRetry: _refresh,
                      );
                    }

                    return Container(
                      decoration: BoxDecoration(
                        color: theme.colorScheme.surfaceContainerLow,
                        borderRadius: BorderRadius.circular(16),
                        border: Border.all(
                          color: theme.colorScheme.outlineVariant,
                        ),
                      ),
                      clipBehavior: Clip.antiAlias,
                      child: ListView.separated(
                        shrinkWrap: true,
                        itemCount: devices.length + 1,
                        separatorBuilder: (context, index) =>
                            const Divider(height: 1),
                        itemBuilder: (context, i) {
                          if (i == 0) {
                            final selected = _selected == null;
                            return ListTile(
                              dense: true,
                              leading: const Icon(Icons.computer),
                              title: const Text('本机'),
                              subtitle: const Text('本地输出'),
                              trailing: selected
                                  ? Icon(
                                      Icons.check_circle,
                                      color: theme.colorScheme.primary,
                                    )
                                  : null,
                              selected: selected,
                              onTap: () => setState(() => _selected = null),
                            );
                          }

                          final d = devices[i - 1];
                          final ok = d.avTransportControlUrl != null;
                          final selected = _selected?.usn == d.usn;
                          final volOk = d.renderingControlUrl != null;
                          final subtitle = ok
                              ? (volOk ? null : '不支持音量控制')
                              : '不支持 AVTransport（无法播放）';
                          return ListTile(
                            dense: true,
                            enabled: ok,
                            leading: const Icon(Icons.speaker),
                            title: Text(d.friendlyName),
                            subtitle: subtitle == null ? null : Text(subtitle),
                            trailing: selected
                                ? Icon(
                                    Icons.check_circle,
                                    color: theme.colorScheme.primary,
                                  )
                                : null,
                            selected: selected,
                            onTap: ok
                                ? () => setState(() => _selected = d)
                                : null,
                          );
                        },
                      ),
                    );
                  },
                ),
              ),
              const SizedBox(height: 16),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: Text(cancelLabel),
                  ),
                  const SizedBox(width: 8),
                  FilledButton(
                    onPressed: () {
                      final d = _selected;
                      if (d == null) {
                        Navigator.of(context).pop(
                          _DlnaActionResult(
                            applySelection: true,
                            selected: null,
                            message: '已切换到本地输出',
                          ),
                        );
                        return;
                      }
                      if (d.avTransportControlUrl == null) {
                        Navigator.of(context).pop(
                          _DlnaActionResult(
                            applySelection: false,
                            message: '不支持 AVTransport（无法播放）',
                          ),
                        );
                        return;
                      }
                      Navigator.of(context).pop(
                        _DlnaActionResult(
                          applySelection: true,
                          selected: d,
                          message: '已选择 DLNA：${d.friendlyName}',
                        ),
                      );
                    },
                    child: Text(okLabel),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _DlnaEmptyState extends StatelessWidget {
  const _DlnaEmptyState({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.onRetry,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 12),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 36, color: theme.colorScheme.onSurfaceVariant),
            const SizedBox(height: 12),
            Text(title, style: theme.textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              subtitle,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            FilledButton.tonalIcon(
              onPressed: onRetry,
              icon: const Icon(Icons.refresh),
              label: const Text('刷新'),
            ),
          ],
        ),
      ),
    );
  }
}
