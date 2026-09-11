import 'dart:async';

import 'package:flutter/material.dart';

/// Keep the seek bar mounted, showing busy feedback only for a sustained wait.
class NowPlayingTransitionProgress extends StatefulWidget {
  const NowPlayingTransitionProgress({
    super.key,
    required this.busy,
    required this.child,
  });
  final bool busy;
  final Widget child;
  @override
  State<NowPlayingTransitionProgress> createState() =>
      _NowPlayingTransitionProgressState();
}

class _NowPlayingTransitionProgressState
    extends State<NowPlayingTransitionProgress> {
  Timer? timer;
  bool visible = false;
  void schedule() {
    timer = Timer(const Duration(milliseconds: 300), () {
      if (mounted && widget.busy) setState(() => visible = true);
    });
  }

  @override
  void initState() {
    super.initState();
    if (widget.busy) schedule();
  }

  @override
  void didUpdateWidget(NowPlayingTransitionProgress oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.busy == widget.busy) return;
    timer?.cancel();
    visible = false;
    if (widget.busy) schedule();
  }

  @override
  void dispose() {
    timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Stack(
    children: [
      widget.child,
      if (visible)
        const Positioned(
          top: 0,
          left: 0,
          right: 0,
          child: IgnorePointer(child: LinearProgressIndicator(minHeight: 3)),
        ),
    ],
  );
}
