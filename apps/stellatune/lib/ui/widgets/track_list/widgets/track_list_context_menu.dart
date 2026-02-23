import 'package:animations/animations.dart';
import 'package:flutter/material.dart';
import 'package:stellatune/ui/widgets/track_list/models/track_list_models.dart';

List<PopupMenuEntry<TrackListAction>> buildTrackListActionMenuItems(
  List<TrackListActionSpec> items,
) {
  return items
      .map((item) {
        return PopupMenuItem<TrackListAction>(
          value: item.action,
          enabled: item.enabled,
          child: Row(
            children: [
              Icon(item.icon, size: 18),
              const SizedBox(width: 10),
              Expanded(child: Text(item.label)),
            ],
          ),
        );
      })
      .toList(growable: false);
}

class TrackListContextMenuCard extends StatelessWidget {
  const TrackListContextMenuCard({
    super.key,
    required this.animation,
    required this.items,
    required this.onSelected,
  });

  final Animation<double> animation;
  final List<TrackListActionSpec> items;
  final ValueChanged<TrackListAction> onSelected;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final iconColor = theme.colorScheme.onSurfaceVariant;
    final menuColor = Color.alphaBlend(
      theme.colorScheme.primary.withValues(alpha: 0.025),
      theme.colorScheme.surface,
    );
    final enabledTextStyle = theme.textTheme.bodyMedium;
    final disabledTextStyle = enabledTextStyle?.copyWith(
      color: theme.colorScheme.onSurface.withValues(alpha: 0.42),
    );
    final menuAnimation = animation;
    final baseMenu = Material(
      elevation: 14,
      shadowColor: Colors.black.withValues(alpha: 0.18),
      color: menuColor,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(
          color: theme.colorScheme.outlineVariant.withValues(alpha: 0.42),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final item in items) ...[
              if (item.showDividerBefore)
                Divider(
                  height: 9,
                  thickness: 0.8,
                  color: theme.colorScheme.outlineVariant.withValues(
                    alpha: 0.60,
                  ),
                ),
              InkWell(
                onTap: item.enabled ? () => onSelected(item.action) : null,
                hoverColor: theme.colorScheme.primary.withValues(alpha: 0.08),
                highlightColor: theme.colorScheme.primary.withValues(
                  alpha: 0.12,
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 10,
                  ),
                  child: Row(
                    children: [
                      Icon(
                        item.icon,
                        size: 19,
                        color: item.enabled
                            ? iconColor
                            : iconColor.withValues(alpha: 0.36),
                      ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Text(
                          item.label,
                          style: item.enabled
                              ? enabledTextStyle
                              : disabledTextStyle,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
    );

    return FadeScaleTransition(
      animation: menuAnimation,
      child: AnimatedBuilder(
        animation: menuAnimation,
        child: baseMenu,
        builder: (context, child) {
          final dy = (1 - menuAnimation.value) * 8;
          return Transform.translate(offset: Offset(0, dy), child: child);
        },
      ),
    );
  }
}
