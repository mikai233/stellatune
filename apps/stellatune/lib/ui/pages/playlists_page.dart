import 'dart:io';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/ui/widgets/track_list.dart';

class PlaylistsPage extends ConsumerStatefulWidget {
  const PlaylistsPage({super.key, this.useGlobalTopBar = false});

  final bool useGlobalTopBar;

  @override
  ConsumerState<PlaylistsPage> createState() => PlaylistsPageState();
}

class PlaylistsPageState extends ConsumerState<PlaylistsPage> {
  final _searchController = TextEditingController();
  bool _playlistsPanelOpen = false;
  bool _autoSelecting = false;

  bool get isPlaylistsPanelOpen => _playlistsPanelOpen;

  void togglePlaylistsPanel() {
    setState(() => _playlistsPanelOpen = !_playlistsPanelOpen);
  }

  Future<void> createPlaylistFromTopBar() => _createPlaylist(context);

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final coverDir = ref.watch(coverDirProvider);

    final playlists = ref.watch(
      libraryControllerProvider.select((s) => s.playlists),
    );
    final selectedPlaylistId = ref.watch(
      libraryControllerProvider.select((s) => s.selectedPlaylistId),
    );
    final results = ref.watch(
      libraryControllerProvider.select((s) => s.results),
    );
    final likedTrackIds = ref.watch(
      libraryControllerProvider.select((s) => s.likedTrackIds),
    );
    final queueSourceSnapshot = ref.watch(
      queueControllerProvider.select((s) => s.sourceLabel),
    );
    _ensurePlaylistSelected(playlists, selectedPlaylistId);

    PlaylistLite? selectedPlaylist;
    if (selectedPlaylistId != null) {
      for (final p in playlists) {
        if (p.id.toInt() == selectedPlaylistId) {
          selectedPlaylist = p;
          break;
        }
      }
    }

    final selectionSourceLabel = selectedPlaylist == null
        ? l10n.queueSourceUnset
        : _playlistDisplayName(l10n, selectedPlaylist);
    final queueSourceLabel = (queueSourceSnapshot ?? '').trim().isEmpty
        ? l10n.queueSourceUnset
        : queueSourceSnapshot!.trim();

    return LayoutBuilder(
      builder: (context, constraints) {
        final panelWidth = constraints.maxWidth < 760
            ? (constraints.maxWidth * 0.84).clamp(280.0, 360.0)
            : (constraints.maxWidth * 0.34).clamp(300.0, 380.0);
        final content = Expanded(
          child: ClipRect(
            child: Stack(
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
                  child: _PlaylistTracksPane(
                    searchController: _searchController,
                    queueSourceLabel: queueSourceLabel,
                    selectedLabel: selectedPlaylist == null
                        ? l10n.queueSourceUnset
                        : _playlistDisplayName(l10n, selectedPlaylist),
                    playlists: playlists,
                    selectedPlaylistId: selectedPlaylistId,
                    results: results,
                    likedTrackIds: likedTrackIds,
                    coverDir: coverDir,
                    onSearchChanged: (q) => ref
                        .read(libraryControllerProvider.notifier)
                        .setQuery(q),
                    onActivate: (index, items) async {
                      await ref
                          .read(playbackControllerProvider.notifier)
                          .setQueueAndPlayTracks(
                            items,
                            startIndex: index,
                            sourceLabel: selectionSourceLabel,
                          );
                    },
                    onEnqueue: (track) async {
                      await ref
                          .read(playbackControllerProvider.notifier)
                          .enqueueTracks([track]);
                    },
                    onSetLiked: (track, liked) async {
                      await ref
                          .read(libraryControllerProvider.notifier)
                          .setTrackLiked(track.id.toInt(), liked);
                    },
                    onAddToPlaylist: (track, playlistId) async {
                      await ref
                          .read(libraryControllerProvider.notifier)
                          .addTrackToPlaylist(playlistId, track.id.toInt());
                    },
                    onRemoveFromPlaylist: (track, playlistId) async {
                      await ref
                          .read(libraryControllerProvider.notifier)
                          .removeTrackFromPlaylist(
                            playlistId,
                            track.id.toInt(),
                          );
                    },
                    onMoveInCurrentPlaylist: selectedPlaylistId == null
                        ? null
                        : (track, newIndex) async {
                            await ref
                                .read(libraryControllerProvider.notifier)
                                .moveTrackInPlaylist(
                                  playlistId: selectedPlaylistId,
                                  trackId: track.id.toInt(),
                                  newIndex: newIndex,
                                );
                          },
                    onBatchAddToPlaylist: (tracks, playlistId) async {
                      await ref
                          .read(libraryControllerProvider.notifier)
                          .addTracksToPlaylist(
                            playlistId: playlistId,
                            trackIds: tracks.map((t) => t.id.toInt()).toList(),
                          );
                    },
                    onBatchRemoveFromCurrentPlaylist: selectedPlaylistId == null
                        ? null
                        : (tracks, playlistId) async {
                            await ref
                                .read(libraryControllerProvider.notifier)
                                .removeTracksFromPlaylist(
                                  playlistId: playlistId,
                                  trackIds: tracks
                                      .map((t) => t.id.toInt())
                                      .toList(),
                                );
                          },
                  ),
                ),
                if (_playlistsPanelOpen)
                  Positioned.fill(
                    child: GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onTap: () => setState(() => _playlistsPanelOpen = false),
                      child: const SizedBox.expand(),
                    ),
                  ),
                Align(
                  alignment: Alignment.centerLeft,
                  child: AnimatedSlide(
                    duration: const Duration(milliseconds: 260),
                    curve: Curves.easeOutCubic,
                    offset: _playlistsPanelOpen
                        ? Offset.zero
                        : const Offset(-1.0, 0),
                    child: SizedBox(
                      width: panelWidth,
                      child: _PlaylistsDrawerPanel(
                        playlists: playlists,
                        selectedPlaylistId: selectedPlaylistId,
                        onSelect: (id) => ref
                            .read(libraryControllerProvider.notifier)
                            .selectPlaylist(id),
                        onRename: (id, currentName) async {
                          final nextName = await _promptPlaylistName(
                            context,
                            title: l10n.playlistRenameTitle,
                            initialValue: currentName,
                          );
                          if (nextName == null) return;
                          await ref
                              .read(libraryControllerProvider.notifier)
                              .renamePlaylist(id, nextName);
                        },
                        onDelete: (id, name) async {
                          final confirmed = await _confirmDeletePlaylist(
                            context,
                            name: name,
                          );
                          if (!confirmed) return;
                          await ref
                              .read(libraryControllerProvider.notifier)
                              .deletePlaylist(id);
                        },
                        onCreate: () => _createPlaylist(context),
                        onClose: () =>
                            setState(() => _playlistsPanelOpen = false),
                        coverDir: coverDir,
                        displayName: (p) => _playlistDisplayName(l10n, p),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        );

        if (widget.useGlobalTopBar) {
          return Column(children: [content]);
        }

        return Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 6, 10, 6),
              child: SizedBox(
                height: 48,
                child: Row(
                  children: [
                    IconButton(
                      tooltip: l10n.playlistSectionTitle,
                      icon: const Icon(Icons.playlist_play),
                      onPressed: togglePlaylistsPanel,
                    ),
                    Expanded(
                      child: Text(
                        l10n.playlistSectionTitle,
                        style: theme.textTheme.headlineSmall?.copyWith(
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                    IconButton(
                      tooltip: l10n.playlistCreateTooltip,
                      icon: const Icon(Icons.playlist_add_outlined),
                      onPressed: createPlaylistFromTopBar,
                    ),
                  ],
                ),
              ),
            ),
            Divider(
              height: 1,
              thickness: 0.8,
              color: theme.colorScheme.onSurface.withValues(alpha: 0.12),
            ),
            content,
          ],
        );
      },
    );
  }

  void _ensurePlaylistSelected(List<PlaylistLite> playlists, int? selectedId) {
    if (_autoSelecting || selectedId != null || playlists.isEmpty) return;
    _autoSelecting = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final notifier = ref.read(libraryControllerProvider.notifier);
      final target = _defaultPlaylistId(playlists);
      notifier.selectPlaylist(target);
      _autoSelecting = false;
    });
  }

  int _defaultPlaylistId(List<PlaylistLite> playlists) {
    for (final p in playlists) {
      if (p.systemKey == 'liked') {
        return p.id.toInt();
      }
    }
    return playlists.first.id.toInt();
  }

  Future<void> _createPlaylist(BuildContext context) async {
    final l10n = AppLocalizations.of(context)!;
    final name = await _promptPlaylistName(
      context,
      title: l10n.playlistCreateTitle,
    );
    if (name == null) return;
    await ref.read(libraryControllerProvider.notifier).createPlaylist(name);
  }

  Future<String?> _promptPlaylistName(
    BuildContext context, {
    required String title,
    String initialValue = '',
  }) async {
    final l10n = AppLocalizations.of(context)!;
    final controller = TextEditingController(text: initialValue);
    try {
      final result = await showDialog<String>(
        context: context,
        builder: (context) {
          return AlertDialog(
            title: Text(title),
            content: TextField(
              controller: controller,
              autofocus: true,
              decoration: InputDecoration(
                border: const OutlineInputBorder(),
                hintText: l10n.playlistNameHint,
              ),
              onSubmitted: (value) {
                final trimmed = value.trim();
                Navigator.of(context).pop(trimmed.isEmpty ? null : trimmed);
              },
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(context).pop(),
                child: Text(l10n.cancel),
              ),
              FilledButton(
                onPressed: () {
                  final trimmed = controller.text.trim();
                  Navigator.of(context).pop(trimmed.isEmpty ? null : trimmed);
                },
                child: Text(l10n.ok),
              ),
            ],
          );
        },
      );
      return result;
    } finally {
      controller.dispose();
    }
  }

  Future<bool> _confirmDeletePlaylist(
    BuildContext context, {
    required String name,
  }) async {
    final l10n = AppLocalizations.of(context)!;
    final result = await showDialog<bool>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: Text(l10n.playlistDeleteTitle),
          content: Text(l10n.playlistDeleteConfirm(name)),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l10n.playlistDeleteAction),
            ),
          ],
        );
      },
    );
    return result ?? false;
  }

  String _playlistDisplayName(AppLocalizations l10n, PlaylistLite playlist) {
    if (playlist.systemKey == 'liked') {
      return l10n.likedPlaylistName;
    }
    return playlist.name;
  }
}

class _PlaylistsSidebar extends StatelessWidget {
  const _PlaylistsSidebar({
    required this.playlists,
    required this.selectedPlaylistId,
    required this.coverDir,
    required this.onSelect,
    required this.onRename,
    required this.onDelete,
    required this.displayName,
  });

  final List<PlaylistLite> playlists;
  final int? selectedPlaylistId;
  final String coverDir;
  final ValueChanged<int> onSelect;
  final Future<void> Function(int id, String currentName) onRename;
  final Future<void> Function(int id, String currentName) onDelete;
  final String Function(PlaylistLite playlist) displayName;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scrollbar(
      child: ListView.builder(
        padding: const EdgeInsets.symmetric(vertical: 6),
        itemCount: playlists.length,
        itemBuilder: (context, index) {
          final playlist = playlists[index];
          final id = playlist.id.toInt();
          final isSelected = selectedPlaylistId == id;
          final isSystem = playlist.systemKey != null;
          final name = displayName(playlist);
          return _PlaylistSidebarItem(
            coverDir: coverDir,
            playlist: playlist,
            name: name,
            subtitle: l10n.playlistTrackCount(playlist.trackCount.toInt()),
            isSelected: isSelected,
            isSystem: isSystem,
            onTap: () => onSelect(id),
            onRename: () => onRename(id, name),
            onDelete: () => onDelete(id, name),
          );
        },
      ),
    );
  }
}

class _PlaylistSidebarItem extends StatefulWidget {
  const _PlaylistSidebarItem({
    required this.coverDir,
    required this.playlist,
    required this.name,
    required this.subtitle,
    required this.isSelected,
    required this.isSystem,
    required this.onTap,
    required this.onRename,
    required this.onDelete,
  });

  final String coverDir;
  final PlaylistLite playlist;
  final String name;
  final String subtitle;
  final bool isSelected;
  final bool isSystem;
  final VoidCallback onTap;
  final Future<void> Function() onRename;
  final Future<void> Function() onDelete;

  @override
  State<_PlaylistSidebarItem> createState() => _PlaylistSidebarItemState();
}

class _PlaylistSidebarItemState extends State<_PlaylistSidebarItem> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final likedPlaylist = widget.playlist.systemKey == 'liked';
    final accent = likedPlaylist
        ? theme.colorScheme.error
        : theme.colorScheme.primary;
    final hovered = _hovering && !widget.isSelected;
    final base = theme.colorScheme.surface.withValues(
      alpha: hovered ? 0.52 : 0.30,
    );
    final selectedBg = theme.colorScheme.secondaryContainer.withValues(
      alpha: 0.88,
    );
    final border = widget.isSelected
        ? accent.withValues(alpha: 0.45)
        : theme.colorScheme.onSurface.withValues(alpha: hovered ? 0.20 : 0.10);

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 3),
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovering = true),
        onExit: (_) => setState(() => _hovering = false),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            gradient: LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: widget.isSelected
                  ? [
                      selectedBg,
                      theme.colorScheme.secondaryContainer.withValues(
                        alpha: 0.74,
                      ),
                    ]
                  : [
                      base,
                      theme.colorScheme.surfaceContainerHighest.withValues(
                        alpha: 0.28,
                      ),
                    ],
            ),
            border: Border.all(color: border),
            boxShadow: [
              if (widget.isSelected)
                BoxShadow(
                  color: accent.withValues(alpha: 0.18),
                  blurRadius: 14,
                  offset: const Offset(0, 4),
                )
              else if (hovered)
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.10),
                  blurRadius: 10,
                  offset: const Offset(0, 3),
                ),
            ],
          ),
          child: Material(
            type: MaterialType.transparency,
            child: InkWell(
              borderRadius: BorderRadius.circular(14),
              onTap: widget.onTap,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(8, 8, 6, 8),
                child: Row(
                  children: [
                    _PlaylistCover(
                      coverDir: widget.coverDir,
                      firstTrackId: widget.playlist.firstTrackId?.toInt(),
                      likedPlaylist: likedPlaylist,
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              Expanded(
                                child: Text(
                                  widget.name,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: theme.textTheme.titleSmall?.copyWith(
                                    fontWeight: widget.isSelected
                                        ? FontWeight.w700
                                        : FontWeight.w600,
                                  ),
                                ),
                              ),
                              if (widget.isSelected)
                                Icon(
                                  Icons.graphic_eq_rounded,
                                  size: 16,
                                  color: accent.withValues(alpha: 0.92),
                                ),
                            ],
                          ),
                          const SizedBox(height: 3),
                          Row(
                            children: [
                              Icon(
                                Icons.queue_music_rounded,
                                size: 13,
                                color: theme.colorScheme.onSurfaceVariant
                                    .withValues(alpha: 0.85),
                              ),
                              const SizedBox(width: 4),
                              Expanded(
                                child: Text(
                                  widget.subtitle,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: theme.textTheme.bodySmall?.copyWith(
                                    color: theme.colorScheme.onSurfaceVariant,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                    if (!widget.isSystem)
                      AnimatedOpacity(
                        duration: const Duration(milliseconds: 160),
                        opacity: widget.isSelected || _hovering ? 1.0 : 0.78,
                        child: PopupMenuButton<String>(
                          icon: const Icon(Icons.more_horiz_rounded, size: 18),
                          onSelected: (value) async {
                            if (value == 'rename') {
                              await widget.onRename();
                              return;
                            }
                            await widget.onDelete();
                          },
                          itemBuilder: (context) => [
                            PopupMenuItem(
                              value: 'rename',
                              child: Text(
                                AppLocalizations.of(
                                  context,
                                )!.playlistRenameAction,
                              ),
                            ),
                            PopupMenuItem(
                              value: 'delete',
                              child: Text(
                                AppLocalizations.of(
                                  context,
                                )!.playlistDeleteAction,
                              ),
                            ),
                          ],
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _PlaylistCover extends StatelessWidget {
  const _PlaylistCover({
    required this.coverDir,
    required this.firstTrackId,
    required this.likedPlaylist,
  });

  final String coverDir;
  final int? firstTrackId;
  final bool likedPlaylist;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final icon = likedPlaylist ? Icons.favorite : Icons.playlist_play;
    final iconColor = likedPlaylist
        ? theme.colorScheme.error
        : theme.colorScheme.primary;
    final placeholder = Container(
      width: 44,
      height: 44,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(11),
        color: iconColor.withValues(alpha: 0.14),
        border: Border.all(color: iconColor.withValues(alpha: 0.22)),
      ),
      child: Icon(icon, size: 20, color: iconColor),
    );

    if (firstTrackId == null || coverDir.isEmpty) {
      return placeholder;
    }

    final path = '$coverDir${Platform.pathSeparator}$firstTrackId';
    final provider = ResizeImage(
      FileImage(File(path)),
      width: 96,
      height: 96,
      allowUpscaling: false,
    );
    return ClipRRect(
      borderRadius: BorderRadius.circular(11),
      child: Image(
        image: provider,
        width: 44,
        height: 44,
        fit: BoxFit.cover,
        filterQuality: FilterQuality.low,
        gaplessPlayback: true,
        errorBuilder: (context, error, stackTrace) => placeholder,
      ),
    );
  }
}

class _PlaylistsDrawerPanel extends StatelessWidget {
  const _PlaylistsDrawerPanel({
    required this.playlists,
    required this.selectedPlaylistId,
    required this.coverDir,
    required this.onSelect,
    required this.onRename,
    required this.onDelete,
    required this.onCreate,
    required this.onClose,
    required this.displayName,
  });

  final List<PlaylistLite> playlists;
  final int? selectedPlaylistId;
  final String coverDir;
  final ValueChanged<int> onSelect;
  final Future<void> Function(int id, String currentName) onRename;
  final Future<void> Function(int id, String currentName) onDelete;
  final VoidCallback onCreate;
  final VoidCallback onClose;
  final String Function(PlaylistLite playlist) displayName;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);

    return SafeArea(
      right: false,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 12, 10, 12),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(18),
          child: BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
            child: DecoratedBox(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                  colors: [
                    theme.colorScheme.surface.withValues(alpha: 0.84),
                    theme.colorScheme.surfaceContainerHigh.withValues(
                      alpha: 0.76,
                    ),
                  ],
                ),
                border: Border.all(
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.14),
                ),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withValues(alpha: 0.14),
                    blurRadius: 24,
                    offset: const Offset(0, 10),
                  ),
                ],
              ),
              child: Column(
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(12, 10, 8, 6),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            l10n.playlistSectionTitle,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: theme.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ),
                        IconButton(
                          tooltip: l10n.playlistCreateTooltip,
                          onPressed: onCreate,
                          icon: const Icon(Icons.playlist_add_outlined),
                        ),
                        IconButton(
                          tooltip: l10n.tooltipBack,
                          onPressed: onClose,
                          icon: const Icon(Icons.close),
                        ),
                      ],
                    ),
                  ),
                  Divider(
                    height: 1,
                    thickness: 0.8,
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.12),
                  ),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(8, 8, 8, 10),
                      child: _PlaylistsSidebar(
                        playlists: playlists,
                        selectedPlaylistId: selectedPlaylistId,
                        coverDir: coverDir,
                        onSelect: onSelect,
                        onRename: onRename,
                        onDelete: onDelete,
                        displayName: displayName,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _PlaylistTracksPane extends StatelessWidget {
  const _PlaylistTracksPane({
    required this.searchController,
    required this.queueSourceLabel,
    required this.selectedLabel,
    required this.playlists,
    required this.selectedPlaylistId,
    required this.results,
    required this.likedTrackIds,
    required this.coverDir,
    required this.onSearchChanged,
    required this.onActivate,
    required this.onEnqueue,
    required this.onSetLiked,
    required this.onAddToPlaylist,
    required this.onRemoveFromPlaylist,
    this.onMoveInCurrentPlaylist,
    this.onBatchAddToPlaylist,
    this.onBatchRemoveFromCurrentPlaylist,
  });

  final TextEditingController searchController;
  final String queueSourceLabel;
  final String selectedLabel;
  final List<PlaylistLite> playlists;
  final int? selectedPlaylistId;
  final List<TrackLite> results;
  final Set<int> likedTrackIds;
  final String coverDir;
  final ValueChanged<String> onSearchChanged;
  final Future<void> Function(int index, List<TrackLite> items) onActivate;
  final Future<void> Function(TrackLite track) onEnqueue;
  final Future<void> Function(TrackLite track, bool liked) onSetLiked;
  final Future<void> Function(TrackLite track, int playlistId) onAddToPlaylist;
  final Future<void> Function(TrackLite track, int playlistId)
  onRemoveFromPlaylist;
  final Future<void> Function(TrackLite track, int newIndex)?
  onMoveInCurrentPlaylist;
  final Future<void> Function(List<TrackLite> tracks, int playlistId)?
  onBatchAddToPlaylist;
  final Future<void> Function(List<TrackLite> tracks, int playlistId)?
  onBatchRemoveFromCurrentPlaylist;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextField(
          controller: searchController,
          decoration: InputDecoration(
            prefixIcon: const Icon(Icons.search),
            hintText: l10n.searchHint,
            filled: true,
            fillColor: theme.colorScheme.surfaceContainerLowest.withValues(
              alpha: 0.72,
            ),
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(14),
              borderSide: BorderSide(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.10),
              ),
            ),
            enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(14),
              borderSide: BorderSide(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.10),
              ),
            ),
            focusedBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(14),
              borderSide: BorderSide(color: theme.colorScheme.primary),
            ),
          ),
          onChanged: onSearchChanged,
        ),
        const SizedBox(height: 12),
        Container(
          width: double.infinity,
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          decoration: BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: [
                theme.colorScheme.surfaceContainerHigh.withValues(alpha: 0.74),
                theme.colorScheme.surfaceContainer.withValues(alpha: 0.58),
              ],
            ),
            border: Border.all(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.08),
            ),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.045),
                blurRadius: 8,
                offset: const Offset(0, 2),
              ),
            ],
            borderRadius: BorderRadius.circular(14),
          ),
          child: Row(
            children: [
              Icon(
                Icons.queue_music,
                size: 18,
                color: theme.colorScheme.primary,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      l10n.queueSourceTitle,
                      style: theme.textTheme.labelMedium,
                    ),
                    Text(
                      queueSourceLabel,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodyMedium,
                    ),
                    Text(
                      l10n.queueSourceHint,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        Text(
          selectedLabel,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: theme.textTheme.titleMedium?.copyWith(
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 12),
        Expanded(
          child: TrackList(
            coverDir: coverDir,
            items: results,
            likedTrackIds: likedTrackIds,
            playlists: playlists,
            currentPlaylistId: selectedPlaylistId,
            onActivate: onActivate,
            onEnqueue: onEnqueue,
            onSetLiked: onSetLiked,
            onAddToPlaylist: onAddToPlaylist,
            onRemoveFromPlaylist: onRemoveFromPlaylist,
            onMoveInCurrentPlaylist: onMoveInCurrentPlaylist,
            onBatchAddToPlaylist: onBatchAddToPlaylist,
            onBatchRemoveFromCurrentPlaylist: onBatchRemoveFromCurrentPlaylist,
          ),
        ),
      ],
    );
  }
}
