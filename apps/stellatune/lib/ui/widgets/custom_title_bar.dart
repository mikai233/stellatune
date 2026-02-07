import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'package:stellatune/l10n/app_localizations.dart';

class CustomTitleBar extends StatelessWidget {
  const CustomTitleBar({
    super.key,
    this.foregroundColor = Colors.white,
    this.backgroundColor = Colors.transparent,
    this.showTitle = true,
    this.height = 32,
    this.leading,
    this.trailing,
  });

  final Color foregroundColor;
  final Color backgroundColor;
  final bool showTitle;
  final double height;
  final Widget? leading;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: height,
      color: backgroundColor,
      padding: const EdgeInsets.symmetric(horizontal: 4), // Window edge padding
      child: Row(
        children: [
          if (leading != null) leading!,
          Expanded(
            child: DragToMoveArea(
              child: Container(
                alignment: Alignment.centerLeft,
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: showTitle
                    ? Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(
                            Icons.music_note,
                            size: 16,
                            color: foregroundColor.withValues(alpha: 0.6),
                          ),
                          const SizedBox(width: 4),
                          Text(
                            AppLocalizations.of(context)!.appTitle,
                            style: TextStyle(
                              color: foregroundColor.withValues(alpha: 0.6),
                              fontSize: 14,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ],
                      )
                    : const SizedBox.shrink(),
              ),
            ),
          ),
          if (trailing != null) trailing!,
          const SizedBox(width: 4),
          WindowButton(
            icon: Icons.fullscreen,
            onPressed: () async {
              final isFullScreen = await windowManager.isFullScreen();
              await windowManager.setFullScreen(!isFullScreen);
            },
            color: foregroundColor,
            height: height,
            tooltip: AppLocalizations.of(context)!.tooltipFullscreen,
          ),
          WindowButton(
            icon: Icons.minimize,
            onPressed: () => windowManager.minimize(),
            color: foregroundColor,
            height: height,
            tooltip: AppLocalizations.of(context)!.tooltipMinimize,
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
            height: height,
            tooltip: AppLocalizations.of(context)!.tooltipMaximize,
          ),
          WindowButton(
            icon: Icons.close,
            onPressed: () => windowManager.close(),
            color: foregroundColor,
            isClose: true,
            height: height,
            tooltip: AppLocalizations.of(context)!.tooltipClose,
          ),
        ],
      ),
    );
  }
}

class TitleBarButton extends StatelessWidget {
  const TitleBarButton({
    super.key,
    required this.icon,
    required this.onPressed,
    required this.color,
    this.height = 32,
    this.tooltip,
  });

  final IconData icon;
  final VoidCallback onPressed;
  final Color color;
  final double height;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 4),
      child: Tooltip(
        message: tooltip ?? '',
        waitDuration: const Duration(milliseconds: 500),
        child: Container(
          decoration: BoxDecoration(borderRadius: BorderRadius.circular(8)),
          clipBehavior: Clip.antiAlias,
          child: Material(
            color: Colors.transparent,
            child: InkWell(
              onTap: onPressed,
              hoverColor: color.withValues(alpha: 0.15),
              child: SizedBox(
                width: 40,
                height: height - 8,
                child: Icon(icon, size: 18, color: color),
              ),
            ),
          ),
        ),
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
    this.height = 32,
    this.tooltip,
  });

  final IconData icon;
  final VoidCallback onPressed;
  final Color color;
  final bool isClose;
  final double height;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 1, vertical: 4),
      child: Tooltip(
        message: tooltip ?? '',
        waitDuration: const Duration(milliseconds: 500),
        child: Container(
          decoration: BoxDecoration(borderRadius: BorderRadius.circular(8)),
          clipBehavior: Clip.antiAlias,
          child: Material(
            color: Colors.transparent,
            child: InkWell(
              onTap: onPressed,
              hoverColor: isClose
                  ? Colors.red.withValues(alpha: 0.8)
                  : color.withValues(alpha: 0.15),
              child: SizedBox(
                width: 44,
                height: height - 8,
                child: Icon(icon, size: 18, color: color),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
