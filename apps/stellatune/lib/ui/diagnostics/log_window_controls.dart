import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'package:stellatune/ui/widgets/desktop_window_controls.dart';

class LogWindowControls extends StatelessWidget {
  const LogWindowControls({super.key, required this.chinese});

  final bool chinese;

  @override
  Widget build(BuildContext context) => Material(
    type: MaterialType.transparency,
    child: DesktopWindowControls(
      chinese: chinese,
      foreground: Theme.of(context).colorScheme.onSurface,
      onMinimize: () => windowManager.minimize(),
      onMaximize: () async {
        if (await windowManager.isMaximized()) {
          await windowManager.restore();
        } else {
          await windowManager.maximize();
        }
      },
      onClose: () => windowManager.close(),
    ),
  );
}
