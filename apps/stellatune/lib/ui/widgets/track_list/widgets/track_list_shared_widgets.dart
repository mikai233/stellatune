import 'dart:io';

import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';

class TrackListSelectionBar extends StatelessWidget {
  const TrackListSelectionBar({
    super.key,
    required this.selectedCount,
    required this.allCount,
    required this.onCancel,
    required this.onSelectAll,
    required this.onAddToPlaylist,
    required this.onRemoveFromCurrentPlaylist,
  });

  final int selectedCount;
  final int allCount;
  final VoidCallback onCancel;
  final VoidCallback? onSelectAll;
  final Future<void> Function() onAddToPlaylist;
  final Future<void> Function()? onRemoveFromCurrentPlaylist;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Material(
      color: Theme.of(context).colorScheme.surfaceContainerHigh,
      child: SizedBox(
        height: 52,
        child: Row(
          children: [
            const SizedBox(width: 8),
            Text(l10n.playlistSelectionCount(selectedCount)),
            const SizedBox(width: 8),
            TextButton(
              onPressed: onSelectAll,
              child: Text(
                selectedCount >= allCount
                    ? l10n.playlistAllSelected
                    : l10n.playlistSelectAll,
              ),
            ),
            const Spacer(),
            TextButton(
              onPressed: onAddToPlaylist,
              child: Text(l10n.playlistBatchAddToPlaylist),
            ),
            if (onRemoveFromCurrentPlaylist != null)
              TextButton(
                onPressed: onRemoveFromCurrentPlaylist,
                child: Text(l10n.playlistBatchRemoveFromCurrent),
              ),
            TextButton(onPressed: onCancel, child: Text(l10n.cancel)),
            const SizedBox(width: 4),
          ],
        ),
      ),
    );
  }
}

class TrackListCoverPlaceholder extends StatelessWidget {
  const TrackListCoverPlaceholder({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        color: theme.colorScheme.surfaceContainerHighest,
      ),
      child: Icon(Icons.music_note, color: theme.colorScheme.onSurfaceVariant),
    );
  }
}

class TrackListCoverThumb extends StatelessWidget {
  const TrackListCoverThumb({super.key, required this.path});

  final String path;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final placeholder = Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        color: theme.colorScheme.primary.withValues(alpha: 0.10),
        border: Border.all(
          color: theme.colorScheme.primary.withValues(alpha: 0.15),
        ),
      ),
      child: Icon(Icons.music_note, color: theme.colorScheme.primary),
    );

    final provider = ResizeImage(
      FileImage(File(path)),
      width: 80,
      height: 80,
      allowUpscaling: false,
    );

    return ClipRRect(
      borderRadius: BorderRadius.circular(8),
      child: Image(
        image: provider,
        width: 40,
        height: 40,
        fit: BoxFit.cover,
        filterQuality: FilterQuality.low,
        gaplessPlayback: true,
        errorBuilder: (context, error, stackTrace) => placeholder,
      ),
    );
  }
}

class TrackListSubtitlePlaceholder extends StatelessWidget {
  const TrackListSubtitlePlaceholder({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      children: [
        Container(
          width: 46,
          height: 16,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(6),
            color: theme.colorScheme.surfaceContainerHighest,
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Container(
            height: 12,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(6),
              color: theme.colorScheme.surfaceContainerHighest,
            ),
          ),
        ),
      ],
    );
  }
}

class TrackListTrailingPlaceholder extends StatelessWidget {
  const TrackListTrailingPlaceholder({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final blockColor = theme.colorScheme.surfaceContainerHighest;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 18,
          height: 18,
          decoration: BoxDecoration(shape: BoxShape.circle, color: blockColor),
        ),
        const SizedBox(width: 10),
        Container(
          width: 36,
          height: 12,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(6),
            color: blockColor,
          ),
        ),
      ],
    );
  }
}

class TrackListDurationText extends StatelessWidget {
  const TrackListDurationText({super.key, required this.ms});

  final int? ms;

  @override
  Widget build(BuildContext context) {
    final v = ms;
    if (v == null || v <= 0) return const SizedBox.shrink();
    final totalSeconds = (v / 1000).floor();
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    return Text(
      '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}',
      style: Theme.of(context).textTheme.bodySmall,
    );
  }
}
