import 'package:stellatune/ui/theme/artwork_palette.dart';

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';

enum LibrarySection { songs, albums, artists, folders }

enum _LibrarySort { original, title, artist }

class LibraryCollection {
  LibraryCollection(this.title, this.artist, this.tracks);
  final String title, artist;
  final List<TrackLite> tracks;
}

List<LibraryCollection> groupLibraryTracks(
  List<TrackLite> tracks, {
  required bool byArtist,
}) {
  final groups = <(String, String), LibraryCollection>{};
  for (final track in tracks) {
    final artist = (track.artist ?? '').trim();
    final album = (track.album ?? '').trim();
    final title = byArtist
        ? (artist.isEmpty ? '未知艺术家' : artist)
        : (album.isEmpty ? '未知专辑' : album);
    final key = (title, byArtist ? '' : artist);
    (groups[key] ??= LibraryCollection(title, artist, [])).tracks.add(track);
  }
  return groups.values.toList();
}

/// Desktop presentation only; selection, playback and folder management are injected.
class DesktopLibraryView extends StatefulWidget {
  const DesktopLibraryView({
    super.key,
    required this.tracks,
    required this.coverDir,
    required this.section,
    required this.onSectionChanged,
    required this.trackListBuilder,
    required this.foldersView,
    required this.onAddFolder,
    required this.onScan,
    this.isScanning = false,
    this.coverBuilder,
  });
  final List<TrackLite> tracks;
  final String coverDir;
  final LibrarySection section;
  final ValueChanged<LibrarySection> onSectionChanged;
  final Widget Function(List<TrackLite>) trackListBuilder;
  final Widget foldersView;
  final VoidCallback onAddFolder;
  final ValueChanged<bool> onScan;
  final bool isScanning;
  final Widget Function(TrackLite)? coverBuilder;

  @override
  State<DesktopLibraryView> createState() => _DesktopLibraryViewState();
}

class _DesktopLibraryViewState extends State<DesktopLibraryView> {
  _LibrarySort sort = _LibrarySort.original;
  bool grid = true;
  (String, String)? opened;
  @override
  void didUpdateWidget(covariant DesktopLibraryView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.section != widget.section) opened = null;
  }

  List<TrackLite> sorted(List<TrackLite> tracks) {
    if (sort == _LibrarySort.original) return tracks;
    String title(TrackLite t) => (t.title ?? t.path).toLowerCase();
    final result = [...tracks];
    result.sort((a, b) {
      final first = sort == _LibrarySort.artist
          ? (a.artist ?? '').toLowerCase().compareTo(
              (b.artist ?? '').toLowerCase(),
            )
          : title(a).compareTo(title(b));
      return first == 0 ? a.id.compareTo(b.id) : first;
    });
    return result;
  }

  @override
  Widget build(BuildContext context) {
    final section = widget.section;
    final grouped =
        section == LibrarySection.albums || section == LibrarySection.artists;
    final groups = grouped
        ? groupLibraryTracks(
            widget.tracks,
            byArtist: section == LibrarySection.artists,
          )
        : <LibraryCollection>[];
    if (sort != _LibrarySort.original) {
      groups.sort(
        (a, b) => (sort == _LibrarySort.artist ? a.artist : a.title).compareTo(
          sort == _LibrarySort.artist ? b.artist : b.title,
        ),
      );
    }
    final matches = groups.where((g) => (g.title, g.artist) == opened);
    final selected = matches.isEmpty ? null : matches.first;
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: EdgeInsets.symmetric(horizontal: 24),
            child: Text(
              '音乐库',
              style: TextStyle(
                fontSize: 26,
                fontWeight: FontWeight.w600,
                color: ArtworkPalette.of(context).onBackdrop,
              ),
            ),
          ),
          SizedBox(height: 18),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24),
            child: Row(
              children: [
                for (final tab in LibrarySection.values)
                  Padding(
                    padding: const EdgeInsets.only(right: 26),
                    child: InkWell(
                      key: ValueKey('library-tab-${tab.name}'),
                      onTap: () => widget.onSectionChanged(tab),
                      child: Container(
                        padding: const EdgeInsets.fromLTRB(5, 5, 5, 12),
                        decoration: BoxDecoration(
                          border: Border(
                            bottom: BorderSide(
                              color: section == tab
                                  ? ArtworkPalette.of(context).onBackdrop
                                  : Colors.transparent,
                              width: 2,
                            ),
                          ),
                        ),
                        child: Text(
                          ['歌曲', '专辑', '艺术家', '文件夹'][tab.index],
                          style: TextStyle(
                            fontSize: 14,
                            color: section == tab
                                ? ArtworkPalette.of(context).onBackdrop
                                : ArtworkPalette.of(context).onBackdrop
                                      .withValues(alpha: .70),
                            fontWeight: section == tab
                                ? FontWeight.w600
                                : FontWeight.w400,
                          ),
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ),
          SizedBox(height: 16),
          Expanded(
            child: Theme(
              data: ArtworkPalette.of(context).applyTo(Theme.of(context)),
              child: Material(
                color: ArtworkPalette.of(context).surface
                    .withValues(alpha: .78),
                child: ClipRect(
                  child: Column(
                    children: [
                      Padding(
                        padding: const EdgeInsets.fromLTRB(28, 10, 24, 8),
                        child: Row(
                          children: [
                            if (selected != null)
                              IconButton(
                                tooltip: '返回',
                                onPressed: () => setState(() => opened = null),
                                icon: Icon(Icons.arrow_back, size: 18),
                              ),
                            Expanded(
                              child: Text(
                                selected?.title ??
                                    (section == LibrarySection.folders
                                        ? '文件夹'
                                        : grouped
                                        ? '${groups.length} ${section == LibrarySection.albums ? '张专辑' : '位艺术家'}'
                                        : '${widget.tracks.length} 首歌曲'),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(
                                  fontSize: 12,
                                  color: Theme.of(context)
                                      .colorScheme
                                      .onSurfaceVariant,
                                ),
                              ),
                            ),
                            if (section != LibrarySection.folders)
                              PopupMenuButton<_LibrarySort>(
                                tooltip: '排序',
                                initialValue: sort,
                                onSelected: (value) =>
                                    setState(() => sort = value),
                                itemBuilder: (_) => [
                                  for (final value in _LibrarySort.values)
                                    PopupMenuItem(
                                      value: value,
                                      child: Text(
                                        ['默认顺序', '标题', '艺术家'][value.index],
                                      ),
                                    ),
                                ],
                                child: Padding(
                                  padding: const EdgeInsets.symmetric(
                                    horizontal: 12,
                                    vertical: 8,
                                  ),
                                  child: Row(
                                    children: [
                                      Text(
                                        '排序：${['默认顺序', '标题', '艺术家'][sort.index]}',
                                        style: TextStyle(
                                          fontSize: 11,
                                          color: Theme.of(context)
                                              .colorScheme
                                              .onSurfaceVariant,
                                        ),
                                      ),
                                      const SizedBox(width: 8),
                                      Icon(Icons.expand_more, size: 16),
                                    ],
                                  ),
                                ),
                              ),
                            if (grouped && selected == null) ...[
                              IconButton(
                                tooltip: '网格视图',
                                isSelected: grid,
                                onPressed: () => setState(() => grid = true),
                                icon: Icon(Icons.grid_view_rounded, size: 18),
                              ),
                              IconButton(
                                tooltip: '列表视图',
                                isSelected: !grid,
                                onPressed: () => setState(() => grid = false),
                                icon: Icon(Icons.view_list_rounded, size: 20),
                              ),
                            ],
                            IconButton(
                              tooltip: '添加音乐文件夹',
                              onPressed: widget.onAddFolder,
                              icon: Icon(
                                Icons.create_new_folder_outlined,
                                size: 19,
                              ),
                            ),
                            PopupMenuButton<bool>(
                              tooltip: '扫描音乐库',
                              enabled: !widget.isScanning,
                              onSelected: widget.onScan,
                              icon: Icon(Icons.refresh, size: 19),
                              itemBuilder: (_) => const [
                                PopupMenuItem(value: false, child: Text('扫描')),
                                PopupMenuItem(value: true, child: Text('强制扫描')),
                              ],
                            ),
                          ],
                        ),
                      ),
                      if (widget.isScanning)
                        const LinearProgressIndicator(minHeight: 2),
                      Expanded(
                        child: section == LibrarySection.folders
                            ? widget.foldersView
                            : selected != null
                            ? widget.trackListBuilder(sorted(selected.tracks))
                            : grouped
                            ? _collections(
                                groups,
                                section == LibrarySection.artists,
                              )
                            : widget.trackListBuilder(sorted(widget.tracks)),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _collections(List<LibraryCollection> groups, bool artists) {
    if (groups.isEmpty) return Center(child: Text('暂无内容，添加音乐文件夹后开始浏览'));
    Widget cover(LibraryCollection group) {
      final track = group.tracks.first;
      return ClipRRect(
        borderRadius: BorderRadius.circular(artists ? 200 : 8),
        child:
            widget.coverBuilder?.call(track) ??
            _CollectionCover(path: '${widget.coverDir}/${track.id}'),
      );
    }

    void open(LibraryCollection group) =>
        setState(() => opened = (group.title, group.artist));
    if (!grid) {
      return ListView.builder(
        key: PageStorageKey('library-${widget.section.name}-list'),
        itemExtent: 72,
        padding: const EdgeInsets.symmetric(horizontal: 14),
        itemCount: groups.length,
        itemBuilder: (context, i) {
          final group = groups[i];
          return ListTile(
            onTap: () => open(group),
            leading: SizedBox(width: 50, height: 50, child: cover(group)),
            title: Text(
              group.title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
            subtitle: Text(
              '${group.artist} · ${group.tracks.length} 首',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
            trailing: Icon(Icons.chevron_right, size: 18),
          );
        },
      );
    }
    return LayoutBuilder(
      builder: (context, size) {
        final columns = ((size.maxWidth - 36) / 220).floor().clamp(2, 7);
        final width = (size.maxWidth - 36 - (columns - 1) * 22) / columns;
        final textScale = MediaQuery.textScalerOf(context).scale(1);
        return GridView.builder(
          key: PageStorageKey('library-${widget.section.name}-grid'),
          padding: const EdgeInsets.fromLTRB(18, 10, 18, 20),
          gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
            crossAxisCount: columns,
            crossAxisSpacing: 22,
            mainAxisSpacing: 20,
            mainAxisExtent: width + 69 * textScale,
          ),
          itemCount: groups.length,
          itemBuilder: (context, i) {
            final group = groups[i];
            return InkWell(
              key: ValueKey('library-collection-$i'),
              borderRadius: BorderRadius.circular(8),
              onTap: () => open(group),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  AspectRatio(aspectRatio: 1, child: cover(group)),
                  const SizedBox(height: 10),
                  Text(
                    group.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: Theme.of(context).colorScheme.onSurface,
                    ),
                  ),
                  if (!artists)
                    Text(
                      group.artist.isEmpty ? '未知艺术家' : group.artist,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 11,
                        color: Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                    ),
                  Text(
                    '${group.tracks.length} 首歌曲',
                    style: TextStyle(
                      fontSize: 11,
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            );
          },
        );
      },
    );
  }
}

class _CollectionCover extends StatelessWidget {
  const _CollectionCover({required this.path});
  final String path;
  @override
  Widget build(BuildContext context) {
    final placeholder = ColoredBox(
      color: Colors.white.withValues(alpha: .15),
      child: Center(
        child: Icon(
          Icons.album_outlined,
          size: 48,
          color: Theme.of(context).colorScheme.onSurfaceVariant,
        ),
      ),
    );
    return Image.file(
      File(path),
      fit: BoxFit.cover,
      cacheWidth: 512,
      frameBuilder: (_, child, frame, _) => frame == null ? placeholder : child,
      errorBuilder: (_, _, _) => placeholder,
    );
  }
}
