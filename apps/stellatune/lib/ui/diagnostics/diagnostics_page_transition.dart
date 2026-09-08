import 'dart:io';

import 'package:animations/animations.dart';
import 'package:flutter/material.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/ui/widgets/window_controls_visibility.dart';

import 'log_page.dart';
import 'log_window_controls.dart';

/// Retain one navigator until the exit animation finishes, including when an
/// open request interrupts closing. The underlying application stays mounted.
class DiagnosticsPageTransition extends StatefulWidget {
  const DiagnosticsPageTransition({
    super.key,
    required this.service,
    required this.visible,
    required this.child,
  });

  final DiagnosticsService service;
  final bool visible;
  final Widget child;

  @override
  State<DiagnosticsPageTransition> createState() =>
      _DiagnosticsPageTransitionState();
}

class _DiagnosticsPageTransitionState extends State<DiagnosticsPageTransition>
    with SingleTickerProviderStateMixin {
  late final _controller = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 300),
    reverseDuration: const Duration(milliseconds: 250),
  )..addStatusListener(_onStatus);

  void _onStatus(AnimationStatus status) {
    if (status == AnimationStatus.dismissed) setState(() {});
  }

  void _animate() {
    if (MediaQuery.disableAnimationsOf(context)) {
      _controller.value = widget.visible ? 1 : 0;
    } else if (widget.visible) {
      _controller.forward();
    } else {
      _controller.reverse();
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _animate();
  }

  @override
  void didUpdateWidget(DiagnosticsPageTransition oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.visible != widget.visible) _animate();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final presented = widget.visible || !_controller.isDismissed;
    return ClipRect(
      child: Stack(
        fit: StackFit.expand,
        children: [
          IgnorePointer(
            ignoring: presented,
            child: ExcludeFocus(
              excluding: presented,
              child: SharedAxisTransition(
                key: const ValueKey('diagnostics-underlying-transition'),
                animation: kAlwaysCompleteAnimation,
                secondaryAnimation: _controller,
                transitionType: SharedAxisTransitionType.scaled,
                fillColor: Theme.of(context).colorScheme.surface,
                child: WindowControlsVisibility(
                  hidden: presented,
                  child: widget.child,
                ),
              ),
            ),
          ),
          if (presented) ...[
            // Prevent click-through while the departing page is still visible.
            const ModalBarrier(dismissible: false),
            IgnorePointer(
              ignoring: !widget.visible,
              child: ExcludeFocus(
                excluding: !widget.visible,
                child: SharedAxisTransition(
                  key: const ValueKey('diagnostics-page-transition'),
                  animation: _controller,
                  secondaryAnimation: kAlwaysDismissedAnimation,
                  transitionType: SharedAxisTransitionType.scaled,
                  fillColor: Colors.transparent,
                  child: HeroControllerScope.none(
                    child: Navigator(
                      onGenerateRoute: (_) => MaterialPageRoute<void>(
                        builder: (_) =>
                            DiagnosticsPage(service: widget.service),
                      ),
                    ),
                  ),
                ),
              ),
            ),
            if (Platform.isWindows || Platform.isLinux || Platform.isMacOS)
              Positioned(
                top: 12,
                right: 14,
                child: LogWindowControls(chinese: widget.service.chinese),
              ),
          ],
        ],
      ),
    );
  }
}
