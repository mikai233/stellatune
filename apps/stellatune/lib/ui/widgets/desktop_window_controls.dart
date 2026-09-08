import 'package:flutter/material.dart';

import 'window_controls_visibility.dart';

/// Shared geometry and interaction styles for the shell and fullscreen pages.
class DesktopWindowControls extends StatelessWidget {
  const DesktopWindowControls({
    super.key,
    required this.foreground,
    this.chinese = true,
    this.onMinimize,
    this.onMaximize,
    this.onClose,
  });

  final Color foreground;
  final bool chinese;
  final VoidCallback? onMinimize, onMaximize, onClose;

  @override
  Widget build(BuildContext context) {
    Widget button(
      IconData icon,
      double size,
      String zh,
      String en,
      VoidCallback? action,
    ) => IconButton(
      tooltip: chinese ? zh : en,
      onPressed: action,
      style: IconButton.styleFrom(
        fixedSize: const Size(42, 34),
        minimumSize: const Size(42, 34),
        maximumSize: const Size(42, 34),
        padding: EdgeInsets.zero,
        visualDensity: VisualDensity.standard,
        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
        foregroundColor: foreground.withValues(alpha: .9),
        disabledForegroundColor: foreground.withValues(alpha: .35),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
      ),
      icon: Icon(icon, size: size),
    );
    return Visibility(
      visible: !WindowControlsVisibility.hiddenOf(context),
      maintainState: true,
      maintainAnimation: true,
      maintainSize: true,
      child: SizedBox(
        height: 39,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            button(Icons.remove, 19, '最小化', 'Minimize', onMinimize),
            button(
              Icons.crop_square_rounded,
              16,
              '最大化 / 还原',
              'Maximize / restore',
              onMaximize,
            ),
            button(Icons.close_rounded, 19, '关闭', 'Close window', onClose),
          ],
        ),
      ),
    );
  }
}
