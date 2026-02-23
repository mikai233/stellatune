import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/library/widgets/library_scan_widgets.dart';
import 'package:stellatune/ui/widgets/queue_source_info_card.dart';
import 'package:stellatune/ui/widgets/stellatune_search_field.dart';
import 'package:stellatune/ui/widgets/track_list.dart';

class LibraryTracksContent extends StatelessWidget {
  const LibraryTracksContent({
    super.key,
    required this.l10n,
    required this.searchController,
    required this.onSearchChanged,
    required this.queueSourceLabel,
    required this.selectedFolder,
    required this.hasSubfolders,
    required this.includeSubfolders,
    required this.onToggleIncludeSubfolders,
    required this.isScanning,
    required this.scanned,
    required this.updated,
    required this.skipped,
    required this.errors,
    required this.lastFinishedMs,
    required this.lastError,
    required this.coverDir,
    required this.results,
    required this.likedTrackIds,
    required this.playlists,
    required this.selectionSourceLabel,
    required this.onActivate,
    required this.onEnqueue,
    required this.onSetLiked,
    required this.onAddToPlaylist,
    required this.onRemoveFromPlaylist,
    required this.onBatchAddToPlaylist,
    required this.blockedReasonByTrackId,
    this.onViewportRangeChanged,
  });

  final AppLocalizations l10n;
  final TextEditingController searchController;
  final ValueChanged<String> onSearchChanged;
  final String queueSourceLabel;
  final String selectedFolder;
  final bool hasSubfolders;
  final bool includeSubfolders;
  final VoidCallback onToggleIncludeSubfolders;
  final bool isScanning;
  final int scanned;
  final int updated;
  final int skipped;
  final int errors;
  final int? lastFinishedMs;
  final String? lastError;
  final String coverDir;
  final List<TrackLite> results;
  final Set<int> likedTrackIds;
  final List<PlaylistLite> playlists;
  final String selectionSourceLabel;
  final Future<void> Function(int index, List<TrackLite> items) onActivate;
  final Future<void> Function(TrackLite track) onEnqueue;
  final Future<void> Function(TrackLite track, bool liked) onSetLiked;
  final Future<void> Function(TrackLite track, int playlistId) onAddToPlaylist;
  final Future<void> Function(TrackLite track, int playlistId)
  onRemoveFromPlaylist;
  final Future<void> Function(List<TrackLite> tracks, int playlistId)
  onBatchAddToPlaylist;
  final Map<int, String> blockedReasonByTrackId;
  final void Function(int startIndex, int endIndex)? onViewportRangeChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        StellatuneSearchField(
          controller: searchController,
          onChanged: onSearchChanged,
        ),
        const SizedBox(height: 12),
        QueueSourceInfoCard(queueSourceLabel: queueSourceLabel),
        const SizedBox(height: 12),
        if (selectedFolder.isNotEmpty)
          Row(
            children: [
              Expanded(
                child: Text(
                  selectedFolder,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.titleSmall,
                ),
              ),
              if (hasSubfolders) ...[
                const SizedBox(width: 12),
                Row(
                  children: [
                    Text(l10n.includeSubfolders),
                    const SizedBox(width: 8),
                    Switch(
                      value: includeSubfolders,
                      onChanged: (_) => onToggleIncludeSubfolders(),
                    ),
                  ],
                ),
              ],
            ],
          ),
        if (selectedFolder.isNotEmpty) const SizedBox(height: 12),
        if (isScanning || lastFinishedMs != null)
          LibraryScanStatusCard(
            isScanning: isScanning,
            scanned: scanned,
            updated: updated,
            skipped: skipped,
            errors: errors,
            durationMs: lastFinishedMs,
          ),
        if (lastError != null)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              lastError!,
              style: theme.textTheme.bodyMedium?.copyWith(
                color: theme.colorScheme.error,
              ),
            ),
          ),
        const SizedBox(height: 12),
        Expanded(
          child: TrackList(
            coverDir: coverDir,
            items: results,
            likedTrackIds: likedTrackIds,
            playlists: playlists,
            currentPlaylistId: null,
            onActivate: onActivate,
            onEnqueue: onEnqueue,
            onSetLiked: onSetLiked,
            onAddToPlaylist: onAddToPlaylist,
            onRemoveFromPlaylist: onRemoveFromPlaylist,
            onBatchAddToPlaylist: onBatchAddToPlaylist,
            blockedReasonByTrackId: blockedReasonByTrackId,
            onViewportRangeChanged: onViewportRangeChanged,
          ),
        ),
      ],
    );
  }
}
