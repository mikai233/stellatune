import 'dart:io';
import 'dart:ui' show ImageFilter;

import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/playlists/models/playlists_data_models.dart';
import 'package:stellatune/ui/widgets/queue_cover_image.dart';

class _UnifiedPlaylistsSidebar extends StatefulWidget {
  const _UnifiedPlaylistsSidebar({
    required this.localPlaylists,
    required this.selectedLocalPlaylistId,
    required this.pluginPlaylists,
    required this.selectedPluginPlaylistKey,
    required this.coverDir,
    required this.onSelectLocal,
    required this.onSelectPlugin,
    required this.onRenameLocal,
    required this.onDeleteLocal,
    required this.displayName,
    this.pluginError,
  });

  final List<PlaylistLite> localPlaylists;
  final int? selectedLocalPlaylistId;
  final List<PluginPlaylistEntry> pluginPlaylists;
  final String? selectedPluginPlaylistKey;
  final String coverDir;
  final ValueChanged<int> onSelectLocal;
  final ValueChanged<PluginPlaylistEntry> onSelectPlugin;
  final Future<void> Function(int id, String currentName) onRenameLocal;
  final Future<void> Function(int id, String currentName) onDeleteLocal;
  final String Function(PlaylistLite playlist) displayName;
  final String? pluginError;

  @override
  State<_UnifiedPlaylistsSidebar> createState() =>
      _UnifiedPlaylistsSidebarState();
}

class _UnifiedPlaylistsSidebarState extends State<_UnifiedPlaylistsSidebar> {
  final ScrollController _scrollController = ScrollController();

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final pluginGroups = <String, List<PluginPlaylistEntry>>{};
    final pluginGroupHeaders = <String, String>{};
    for (final playlist in widget.pluginPlaylists) {
      final groupKey = '${playlist.pluginId}::${playlist.typeId}';
      pluginGroups
          .putIfAbsent(groupKey, () => <PluginPlaylistEntry>[])
          .add(playlist);
      pluginGroupHeaders.putIfAbsent(
        groupKey,
        () => '${playlist.pluginName} / ${playlist.typeDisplayName}',
      );
    }

    return Scrollbar(
      controller: _scrollController,
      child: ListView(
        controller: _scrollController,
        primary: false,
        padding: const EdgeInsets.symmetric(vertical: 4),
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 6, 8, 6),
            child: Text('本地歌单', style: Theme.of(context).textTheme.labelLarge),
          ),
          for (final playlist in widget.localPlaylists)
            _PlaylistSidebarItem(
              coverDir: widget.coverDir,
              playlist: playlist,
              name: widget.displayName(playlist),
              subtitle: l10n.playlistTrackCount(playlist.trackCount.toInt()),
              isSelected:
                  widget.selectedPluginPlaylistKey == null &&
                  widget.selectedLocalPlaylistId == playlist.id.toInt(),
              isSystem: playlist.systemKey != null,
              onTap: () => widget.onSelectLocal(playlist.id.toInt()),
              onRename: () => widget.onRenameLocal(
                playlist.id.toInt(),
                widget.displayName(playlist),
              ),
              onDelete: () => widget.onDeleteLocal(
                playlist.id.toInt(),
                widget.displayName(playlist),
              ),
            ),
          const SizedBox(height: 8),
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 6, 8, 6),
            child: Text('插件歌单', style: Theme.of(context).textTheme.labelLarge),
          ),
          if (widget.pluginError != null &&
              widget.pluginError!.trim().isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 2, 10, 8),
              child: Text(
                widget.pluginError!,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.error,
                ),
              ),
            ),
          if (widget.pluginPlaylists.isEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 2, 10, 10),
              child: Text(
                '暂无插件歌单',
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ),
          for (final groupKey in pluginGroups.keys) ...[
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 4, 10, 4),
              child: Text(
                pluginGroupHeaders[groupKey] ?? groupKey,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            for (final playlist in pluginGroups[groupKey]!)
              _PluginPlaylistSidebarItem(
                playlist: playlist,
                isSelected: widget.selectedPluginPlaylistKey == playlist.key,
                onTap: () => widget.onSelectPlugin(playlist),
              ),
            const SizedBox(height: 4),
          ],
        ],
      ),
    );
  }
}

class _PluginPlaylistSidebarItem extends StatefulWidget {
  const _PluginPlaylistSidebarItem({
    required this.playlist,
    required this.isSelected,
    required this.onTap,
  });

  final PluginPlaylistEntry playlist;
  final bool isSelected;
  final VoidCallback onTap;

  @override
  State<_PluginPlaylistSidebarItem> createState() =>
      _PluginPlaylistSidebarItemState();
}

class _PluginPlaylistSidebarItemState
    extends State<_PluginPlaylistSidebarItem> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent = theme.colorScheme.primary;
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
    final subtitle =
        '${widget.playlist.sourceLabel}${widget.playlist.trackCount == null ? '' : ' · ${widget.playlist.trackCount}'}';

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
                    _PluginPlaylistCover(
                      accent: accent,
                      cover: widget.playlist.cover,
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
                                  widget.playlist.title,
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
                                  subtitle,
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

class _PluginPlaylistCover extends StatelessWidget {
  const _PluginPlaylistCover({required this.accent, this.cover});

  final Color accent;
  final QueueCover? cover;

  @override
  Widget build(BuildContext context) {
    final placeholder = Container(
      width: 44,
      height: 44,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(11),
        color: accent.withValues(alpha: 0.14),
        border: Border.all(color: accent.withValues(alpha: 0.22)),
      ),
      child: Icon(Icons.cloud_outlined, size: 20, color: accent),
    );

    return QueueCoverImage(cover: cover, placeholder: placeholder, size: 44);
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

class PlaylistsDrawerPanel extends StatelessWidget {
  const PlaylistsDrawerPanel({
    super.key,
    required this.playlists,
    required this.selectedPlaylistId,
    required this.pluginPlaylists,
    required this.selectedPluginPlaylistKey,
    required this.coverDir,
    required this.onSelect,
    required this.onSelectPlugin,
    required this.onRename,
    required this.onDelete,
    required this.onCreate,
    required this.onRefreshPlugins,
    required this.pluginLoading,
    required this.pluginError,
    required this.onClose,
    required this.displayName,
  });

  final List<PlaylistLite> playlists;
  final int? selectedPlaylistId;
  final List<PluginPlaylistEntry> pluginPlaylists;
  final String? selectedPluginPlaylistKey;
  final String coverDir;
  final ValueChanged<int> onSelect;
  final ValueChanged<PluginPlaylistEntry> onSelectPlugin;
  final Future<void> Function(int id, String currentName) onRename;
  final Future<void> Function(int id, String currentName) onDelete;
  final VoidCallback onCreate;
  final Future<void> Function() onRefreshPlugins;
  final bool pluginLoading;
  final String? pluginError;
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
                          tooltip: '刷新插件歌单',
                          onPressed: pluginLoading
                              ? null
                              : () => onRefreshPlugins(),
                          icon: pluginLoading
                              ? const SizedBox(
                                  width: 16,
                                  height: 16,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.cloud_sync_outlined),
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
                      child: _UnifiedPlaylistsSidebar(
                        localPlaylists: playlists,
                        selectedLocalPlaylistId: selectedPlaylistId,
                        pluginPlaylists: pluginPlaylists,
                        selectedPluginPlaylistKey: selectedPluginPlaylistKey,
                        coverDir: coverDir,
                        onSelectLocal: onSelect,
                        onSelectPlugin: onSelectPlugin,
                        onRenameLocal: onRename,
                        onDeleteLocal: onDelete,
                        displayName: displayName,
                        pluginError: pluginError,
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
