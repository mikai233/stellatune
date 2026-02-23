import 'dart:io';

import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/audio_format_badge.dart';
import 'package:stellatune/ui/widgets/track_list/models/track_list_models.dart';
import 'package:stellatune/ui/widgets/track_list/widgets/track_list_shared_widgets.dart';

class TrackListTile extends StatelessWidget {
  const TrackListTile({
    super.key,
    required this.l10n,
    required this.index,
    required this.track,
    required this.coverDir,
    required this.deferHeavy,
    required this.selectionMode,
    required this.selected,
    required this.pressed,
    required this.isLiked,
    required this.isBlocked,
    required this.isDesktopPlatform,
    required this.onPressedDown,
    required this.onPressedUp,
    required this.onPressedCancel,
    required this.onToggleLike,
    required this.onToggleSelected,
    required this.onTapTrack,
    required this.onTrackAction,
    required this.buildTrackActionMenuItems,
    this.onDesktopContextMenuRequested,
    this.reorderIndex,
    this.blockedReason,
    this.onLongPressSelect,
  });

  final AppLocalizations l10n;
  final int index;
  final TrackLite track;
  final String coverDir;
  final bool deferHeavy;
  final bool selectionMode;
  final bool selected;
  final bool pressed;
  final bool isLiked;
  final bool isBlocked;
  final bool isDesktopPlatform;
  final String? blockedReason;
  final int? reorderIndex;
  final VoidCallback onPressedDown;
  final VoidCallback onPressedUp;
  final VoidCallback onPressedCancel;
  final VoidCallback onToggleLike;
  final VoidCallback onToggleSelected;
  final VoidCallback onTapTrack;
  final Future<void> Function(TrackListAction action) onTrackAction;
  final List<PopupMenuEntry<TrackListAction>> Function(BuildContext context)
  buildTrackActionMenuItems;
  final void Function(Offset globalPosition)? onDesktopContextMenuRequested;
  final VoidCallback? onLongPressSelect;

  @override
  Widget build(BuildContext context) {
    final title = (track.title ?? '').trim();
    final artist = (track.artist ?? '').trim();
    final album = (track.album ?? '').trim();
    final line1 = title.isNotEmpty ? title : _basename(track.path);
    final line2 = [artist, album].where((s) => s.isNotEmpty).join(' • ');
    final coverPath = '$coverDir${Platform.pathSeparator}${track.id}';

    final theme = Theme.of(context);
    final rowBg = selected
        ? theme.colorScheme.secondaryContainer.withValues(alpha: 0.92)
        : theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.28);
    final rowBorder = selected
        ? Border.all(color: theme.colorScheme.secondary.withValues(alpha: 0.70))
        : null;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 4),
      child: Listener(
        onPointerDown: (_) => onPressedDown(),
        onPointerUp: (_) => onPressedUp(),
        onPointerCancel: (_) => onPressedCancel(),
        child: AnimatedScale(
          duration: const Duration(milliseconds: 90),
          curve: Curves.easeOutCubic,
          scale: pressed ? 0.995 : 1.0,
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 220),
            curve: const Cubic(0.22, 1.0, 0.36, 1.0),
            decoration: BoxDecoration(
              color: rowBg,
              borderRadius: BorderRadius.circular(14),
              border: rowBorder,
            ),
            child: GestureDetector(
              onSecondaryTapDown:
                  onDesktopContextMenuRequested == null || deferHeavy
                  ? null
                  : (details) =>
                        onDesktopContextMenuRequested!(details.globalPosition),
              child: Material(
                type: MaterialType.transparency,
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(14),
                ),
                clipBehavior: Clip.antiAlias,
                child: ListTile(
                  dense: true,
                  hoverColor: theme.colorScheme.surfaceContainerHighest
                      .withValues(alpha: 0.42),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(14),
                  ),
                  leading: deferHeavy
                      ? const TrackListCoverPlaceholder()
                      : TrackListCoverThumb(path: coverPath),
                  title: Text(
                    line1,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: isBlocked
                        ? theme.textTheme.bodyLarge?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          )
                        : null,
                  ),
                  subtitle: deferHeavy
                      ? const TrackListSubtitlePlaceholder()
                      : Row(
                          children: [
                            AudioFormatBadge(path: track.path),
                            Expanded(
                              child: Text(
                                line2.isNotEmpty ? line2 : track.path,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: isBlocked
                                    ? theme.textTheme.bodyMedium?.copyWith(
                                        color:
                                            theme.colorScheme.onSurfaceVariant,
                                      )
                                    : null,
                              ),
                            ),
                          ],
                        ),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (isBlocked)
                        Padding(
                          padding: const EdgeInsets.only(right: 4),
                          child: Tooltip(
                            message: blockedReason,
                            child: Icon(
                              Icons.block,
                              size: 18,
                              color: theme.colorScheme.error,
                            ),
                          ),
                        ),
                      if (selectionMode)
                        Checkbox(
                          value: selected,
                          onChanged: (_) => onToggleSelected(),
                        ),
                      if (deferHeavy && !selectionMode) ...[
                        const TrackListTrailingPlaceholder(),
                      ] else ...[
                        IconButton(
                          tooltip: isLiked
                              ? l10n.likedRemoveTooltip
                              : l10n.likedAddTooltip,
                          onPressed: onToggleLike,
                          icon: Icon(
                            isLiked ? Icons.favorite : Icons.favorite_border,
                            color: isLiked ? theme.colorScheme.error : null,
                          ),
                        ),
                        TrackListDurationText(ms: track.durationMs?.toInt()),
                        if (!isDesktopPlatform) ...[
                          const SizedBox(width: 8),
                          PopupMenuButton<TrackListAction>(
                            onSelected: onTrackAction,
                            itemBuilder: buildTrackActionMenuItems,
                          ),
                        ],
                      ],
                      if (reorderIndex != null)
                        ReorderableDragStartListener(
                          index: reorderIndex!,
                          child: const Padding(
                            padding: EdgeInsets.only(left: 4),
                            child: Icon(Icons.drag_handle),
                          ),
                        ),
                    ],
                  ),
                  onTap: onTapTrack,
                  onLongPress: onLongPressSelect,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  static String _basename(String path) {
    final parts = path.split(RegExp(r'[\\/]+'));
    return parts.isEmpty ? path : parts.last;
  }
}
