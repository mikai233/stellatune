import 'dart:async';

import 'package:flutter/material.dart';
import 'package:stellatune/ui/widgets/app_select.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart' show LibraryEventPatterns;
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_controller.dart';
import 'package:stellatune/library/catalog_tree_controller.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/ui/theme/artwork_palette.dart';

import 'catalog_folder_view.dart';
import 'catalog_widgets.dart';

class CatalogLibraryView extends ConsumerStatefulWidget {
  const CatalogLibraryView({
    super.key,
    required this.onAddFolder,
    required this.onScan,
    required this.folderManager,
    this.isScanning = false,
    this.currentItem,
  });
  final VoidCallback onAddFolder;
  final ValueChanged<bool> onScan;
  final Widget folderManager;
  final bool isScanning;
  final QueueItem? currentItem;
  @override
  ConsumerState<CatalogLibraryView> createState() => _CatalogLibraryViewState();
}

final _catalogGridPreferences = Provider((ref) => <MediaKind, bool>{});

class _CatalogLibraryViewState extends ConsumerState<CatalogLibraryView> {
  Timer? _refreshTimer;
  StreamSubscription? _events;
  String? _collectionId, _actionError;
  int _actionGeneration = 0;
  late CatalogBridge _bridge;
  String title(CatalogItem item) => catalogTitle(context, item);
  String kindLabel(MediaKind kind) => catalogKindLabel(context, kind);
  String tr(String zh, String en) =>
      Localizations.localeOf(context).languageCode == 'zh' ? zh : en;
  @override
  void initState() {
    super.initState();
    _bridge = ref.read(catalogBridgeProvider);
    Future.microtask(() {
      if (mounted) {
        ref
            .read(catalogControllerProvider.notifier)
            .refreshSources(preserveItems: true);
      }
    });
    _events = ref.read(libraryBridgeProvider).events().listen((event) {
      event.when(
        changed: _scheduleRefresh,
        scanProgress: (_, _, _, _) {},
        scanFinished: (_, _, _, _, _) => _scheduleRefresh(),
        error: (_) {},
        log: (_) {},
      );
    });
  }

  void _scheduleRefresh() {
    _refreshTimer?.cancel();
    _refreshTimer = Timer(const Duration(milliseconds: 400), () {
      if (!mounted) return;
      final state = ref.read(catalogControllerProvider);
      if (state.source?.local == true) {
        ref
            .read(catalogControllerProvider.notifier)
            .refresh(preserveItems: true);
        final provider = catalogTreeProvider(state.sourceId!);
        if (ref.exists(provider)) ref.read(provider).refresh();
      }
    });
  }

  @override
  void dispose() {
    _refreshTimer?.cancel();
    _events?.cancel();
    _actionGeneration++;
    if (_collectionId != null) unawaited(_bridge.cancel(_collectionId!));
    super.dispose();
  }

  Future<void> _play({CatalogItem? item, bool enqueue = false}) async {
    final state = ref.read(catalogControllerProvider);
    if (state.source == null) return;
    if (_collectionId != null) unawaited(_bridge.cancel(_collectionId!));
    final generation = ++_actionGeneration;
    final requestId =
        'catalog-${DateTime.now().microsecondsSinceEpoch}-$generation';
    setState(() {
      _collectionId = requestId;
      _actionError = null;
    });
    try {
      if (!enqueue && item != null && state.search.isNotEmpty) {
        final items = await _bridge.prepare([item]);
        if (!mounted || generation != _actionGeneration) return;
        if (items.length != 1) {
          throw StateError('Expected one playable search result');
        }
        await ref
            .read(playbackControllerProvider.notifier)
            .playItemPreservingQueue(items.single);
        return;
      }
      final completeView =
          state.kind == MediaKind.track &&
          !state.loading &&
          !state.loadingMore &&
          state.nextCursor == null &&
          state.error == null;
      final rows = enqueue && item != null
          ? [item]
          : item != null && completeView
          ? state.items
          : await _bridge.collect(
              CatalogQuery(
                sourceInstanceId: state.sourceId!,
                kind: MediaKind.track,
                parent: state.parent?.reference,
                search: state.search,
                sort: state.sort,
                limit: 200,
              ),
              requestId,
            );
      if (!mounted || generation != _actionGeneration) return;
      final ordered = state.trackSort.apply(rows);
      final startIndex = item == null || enqueue
          ? 0
          : ordered.indexWhere((row) => row.reference == item.reference);
      if (startIndex < 0) {
        throw StateError('Selected track is no longer in this collection');
      }
      if (!enqueue && ref.exists(queueControllerProvider)) {
        final queue = ref.read(queueControllerProvider);
        if (queue.source?.type == QueueSourceType.catalog &&
            queue.source?.sourceInstanceId == state.sourceId &&
            queue.items.length == ordered.length &&
            ordered.indexed.every(
              (entry) =>
                  queue.items[entry.$1].catalogItem?.reference ==
                  entry.$2.reference,
            )) {
          await ref
              .read(playbackControllerProvider.notifier)
              .playIndex(startIndex);
          return;
        }
      }
      final items = await _bridge.prepare(ordered);
      if (!mounted || generation != _actionGeneration) return;
      final playback = ref.read(playbackControllerProvider.notifier);
      if (enqueue) {
        await playback.enqueueItems(items);
      } else {
        await playback.setQueueAndPlayItems(
          items,
          startIndex: startIndex,
          source: QueueSource(
            type: QueueSourceType.catalog,
            sourceInstanceId: state.sourceId,
            mediaKind: state.parent?.reference.kind.name ?? state.kind.name,
            mediaId: state.parent?.reference.id,
            label:
                '${state.source!.local ? tr('本地音乐库', 'Local library') : state.source!.name} · ${state.parent == null ? kindLabel(state.kind) : title(state.parent!)}',
          ),
        );
      }
    } catch (e) {
      if (mounted && generation == _actionGeneration) {
        setState(
          () => _actionError = DiagnosticsService.instance.failureMessage(
            e,
            operation: 'catalog',
          ),
        );
      }
    } finally {
      if (mounted && generation == _actionGeneration) {
        setState(() => _collectionId = null);
      }
    }
  }

  void _cancel() {
    final id = _collectionId;
    _actionGeneration++;
    setState(() => _collectionId = null);
    if (id != null) unawaited(_bridge.cancel(id));
  }

  Future<void> _refresh() async {
    _cancel();
    await ref
        .read(catalogControllerProvider.notifier)
        .refreshSources(preserveItems: true);
    if (!mounted) return;
    final state = ref.read(catalogControllerProvider);
    if (state.sourceId != null && state.source?.available == true) {
      final provider = catalogTreeProvider(state.sourceId!);
      if (ref.exists(provider)) await ref.read(provider).refresh();
    }
  }

  void _open(CatalogItem item) {
    _cancel();
    final controller = ref.read(catalogControllerProvider.notifier);
    if (item.reference.kind == MediaKind.folder) {
      controller.selectFolder([item]);
    } else {
      controller.open(item);
    }
  }

  void _openRelated(MediaRef reference, String title) {
    if (!mounted ||
        ref.read(catalogControllerProvider).sourceId !=
            reference.sourceInstanceId) {
      return;
    }
    _cancel();
    ref
        .read(catalogControllerProvider.notifier)
        .open(
          CatalogItem(reference: reference, title: title, artistRefs: const []),
          replacePath: true,
        );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(catalogControllerProvider);
    ref.listen(
      catalogControllerProvider.select((s) => (s.locationKey, s.trackSort)),
      (previous, next) {
        if (previous != next && _collectionId != null) _cancel();
      },
    );
    final controller = ref.read(catalogControllerProvider.notifier);
    final l = context.catalogL10n;
    final palette = ArtworkPalette.of(context);
    final categories = (state.source?.browseKinds ?? const <MediaKind>[])
        .where((kind) => kind != MediaKind.playlist)
        .toList();
    final section = state.parents.firstOrNull?.reference.kind ?? state.kind;
    final folders = section == MediaKind.folder;
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 12, 24, 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SizedBox(
            height: 44,
            child: Row(
              children: [
                Expanded(
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.baseline,
                    textBaseline: TextBaseline.alphabetic,
                    children: [
                      Text(
                        l.libraryTitle,
                        style: TextStyle(
                          fontSize: 26,
                          fontWeight: FontWeight.w600,
                          color: palette.onBackdrop,
                        ),
                      ),
                      const SizedBox(width: 8),
                      Flexible(
                        flex: 0,
                        child: ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 155),
                          child: AppSelect<String>(
                            value: state.sourceId,
                            height: 36,
                            menuWidth: 260,
                            filled: false,
                            foregroundColor: palette.onBackdrop,
                            items: [
                              for (final source in state.sources)
                                DropdownMenuItem(
                                  value: source.id,
                                  child: Text(
                                    '${source.local ? l.catalogLocalLibrary : source.name}${source.available ? '' : ' · ${l.catalogUnavailable}'}',
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),
                            ],
                            onChanged: (id) {
                              if (id != null) {
                                _cancel();
                                controller.selectSource(id);
                              }
                            },
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: l.catalogRefreshSources,
                  onPressed: _refresh,
                  icon: const Icon(Icons.refresh, size: 20),
                ),
                if (state.source?.local == true) ...[
                  IconButton(
                    tooltip: l.tooltipAddFolder,
                    onPressed: widget.onAddFolder,
                    icon: const Icon(
                      Icons.create_new_folder_outlined,
                      size: 19,
                    ),
                  ),
                  _managementMenu(),
                ],
              ],
            ),
          ),
          const SizedBox(height: 8),
          SizedBox(
            height: 36,
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  for (final kind in categories)
                    Padding(
                      padding: const EdgeInsets.only(right: 22),
                      child: Semantics(
                        selected: section == kind,
                        child: InkWell(
                          key: ValueKey('catalog-tab-${kind.name}'),
                          onTap: () {
                            _cancel();
                            controller.selectKind(kind);
                          },
                          child: AnimatedContainer(
                            duration: MediaQuery.disableAnimationsOf(context)
                                ? Duration.zero
                                : const Duration(milliseconds: 180),
                            padding: const EdgeInsets.fromLTRB(5, 4, 5, 8),
                            decoration: BoxDecoration(
                              border: Border(
                                bottom: BorderSide(
                                  width: 2,
                                  color: section == kind
                                      ? palette.onBackdrop
                                      : Colors.transparent,
                                ),
                              ),
                            ),
                            child: Text(
                              kindLabel(kind),
                              style: TextStyle(
                                fontSize: 14,
                                fontWeight: section == kind
                                    ? FontWeight.w600
                                    : FontWeight.normal,
                                color: palette.onBackdrop.withValues(
                                  alpha: section == kind ? 1 : .65,
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 8),
          Expanded(
            child: Theme(
              data: palette.applyTo(Theme.of(context)),
              child: Material(
                key: const ValueKey('catalog-content-panel'),
                color: palette.surface.withValues(alpha: .90),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(11),
                  side: BorderSide(color: palette.outline),
                ),
                clipBehavior: Clip.antiAlias,
                child: Column(
                  children: [
                    if (widget.isScanning)
                      const LinearProgressIndicator(minHeight: 2),
                    if (state.sourcesError != null) _error(state.sourcesError!),
                    if (state.source?.available == false)
                      _error(state.source?.error ?? l.catalogUnavailable),
                    if (_actionError != null) _error(_actionError!),
                    Expanded(
                      child: folders && state.source != null
                          ? CatalogFolderView(
                              key: ValueKey(state.sourceId),
                              source: state.source!,
                              path: state.parents,
                              onSelect: (path) {
                                _cancel();
                                controller.selectFolder(path);
                              },
                              onRoot: () {
                                _cancel();
                                controller.selectKind(MediaKind.folder);
                              },
                              onAddFolder: state.source!.local
                                  ? widget.onAddFolder
                                  : null,
                              child: _content(state, showDetail: false),
                            )
                          : _content(state),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _error(String message) => Padding(
    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
    child: Text(
      message,
      style: TextStyle(
        fontSize: 12,
        color: Theme.of(context).colorScheme.error,
      ),
    ),
  );
  Widget _managementMenu() {
    final l = context.catalogL10n;
    return PopupMenuButton<String>(
      tooltip: l.catalogManageLibrary,
      icon: const Icon(Icons.more_vert, size: 19),
      onSelected: (value) {
        if (value != 'folders') {
          widget.onScan(value == 'force');
          return;
        }
        showDialog<void>(
          context: context,
          builder: (context) => Dialog(
            child: SizedBox(
              width: 1100,
              height: 700,
              child: Column(
                children: [
                  Align(
                    alignment: Alignment.centerRight,
                    child: IconButton(
                      onPressed: () => Navigator.pop(context),
                      icon: const Icon(Icons.close),
                    ),
                  ),
                  Expanded(child: widget.folderManager),
                ],
              ),
            ),
          ),
        );
      },
      itemBuilder: (_) => [
        PopupMenuItem(value: 'folders', child: Text(l.catalogManageFolders)),
        PopupMenuItem(
          value: 'scan',
          enabled: !widget.isScanning,
          child: Text(l.catalogScan),
        ),
        PopupMenuItem(
          value: 'force',
          enabled: !widget.isScanning,
          child: Text(l.catalogRescan),
        ),
      ],
    );
  }

  Widget _content(CatalogState state, {bool showDetail = true}) {
    final l = context.catalogL10n;
    final numberWidth = state.kind == MediaKind.track
        ? catalogTrackNumberWidth(
            context,
            (state.total ?? 0) > state.items.length
                ? state.total!
                : state.items.length,
          )
        : 30.0;
    final controller = ref.read(catalogControllerProvider.notifier);
    final grouped =
        state.kind == MediaKind.album || state.kind == MediaKind.artist;
    final grid =
        grouped && (ref.read(_catalogGridPreferences)[state.kind] ?? true);
    final count =
        state.total ?? (state.nextCursor == null ? state.items.length : null);
    final label = count == null
        ? l.catalogLoadedCount(state.items.length)
        : switch (state.kind) {
            MediaKind.track => l.catalogSongCount(count),
            MediaKind.album => l.catalogAlbumCount(count),
            MediaKind.artist => l.catalogArtistCount(count),
            MediaKind.folder => l.catalogFolderCount(count),
            _ => l.catalogLoadedCount(count),
          };
    return Column(
      children: [
        if (showDetail && state.parent != null)
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
            child: Row(
              children: [
                IconButton(
                  tooltip: l.catalogBack,
                  style: IconButton.styleFrom(
                    fixedSize: const Size.square(36),
                    minimumSize: const Size.square(36),
                    padding: const EdgeInsets.all(8),
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(8),
                    ),
                    hoverColor: Theme.of(context).colorScheme.primary
                        .withValues(alpha: .06),
                    highlightColor: Theme.of(context).colorScheme.primary
                        .withValues(alpha: .10),
                  ),
                  onPressed: () {
                    _cancel();
                    controller.back();
                  },
                  icon: const Icon(Icons.arrow_back, size: 19),
                ),
                const SizedBox(width: 12),
                CatalogArtwork(item: state.parent!, size: 64),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        title(state.parent!),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontSize: 20,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      if (state.parent!.artist != null)
                        Text(
                          state.parent!.artist!,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                    ],
                  ),
                ),
                if (state.parent!.reference.kind == MediaKind.artist)
                  for (final kind in [MediaKind.album, MediaKind.track])
                    if (state.source!.browseKinds.contains(kind))
                      TextButton(
                        onPressed: () => controller.childKind(kind),
                        child: Text(
                          kindLabel(kind),
                          style: TextStyle(
                            fontWeight: kind == state.kind
                                ? FontWeight.bold
                                : FontWeight.normal,
                          ),
                        ),
                      ),
              ],
            ),
          ),
        if (state.search.isNotEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    '“${state.search}”',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                IconButton(
                  tooltip: l.catalogClearSearch,
                  onPressed: () => controller.setSearch(''),
                  icon: const Icon(Icons.close, size: 17),
                ),
              ],
            ),
          ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 2),
          child: Row(
            children: [
              if (state.kind == MediaKind.track) ...[
                TextButton.icon(
                  onPressed: _collectionId != null
                      ? _cancel
                      : state.source?.available == true &&
                            !state.loading &&
                            state.items.isNotEmpty
                      ? () => _play()
                      : null,
                  icon: Container(
                    width: 28,
                    height: 28,
                    decoration: BoxDecoration(
                      color: ArtworkPalette.of(context).accent,
                      borderRadius: BorderRadius.circular(7),
                    ),
                    child: Icon(
                      _collectionId == null
                          ? Icons.play_arrow_rounded
                          : Icons.close,
                      size: 20,
                      color: ArtworkPalette.of(context).onAccent,
                    ),
                  ),
                  label: Text(
                    _collectionId == null
                        ? state.search.isEmpty
                              ? l.catalogPlayAll
                              : l.catalogPlaySearchResults
                        : l.catalogCancelPreparation,
                  ),
                ),
                const SizedBox(width: 10),
              ],
              Expanded(
                child: Text(
                  state.loading && state.items.isEmpty ? '' : label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontSize: 12,
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
              if (state.kind != MediaKind.track &&
                  state.source?.sorts.isNotEmpty == true)
                AppSelect<CatalogSort>(
                  value: state.sort,
                  width: 132,
                  height: 36,
                  filled: false,
                  items: [
                    for (final sort in state.source!.sorts)
                      DropdownMenuItem(
                        value: sort,
                        child: Text(
                          sort == CatalogSort.default_
                              ? l.catalogDefaultOrder
                              : l.catalogTitle,
                        ),
                      ),
                  ],
                  onChanged: (sort) {
                    if (sort != null) controller.setSort(sort);
                  },
                ),
              if (grouped) ...[
                IconButton(
                  tooltip: l.catalogGrid,
                  isSelected: grid,
                  onPressed: () => setState(
                    () => ref.read(_catalogGridPreferences)[state.kind] = true,
                  ),
                  icon: const Icon(Icons.grid_view_rounded, size: 18),
                ),
                IconButton(
                  tooltip: l.catalogList,
                  isSelected: !grid,
                  onPressed: () => setState(
                    () => ref.read(_catalogGridPreferences)[state.kind] = false,
                  ),
                  icon: const Icon(Icons.view_list_rounded, size: 19),
                ),
              ],
            ],
          ),
        ),
        if (state.loading && state.items.isEmpty)
          const LinearProgressIndicator(minHeight: 2),
        if (state.kind == MediaKind.track)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10),
            child: CatalogTrackHeader(
              numberWidth: numberWidth,
              sort: state.trackSort,
              onSort: controller.sortTracks,
            ),
          ),
        Expanded(
          child: CatalogItemsView(
            key: ValueKey('${state.locationKey}:$grid:${state.trackSort.key}'),
            locationKey: '${state.locationKey}:$grid:${state.trackSort.key}',
            items: ref.watch(catalogVisibleItemsProvider),
            grid: grid,
            itemExtent: state.kind == MediaKind.track ? 48 : null,
            footer: _footer(state),
            itemBuilder: (item, index) {
              if (item.reference.kind == MediaKind.track) {
                return CatalogTrackRow(
                  numberWidth: numberWidth,
                  key: ValueKey(item.reference),
                  item: item,
                  index: index,
                  current:
                      widget.currentItem?.catalogItem?.reference ==
                          item.reference ||
                      (state.source?.local == true &&
                          item.localTrackId != null &&
                          widget.currentItem?.id == item.localTrackId!.toInt()),
                  onPlay: state.source?.available == true
                      ? () => _play(item: item)
                      : null,
                  localArtists: state.source?.local == true,
                  onOpenArtist: state.source?.available == true
                      ? _openRelated
                      : null,
                  onOpenAlbum:
                      state.source?.available == true &&
                          item.albumRef?.kind == MediaKind.album &&
                          item.albumRef?.sourceInstanceId ==
                              item.reference.sourceInstanceId
                      ? () => _openRelated(item.albumRef!, item.album ?? '')
                      : null,
                  actions: _trackActions(
                    item,
                    state.source?.local == true,
                    state.source?.available == true,
                  ),
                );
              }
              if (grid) {
                return CatalogCollectionCard(
                  key: ValueKey(item.reference),
                  item: item,
                  onOpen: state.source?.available == true
                      ? () => _open(item)
                      : null,
                );
              }
              return ListTile(
                key: ValueKey(item.reference),
                leading: CatalogArtwork(item: item, size: 44),
                title: Text(
                  title(item),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                subtitle: item.artist != null
                    ? Text(
                        item.artist!,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      )
                    : null,
                trailing: const Icon(Icons.chevron_right, size: 18),
                onTap: state.source?.available == true
                    ? () => _open(item)
                    : null,
              );
            },
          ),
        ),
      ],
    );
  }

  Widget _trackActions(CatalogItem item, bool local, bool available) {
    final l = context.catalogL10n;
    return LayoutBuilder(
      builder: (context, constraints) => Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          if (constraints.maxWidth >= 64)
            SizedBox(
              width: 30,
              child: IconButton(
                padding: EdgeInsets.zero,
                tooltip: l.catalogAddToQueue,
                onPressed: available
                    ? () => _play(item: item, enqueue: true)
                    : null,
                icon: const Icon(Icons.playlist_add, size: 18),
              ),
            ),
          SizedBox(
            width: 30,
            child: PopupMenuButton<String>(
              enabled: available,
              padding: EdgeInsets.zero,
              icon: const Icon(Icons.more_vert, size: 18),
              onSelected: (value) {
                if (value == 'enqueue') {
                  _play(item: item, enqueue: true);
                  return;
                }
                final id = item.localTrackId?.toInt();
                if (id == null) return;
                final library = ref.read(libraryControllerProvider.notifier);
                if (value == 'like') {
                  library.setTrackLiked(
                    id,
                    !ref
                        .read(libraryControllerProvider)
                        .likedTrackIds
                        .contains(id),
                  );
                } else {
                  library.addTrackToPlaylist(int.parse(value), id);
                }
              },
              itemBuilder: (_) => [
                PopupMenuItem(
                  value: 'enqueue',
                  child: Text(l.catalogAddToQueue),
                ),
                if (local && item.localTrackId != null) ...[
                  PopupMenuItem(
                    value: 'like',
                    child: Text(
                      ref
                              .read(libraryControllerProvider)
                              .likedTrackIds
                              .contains(item.localTrackId!.toInt())
                          ? l.catalogUnlike
                          : l.catalogLike,
                    ),
                  ),
                  for (final playlist
                      in ref.read(libraryControllerProvider).playlists)
                    PopupMenuItem(
                      value: playlist.id.toString(),
                      child: Text(l.catalogAddTo(playlist.name)),
                    ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _footer(CatalogState state) {
    final l = context.catalogL10n;
    final controller = ref.read(catalogControllerProvider.notifier);
    if (state.error != null) {
      return Column(
        children: [
          _error(state.error!),
          Wrap(
            children: [
              TextButton(
                onPressed: controller.refresh,
                child: Text(l.catalogRefreshRetry),
              ),
              if (state.nextCursor != null)
                TextButton(
                  onPressed: controller.retryLoading,
                  child: Text(l.catalogRetryPage),
                ),
            ],
          ),
        ],
      );
    }
    if (state.loadingMore) {
      return const Padding(
        padding: EdgeInsets.all(16),
        child: Center(child: CircularProgressIndicator()),
      );
    }
    if (state.items.isEmpty && !state.loading) {
      return Padding(
        padding: const EdgeInsets.all(32),
        child: Center(child: Text(l.catalogNoItems)),
      );
    }
    return const SizedBox(height: 16);
  }
}
