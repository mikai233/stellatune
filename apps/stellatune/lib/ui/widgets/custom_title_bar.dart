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

class TitleBarButton extends StatefulWidget {
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
  State<TitleBarButton> createState() => _TitleBarButtonState();
}

class _TitleBarButtonState extends State<TitleBarButton> {
  bool _isHovered = false;

  @override
  Widget build(BuildContext context) {
    final borderColor = _isHovered
        ? widget.color.withValues(alpha: 0.35)
        : Colors.transparent;

    final borderWidth = _isHovered ? 1.5 : 0.0;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 4),
      child: Tooltip(
        message: widget.tooltip ?? '',
        waitDuration: const Duration(milliseconds: 500),
        child: MouseRegion(
          onEnter: (_) => setState(() => _isHovered = true),
          onExit: (_) => setState(() => _isHovered = false),
          cursor: SystemMouseCursors.click,
          child: GestureDetector(
            onTap: widget.onPressed,
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 240),
              curve: Curves.easeOutCubic,
              width: 40,
              height: widget.height - 8,
              decoration: BoxDecoration(
                color: _isHovered
                    ? widget.color.withValues(alpha: 0.12)
                    : Colors.transparent,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: borderColor, width: borderWidth),
              ),
              child: Icon(
                widget.icon,
                size: 18,
                color: widget.color.withValues(alpha: _isHovered ? 1.0 : 0.75),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class WindowButton extends StatefulWidget {
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
  State<WindowButton> createState() => _WindowButtonState();
}

class _WindowButtonState extends State<WindowButton> {
  bool _isHovered = false;

  @override
  Widget build(BuildContext context) {
    final activeColor = widget.isClose
        ? Colors.red.withValues(alpha: 1.0)
        : widget.color;

    final borderColor = _isHovered
        ? activeColor.withValues(alpha: widget.isClose ? 0.6 : 0.35)
        : Colors.transparent;

    final borderWidth = _isHovered ? 1.5 : 0.0;

    final backgroundColor = _isHovered
        ? (widget.isClose
              ? Colors.red.withValues(alpha: 0.15)
              : widget.color.withValues(alpha: 0.12))
        : Colors.transparent;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 1, vertical: 4),
      child: Tooltip(
        message: widget.tooltip ?? '',
        waitDuration: const Duration(milliseconds: 500),
        child: MouseRegion(
          onEnter: (_) => setState(() => _isHovered = true),
          onExit: (_) => setState(() => _isHovered = false),
          cursor: SystemMouseCursors.click,
          child: GestureDetector(
            onTap: widget.onPressed,
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 240),
              curve: Curves.easeOutCubic,
              width: 44,
              height: widget.height - 8,
              decoration: BoxDecoration(
                color: backgroundColor,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: borderColor, width: borderWidth),
              ),
              child: Icon(
                widget.icon,
                size: 18,
                color: widget.isClose && _isHovered
                    ? Colors.red
                    : widget.color.withValues(alpha: _isHovered ? 1.0 : 0.75),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
