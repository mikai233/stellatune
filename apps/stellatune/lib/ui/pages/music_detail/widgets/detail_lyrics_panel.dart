import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/lyrics/lyrics_controller.dart';

class DetailLyricsPanel extends ConsumerWidget {
  const DetailLyricsPanel({super.key, required this.foregroundColor});
  final Color foregroundColor;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final lyrics = ref.watch(
      lyricsControllerProvider.select(
        (state) => (lines: state.lines, index: state.currentLineIndex),
      ),
    );
    return _LyricsPanel(
      lines: lyrics.lines,
      currentLineIndex: lyrics.index,
      foregroundColor: foregroundColor,
    );
  }
}

class _LyricsPanel extends StatefulWidget {
  const _LyricsPanel({
    required this.lines,
    required this.currentLineIndex,
    required this.foregroundColor,
  });

  final List<LyricLine> lines;
  final int currentLineIndex;
  final Color foregroundColor;

  @override
  State<_LyricsPanel> createState() => _LyricsPanelState();
}

class _LyricsPanelState extends State<_LyricsPanel> {
  final ScrollController _controller = ScrollController();
  final Map<int, GlobalKey> _lineKeys = <int, GlobalKey>{};

  @override
  void didUpdateWidget(covariant _LyricsPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.lines.length != oldWidget.lines.length) {
      _lineKeys.removeWhere((k, _) => k >= widget.lines.length);
    }
    if (widget.currentLineIndex != oldWidget.currentLineIndex) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _scrollToCurrentLine();
      });
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _scrollToCurrentLine() {
    if (!mounted) return;
    final idx = widget.currentLineIndex;
    if (idx < 0 || idx >= widget.lines.length) return;
    final key = _lineKeys[idx];
    final ctx = key?.currentContext;
    if (ctx == null) return;
    Scrollable.ensureVisible(
      ctx,
      alignment: 0.32,
      duration: const Duration(milliseconds: 260),
      curve: Curves.easeOutCubic,
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final shortestSide = MediaQuery.sizeOf(context).shortestSide;
    final inactiveFontSize = shortestSide < 420
        ? 17.0
        : shortestSide < 700
        ? 18.0
        : 19.0;
    final activeFontSize = inactiveFontSize + 3.0;
    return ListView.builder(
      controller: _controller,
      itemCount: widget.lines.length,
      itemBuilder: (context, index) {
        final line = widget.lines[index];
        final active = index == widget.currentLineIndex;
        final key = _lineKeys.putIfAbsent(index, GlobalKey.new);
        final style =
            (active ? theme.textTheme.titleMedium : theme.textTheme.bodyLarge)
                ?.copyWith(
                  color: widget.foregroundColor.withValues(
                    alpha: active ? 0.96 : 0.58,
                  ),
                  fontWeight: active ? FontWeight.w700 : FontWeight.w500,
                  fontSize: active ? activeFontSize : inactiveFontSize,
                  height: 1.32,
                );

        return Container(
          key: key,
          padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 6),
          child: AnimatedDefaultTextStyle(
            duration: const Duration(milliseconds: 180),
            curve: Curves.easeOut,
            style: style ?? TextStyle(fontSize: inactiveFontSize, height: 1.32),
            child: Text(
              line.text,
              maxLines: 3,
              softWrap: true,
              overflow: TextOverflow.ellipsis,
              textAlign: TextAlign.center,
            ),
          ),
        );
      },
    );
  }
}
