import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';

/// A text widget that scrolls horizontally in a loop when the text overflows.
class MarqueeText extends StatefulWidget {
  final String text;
  final TextStyle? style;
  final Duration pauseDuration;
  final Curve scrollCurve;
  final double pixelsPerSecond;
  final int? maxLines;
  final TextAlign textAlign;
  final double gap;

  const MarqueeText({
    super.key,
    required this.text,
    this.style,
    this.pauseDuration = const Duration(seconds: 2),
    this.scrollCurve = Curves.linear,
    this.pixelsPerSecond = 30.0,
    this.maxLines = 1,
    this.textAlign = TextAlign.start,
    this.gap = 20.0,
  });

  @override
  State<MarqueeText> createState() => _MarqueeTextState();
}

class _MarqueeTextState extends State<MarqueeText>
    with SingleTickerProviderStateMixin {
  final ScrollController _scrollController = ScrollController();
  late final Ticker _ticker;
  bool _isScrolling = false;
  bool _startScheduled = false;
  bool _stopScheduled = false;
  bool _shouldScroll = false;
  double _textWidth = 0;
  double _scrollDistance = 0;
  DateTime? _scrollStartTime;
  Duration _currentPauseDuration = Duration.zero;
  bool _isPaused = true;

  @override
  void initState() {
    super.initState();
    _ticker = createTicker(_onTick);
  }

  @override
  void didUpdateWidget(MarqueeText oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.text != widget.text) {
      _resetScroll();
    }
  }

  @override
  void dispose() {
    if (_ticker.isActive) {
      _ticker.stop();
    }
    _ticker.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  void _resetScroll() {
    _stopScroll();
    _isPaused = true;
    _scrollStartTime = null;
    if (_scrollController.hasClients) {
      _scrollController.jumpTo(0);
    }
  }

  void _onTick(Duration elapsed) {
    if (!mounted || !_shouldScroll || !_scrollController.hasClients) return;

    final maxExtent = _scrollController.position.maxScrollExtent;
    if (maxExtent <= 0) return;

    if (_isPaused) {
      // Handle pause at start/end
      if (_scrollStartTime == null) {
        _scrollStartTime = DateTime.now();
        _currentPauseDuration = widget.pauseDuration;
      }
      final pauseElapsed = DateTime.now().difference(_scrollStartTime!);
      if (pauseElapsed >= _currentPauseDuration) {
        _isPaused = false;
        _scrollStartTime = DateTime.now();
      }
      return;
    }

    // Calculate scroll progress
    final scrollDuration = _scrollDistance / widget.pixelsPerSecond;
    final scrollElapsed =
        DateTime.now().difference(_scrollStartTime!).inMilliseconds / 1000.0;
    final progress = (scrollElapsed / scrollDuration).clamp(0.0, 1.0);
    final targetOffset = progress * _scrollDistance;

    if (_scrollController.hasClients) {
      _scrollController.jumpTo(targetOffset.clamp(0.0, maxExtent));
    }

    // Check if we've completed the scroll
    if (progress >= 1.0) {
      // Jump back to start and pause
      if (_scrollController.hasClients) {
        _scrollController.jumpTo(0);
      }
      _isPaused = true;
      _scrollStartTime = null;
    }
  }

  void _startScrollIfNeeded() {
    _startScheduled = false;
    if (!mounted || !_shouldScroll || _isScrolling || _ticker.isActive) return;
    _ticker.start();
    _isScrolling = true;
    _isPaused = true;
    _scrollStartTime = null;
  }

  void _stopScroll() {
    _stopScheduled = false;
    if (!mounted) return;
    if (_ticker.isActive) {
      _ticker.stop();
    }
    _isScrolling = false;
    if (_scrollController.hasClients) {
      _scrollController.jumpTo(0);
    }
  }

  @override
  Widget build(BuildContext context) {
    final defaultStyle = DefaultTextStyle.of(context).style;
    final effectiveStyle = defaultStyle.merge(widget.style);

    return LayoutBuilder(
      builder: (context, constraints) {
        // Measure text width
        final span = TextSpan(text: widget.text, style: effectiveStyle);
        final tp = TextPainter(
          text: span,
          maxLines: 1,
          textDirection: Directionality.of(context),
        )..layout();

        _textWidth = tp.width;
        final textOverflows = _textWidth > constraints.maxWidth;
        _scrollDistance = _textWidth + widget.gap;

        if (!textOverflows) {
          _shouldScroll = false;
          if (_isScrolling && !_stopScheduled) {
            _stopScheduled = true;
            WidgetsBinding.instance.addPostFrameCallback((_) => _stopScroll());
          }
          return Text(
            widget.text,
            style: widget.style,
            maxLines: widget.maxLines,
            textAlign: widget.textAlign,
            overflow: TextOverflow.ellipsis,
          );
        }

        _shouldScroll = true;
        if (!_isScrolling && !_startScheduled) {
          _startScheduled = true;
          WidgetsBinding.instance.addPostFrameCallback(
            (_) => _startScrollIfNeeded(),
          );
        }

        return ClipRect(
          child: SingleChildScrollView(
            controller: _scrollController,
            scrollDirection: Axis.horizontal,
            physics: const NeverScrollableScrollPhysics(),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  widget.text,
                  style: widget.style,
                  maxLines: widget.maxLines,
                  overflow: TextOverflow.clip,
                ),
                SizedBox(width: widget.gap),
                Text(
                  widget.text,
                  style: widget.style,
                  maxLines: widget.maxLines,
                  overflow: TextOverflow.clip,
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
