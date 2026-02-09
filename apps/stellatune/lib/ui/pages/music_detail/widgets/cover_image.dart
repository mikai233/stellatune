import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:stellatune/player/queue_models.dart';

/// Cover image with placeholder fallback.
class CoverImage extends StatelessWidget {
  const CoverImage({
    super.key,
    required this.coverDir,
    required this.trackId,
    this.cover,
    required this.size,
  });

  final String coverDir;
  final int? trackId;
  final QueueCover? cover;
  final double size;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final dpr = MediaQuery.devicePixelRatioOf(context).clamp(1.0, 2.0);
    final decodeSize = (size * dpr).round().clamp(256, 896);

    final placeholder = Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(16),
        color: theme.colorScheme.primary.withValues(alpha: 0.10),
        border: Border.all(
          color: theme.colorScheme.primary.withValues(alpha: 0.18),
        ),
        boxShadow: [
          BoxShadow(
            color: theme.shadowColor.withValues(alpha: 0.15),
            blurRadius: 24,
            offset: const Offset(0, 8),
          ),
        ],
      ),
      child: Icon(
        Icons.music_note,
        size: size * 0.4,
        color: theme.colorScheme.primary,
      ),
    );

    if (trackId == null && cover == null) return placeholder;

    final useLocal = trackId != null && coverDir.trim().isNotEmpty;
    final localProvider = useLocal
        ? ResizeImage(
            FileImage(File('$coverDir${Platform.pathSeparator}$trackId')),
            width: decodeSize,
            height: decodeSize,
            allowUpscaling: false,
          )
        : null;

    return Container(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(16),
        boxShadow: [
          BoxShadow(
            color: theme.shadowColor.withValues(alpha: 0.2),
            blurRadius: 32,
            offset: const Offset(0, 12),
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(16),
        child:
            localProvider != null
                ? Image(
                  image: localProvider,
                  width: size,
                  height: size,
                  fit: BoxFit.cover,
                  gaplessPlayback: true,
                  errorBuilder:
                      (context, error, stackTrace) => _buildByCoverOrPlaceholder(
                        cover: cover,
                        size: size,
                        placeholder: placeholder,
                      ),
                )
                : _buildByCoverOrPlaceholder(
                  cover: cover,
                  size: size,
                  placeholder: placeholder,
                ),
      ),
    );
  }

  Widget _buildByCoverOrPlaceholder({
    required QueueCover? cover,
    required double size,
    required Widget placeholder,
  }) {
    if (cover == null) return placeholder;
    switch (cover.kind) {
      case QueueCoverKind.url:
        return Image.network(
          cover.value,
          width: size,
          height: size,
          fit: BoxFit.cover,
          gaplessPlayback: true,
          errorBuilder: (context, error, stackTrace) => placeholder,
        );
      case QueueCoverKind.file:
        return Image.file(
          File(cover.value),
          width: size,
          height: size,
          fit: BoxFit.cover,
          gaplessPlayback: true,
          errorBuilder: (context, error, stackTrace) => placeholder,
        );
      case QueueCoverKind.data:
        final bytes = _decodeCoverBytes(cover.value);
        if (bytes == null) return placeholder;
        return Image.memory(
          bytes,
          width: size,
          height: size,
          fit: BoxFit.cover,
          gaplessPlayback: true,
          errorBuilder: (context, error, stackTrace) => placeholder,
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

/// A switcher that synchronizes the movement of two children to create
/// a perfectly coordinated carousel/film-strip effect.
class SyncedTransformSwitcher extends StatefulWidget {
  const SyncedTransformSwitcher({
    super.key,
    required this.child,
    required this.slideOffset, // e.g. 1.2 or -1.2
    required this.moveScale,
    this.duration = const Duration(milliseconds: 550),
    this.crossFade = false,
  });

  final Widget child;
  final double slideOffset;
  final double moveScale;
  final Duration duration;
  final bool crossFade;

  @override
  State<SyncedTransformSwitcher> createState() =>
      _SyncedTransformSwitcherState();
}

class _SyncedTransformSwitcherState extends State<SyncedTransformSwitcher>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  late Animation<double> _animation;
  Widget? _lastChild;
  double _lastSlideOffset = 0;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(vsync: this, duration: widget.duration);

    // Physics Model: Accelerating Collision
    // 1. Accelerate towards target (0.0 -> 0.50)
    // 2. High-velocity impact & overshoot (0.50 -> 0.65)
    // 3. Gradual snap back to center (0.65 -> 1.0)
    _animation = TweenSequence<double>([
      // 0.0 -> 0.50: Accelerating approach (easeIn for a punchy start)
      TweenSequenceItem(
        tween: Tween<double>(
          begin: 0.0,
          end: 1.0,
        ).chain(CurveTween(curve: Curves.easeIn)),
        weight: 50,
      ),
      // 0.50 -> 0.65: Momentum carry-over (Overshoot to 1.08)
      TweenSequenceItem(
        tween: Tween<double>(
          begin: 1.0,
          end: 1.08,
        ).chain(CurveTween(curve: Curves.easeOutCubic)),
        weight: 15,
      ),
      // 0.65 -> 1.0: Slower, viscous snap-back
      TweenSequenceItem(
        tween: Tween<double>(
          begin: 1.08,
          end: 1.0,
        ).chain(CurveTween(curve: Curves.easeOutBack)),
        weight: 35,
      ),
    ]).animate(_controller);

    // Initial state is finished
    _controller.value = 1.0;
  }

  @override
  void didUpdateWidget(SyncedTransformSwitcher oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.child.key != oldWidget.child.key) {
      _lastChild = oldWidget.child;
      _lastSlideOffset = widget.slideOffset;
      _controller.forward(from: 0.0);
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        final controllerValue = _controller.value;
        final animValue = _animation.value;
        final isFinished = controllerValue >= 1.0;
        final moveScale = widget.moveScale;

        // Incoming child follows the main animation sequence (glide + snap)
        final incomingMoveValue = 1.0 - animValue;
        final incomingOpacity = (controllerValue / 0.3).clamp(0.0, 1.0);

        // Outgoing child follows the glide until impact at 50%, then "launched"
        double outgoingMoveValue;
        if (controllerValue <= 0.50) {
          outgoingMoveValue = animValue;
        } else {
          // Explosive momentum transfer at 0.50
          final t = ((controllerValue - 0.50) / 0.50).clamp(0.0, 1.0);
          outgoingMoveValue = 1.0 + Curves.easeInExpo.transform(t) * 4.5;
        }

        return Stack(
          clipBehavior: Clip.none,
          alignment: Alignment.center,
          children: [
            // Outgoing child
            if (!isFinished && _lastChild != null)
              Transform.translate(
                offset: Offset(
                  -_lastSlideOffset * outgoingMoveValue * moveScale,
                  0,
                ),
                child: Opacity(
                  opacity: widget.crossFade
                      ? (1.0 - (controllerValue / 0.4)).clamp(0.0, 1.0)
                      : 1.0,
                  child: _lastChild,
                ),
              ),
            // Incoming child
            Transform.translate(
              offset: Offset(
                _lastSlideOffset * incomingMoveValue * moveScale,
                0,
              ),
              child: Opacity(
                opacity: widget.crossFade ? incomingOpacity : 1.0,
                child: widget.child,
              ),
            ),
          ],
        );
      },
    );
  }
}
