import 'package:flutter/widgets.dart';

/// Page transitions draw stationary window controls above the moving content.
class WindowControlsVisibility extends InheritedWidget {
  const WindowControlsVisibility({
    super.key,
    required this.hidden,
    required super.child,
  });

  final bool hidden;

  static bool hiddenOf(BuildContext context) =>
      context
          .dependOnInheritedWidgetOfExactType<WindowControlsVisibility>()
          ?.hidden ??
      false;

  @override
  bool updateShouldNotify(WindowControlsVisibility oldWidget) =>
      oldWidget.hidden != hidden;
}
