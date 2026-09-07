import 'package:flutter/material.dart';

class SettingsSectionCard extends StatelessWidget {
  const SettingsSectionCard({
    super.key,
    required this.title,
    required this.children,
    this.trailing,
    this.padding = const EdgeInsets.all(16),
    this.headerBottomSpacing = 12,
    this.icon = Icons.tune_rounded,
    this.subtitle,
  });

  final String title;
  final IconData icon;
  final String? subtitle;
  final List<Widget> children;
  final Widget? trailing;
  final EdgeInsetsGeometry padding;
  final double headerBottomSpacing;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Material(
      color: theme.colorScheme.surface.withValues(alpha: .82),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(
          color: theme.colorScheme.onSurface.withValues(alpha: .10),
        ),
      ),
      child: Padding(
        padding: padding,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(icon, size: 32, color: theme.colorScheme.primary),
                const SizedBox(width: 14),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        title,
                        style: TextStyle(
                          fontSize: 19,
                          fontWeight: FontWeight.w600,
                          color: theme.colorScheme.onSurface,
                        ),
                      ),
                      if (subtitle != null) ...[
                        const SizedBox(height: 3),
                        Text(
                          subtitle!,
                          style: TextStyle(
                            fontSize: 12,
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
                ?trailing,
              ],
            ),
            if (children.isNotEmpty) SizedBox(height: headerBottomSpacing),
            Material(
              color: theme.colorScheme.onSurface.withValues(alpha: .025),
              borderRadius: BorderRadius.circular(10),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: children,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
