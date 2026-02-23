import 'dart:async';

import 'package:animations/animations.dart';
import 'package:flutter/material.dart';

class VolumePopupButton extends StatefulWidget {
  const VolumePopupButton({
    super.key,
    required this.volume,
    required this.onChanged,
    this.foregroundColor,
    this.iconSize = 24.0,
    this.buttonSize = 48.0,
    this.enableHover = false,
    this.onToggleMute,
  });

  final double volume;
  final ValueChanged<double> onChanged;
  final Color? foregroundColor;
  final double iconSize;
  final double buttonSize;
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
    final visualIconSize = iconData == Icons.volume_down
        ? widget.iconSize + 2
        : widget.iconSize;

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
          iconSize: visualIconSize,
          constraints: BoxConstraints.tightFor(
            width: widget.buttonSize,
            height: widget.buttonSize,
          ),
          color: color,
          onPressed: _onPressed,
        ),
      ),
    );
  }
}
