import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

class CustomTitleBar extends StatelessWidget {
  const CustomTitleBar({
    super.key,
    this.foregroundColor = Colors.white,
    this.backgroundColor = Colors.transparent,
    this.showTitle = true,
  });

  final Color foregroundColor;
  final Color backgroundColor;
  final bool showTitle;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 32,
      color: backgroundColor,
      child: Row(
        children: [
          Expanded(
            child: DragToMoveArea(
              child: Container(
                alignment: Alignment.centerLeft,
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: showTitle
                    ? Text(
                        'Stellatune',
                        style: TextStyle(
                          color: foregroundColor.withValues(alpha: 0.6),
                          fontSize: 12,
                          fontWeight: FontWeight.w500,
                        ),
                      )
                    : const SizedBox.shrink(),
              ),
            ),
          ),
          WindowButton(
            icon: Icons.minimize,
            onPressed: () => windowManager.minimize(),
            color: foregroundColor,
          ),
          WindowButton(
            icon: Icons.crop_square,
            onPressed: () async {
              if (await windowManager.isMaximized()) {
                windowManager.restore();
              } else {
                windowManager.maximize();
              }
            },
            color: foregroundColor,
          ),
          WindowButton(
            icon: Icons.close,
            onPressed: () => windowManager.close(),
            color: foregroundColor,
            isClose: true,
          ),
        ],
      ),
    );
  }
}

class WindowButton extends StatelessWidget {
  const WindowButton({
    super.key,
    required this.icon,
    required this.onPressed,
    required this.color,
    this.isClose = false,
  });

  final IconData icon;
  final VoidCallback onPressed;
  final Color color;
  final bool isClose;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 46,
      height: 32,
      child: InkWell(
        onTap: onPressed,
        hoverColor: isClose
            ? Colors.red.withValues(alpha: 0.8)
            : color.withValues(alpha: 0.1),
        child: Icon(icon, size: 16, color: color),
      ),
    );
  }
}
