import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/decoder_extension_support.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/playability_messages.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/player/track_playability_utils.dart';
import 'package:stellatune/ui/pages/playlists/logic/playlists_plugin_bridge_service.dart';
import 'package:stellatune/ui/pages/playlists/models/playlists_data_models.dart';
import 'package:stellatune/ui/pages/playlists/widgets/playlists_page_header.dart';
import 'package:stellatune/ui/pages/playlists/widgets/playlist_track_panes.dart';
import 'package:stellatune/ui/pages/playlists/widgets/playlists_sidebar_widgets.dart';

class PlaylistsPage extends ConsumerStatefulWidget {
  const PlaylistsPage({super.key, this.useGlobalTopBar = false});

  final bool useGlobalTopBar;

  @override
  ConsumerState<PlaylistsPage> createState() => PlaylistsPageState();
}

class PlaylistsPageState extends ConsumerState<PlaylistsPage> {
  static const int _pluginPlaylistPageSize = 500;
  static const int _pluginPlaylistEagerLoadThreshold = 10000;

  final _librarySearchController = TextEditingController();
  final _pluginSearchController = TextEditingController();
  bool _playlistsPanelOpen = false;
  bool _autoSelecting = false;
  final TrackPlayabilityProbe _playabilityProbe = TrackPlayabilityProbe();
  Map<int, String> _blockedReasonByTrackId = const <int, String>{};
  List<PluginPlaylistEntry> _pluginPlaylists = const <PluginPlaylistEntry>[];
  String? _selectedPluginPlaylistKey;
  List<QueueItem> _pluginPlaylistTracks = const <QueueItem>[];
  bool _loadingPluginPlaylists = false;
  bool _loadingPluginPlaylistTracks = false;
  bool _loadingPluginPlaylistMore = false;
  int _pluginPlaylistNextOffset = 0;
  bool _pluginPlaylistHasMore = false;
  int _pluginTrackLoadSeq = 0;
  String? _pluginPlaylistError;
  final PlaylistsPluginBridgeService _pluginBridgeService =
      const PlaylistsPluginBridgeService();
  final Map<String, SparseTrackCacheEntry<QueueItem>>
  _pluginPlaylistTracksCache = <String, SparseTrackCacheEntry<QueueItem>>{};

  bool get isPlaylistsPanelOpen => _playlistsPanelOpen;

  void togglePlaylistsPanel() {
    _updateUi(() => _playlistsPanelOpen = !_playlistsPanelOpen);
  }

  Future<void> createPlaylistFromTopBar() => _createPlaylist(context);

  @override
  void initState() {
    super.initState();
    unawaited(_refreshDecoderExtensionSupport());
    unawaited(_refreshPluginPlaylists());
  }

  @override
  void dispose() {
    _librarySearchController.dispose();
    _pluginSearchController.dispose();
    super.dispose();
  }

  void _syncSearchController(TextEditingController controller, String query) {
    if (controller.text == query) return;
    controller.value = TextEditingValue(
      text: query,
      selection: TextSelection.collapsed(offset: query.length),
    );
  }

  void _applyBlockedReasonByTrackId(Map<int, String> blocked) {
    if (hasSameTrackBlockedReasons(_blockedReasonByTrackId, blocked)) return;
    _updateUi(() => _blockedReasonByTrackId = blocked);
  }

  void _updateUi(VoidCallback updater) => setState(updater);

  void _onViewportRangeChanged(int startIndex, int endIndex) {
    if (!_playabilityProbe.updateViewportRange(startIndex, endIndex)) {
      return;
    }
    final results = ref.read(libraryControllerProvider).results;
    unawaited(_refreshTrackPlayability(results));
  }

  Future<void> _refreshDecoderExtensionSupport() async {
    try {
      await DecoderExtensionSupportCache.instance.refresh(
        ref.read(playerBridgeProvider),
      );
    } catch (_) {}
  }

  Future<void> _refreshTrackPlayability(
    List<TrackLite> items, {
    bool force = false,
  }) async {
    final l10n = AppLocalizations.of(context);
    if (l10n == null) return;
    String localizeReason(String rawReason) =>
        localizePlayabilityReason(l10n, rawReason);

    if (items.isEmpty) {
      if (!mounted) return;
      _applyBlockedReasonByTrackId(const <int, String>{});
      return;
    }

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _applyBlockedReasonByTrackId(
        _playabilityProbe.buildBlockedReasons(
          items,
          localizeReason: localizeReason,
        ),
      );
    });

    final blocked = await _playabilityProbe.refreshBlockedReasons(
      items: items,
      force: force,
      localizeReason: localizeReason,
      ensureDecoderSupport: _refreshDecoderExtensionSupport,
      readDecoderSnapshot: () =>
          DecoderExtensionSupportCache.instance.snapshotOrNull,
    );
    if (!mounted || blocked == null) return;
    _applyBlockedReasonByTrackId(blocked);
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
    final libraryQuery = ref.watch(
      libraryControllerProvider.select((s) => s.query),
    );
    _syncSearchController(_librarySearchController, libraryQuery);
    final results = ref.watch(
      libraryControllerProvider.select((s) => s.results),
    );
    // TODO(local-sparse): Keep local list in-memory for now.
    // Switch to true sparse range loading after library events/bridge
    // provide stable offset-based incremental fetch semantics.
    final localSparseSource = InMemorySparseTrackSource<TrackLite>(
      cacheKey: 'local::$selectedPlaylistId',
      items: results,
      pageSize: _pluginPlaylistPageSize,
      eagerLoadThreshold: _pluginPlaylistEagerLoadThreshold,
    );
    final localTracks = localSparseSource.items;
    unawaited(_refreshTrackPlayability(localTracks));
    final likedTrackIds = ref.watch(
      libraryControllerProvider.select((s) => s.likedTrackIds),
    );
    final queueSourceSnapshot = ref.watch(
      queueControllerProvider.select((s) => s.sourceLabel),
    );
    final selectedPluginPlaylist = _selectedPluginPlaylist();
    if (selectedPluginPlaylist == null) {
      _ensurePlaylistSelected(playlists, selectedPlaylistId);
    }

    PlaylistLite? selectedPlaylist;
    if (selectedPluginPlaylist == null && selectedPlaylistId != null) {
      for (final p in playlists) {
        if (p.id.toInt() == selectedPlaylistId) {
          selectedPlaylist = p;
          break;
        }
      }
    }

    final selectionSourceLabel = selectedPluginPlaylist != null
        ? '${selectedPluginPlaylist.sourceLabel} - ${selectedPluginPlaylist.title}'
        : (selectedPlaylist == null
              ? l10n.queueSourceUnset
              : _playlistDisplayName(l10n, selectedPlaylist));
    final queueSourceLabel = (queueSourceSnapshot ?? '').trim().isEmpty
        ? l10n.queueSourceUnset
        : queueSourceSnapshot!.trim();
    final pluginFilterActive = _pluginSearchController.text.trim().isNotEmpty;
    final pluginVisibleTracks = _filteredPluginTracks(
      _pluginSearchController.text,
    );

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
                  child: selectedPluginPlaylist == null
                      ? PlaylistTracksPane(
                          searchController: _librarySearchController,
                          queueSourceLabel: queueSourceLabel,
                          selectedLabel: selectedPlaylist == null
                              ? l10n.queueSourceUnset
                              : _playlistDisplayName(l10n, selectedPlaylist),
                          playlists: playlists,
                          selectedPlaylistId: selectedPlaylistId,
                          results: localTracks,
                          likedTrackIds: likedTrackIds,
                          coverDir: coverDir,
                          onSearchChanged: (q) => ref
                              .read(libraryControllerProvider.notifier)
                              .setQuery(q),
                          onActivate: (index, items) async {
                            final source = QueueSource(
                              type: QueueSourceType.playlist,
                              playlistId: selectedPlaylistId,
                              label: selectionSourceLabel,
                            );
                            await ref
                                .read(playbackControllerProvider.notifier)
                                .setQueueAndPlayTracks(
                                  items,
                                  startIndex: index,
                                  source: source,
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
                                .addTrackToPlaylist(
                                  playlistId,
                                  track.id.toInt(),
                                );
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
                                  trackIds: tracks
                                      .map((t) => t.id.toInt())
                                      .toList(),
                                );
                          },
                          onBatchRemoveFromCurrentPlaylist:
                              selectedPlaylistId == null
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
                          blockedReasonByTrackId: _blockedReasonByTrackId,
                          onViewportRangeChanged: _onViewportRangeChanged,
                        )
                      : PluginPlaylistTracksPane(
                          searchController: _pluginSearchController,
                          queueSourceLabel: queueSourceLabel,
                          selectedLabel:
                              '${selectedPluginPlaylist.sourceLabel} - ${selectedPluginPlaylist.title}',
                          sourceLabel: selectedPluginPlaylist.sourceLabel,
                          tracks: pluginVisibleTracks,
                          loading: _loadingPluginPlaylistTracks,
                          loadingMore: _loadingPluginPlaylistMore,
                          hasMore: _pluginPlaylistHasMore,
                          filterActive: pluginFilterActive,
                          error: _pluginPlaylistError,
                          onSearchChanged: (_) => _updateUi(() {}),
                          onLoadMore: _loadMorePluginPlaylistTracks,
                          onActivate: (index, items) async {
                            final source = QueueSource(
                              type: QueueSourceType.all,
                              label: selectionSourceLabel,
                            );
                            await ref
                                .read(playbackControllerProvider.notifier)
                                .setQueueAndPlayItems(
                                  items,
                                  startIndex: index,
                                  source: source,
                                );
                          },
                          onEnqueue: (item) async {
                            await ref
                                .read(playbackControllerProvider.notifier)
                                .enqueueItems([item]);
                          },
                        ),
                ),
                if (_playlistsPanelOpen)
                  Positioned.fill(
                    child: GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onTap: () => _updateUi(() => _playlistsPanelOpen = false),
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
                      child: PlaylistsDrawerPanel(
                        playlists: playlists,
                        selectedPlaylistId: selectedPlaylistId,
                        pluginPlaylists: _pluginPlaylists,
                        selectedPluginPlaylistKey: _selectedPluginPlaylistKey,
                        onSelect: (id) {
                          if (_selectedPluginPlaylistKey != null) {
                            _updateUi(() {
                              _selectedPluginPlaylistKey = null;
                              _pluginPlaylistTracks = const <QueueItem>[];
                              _pluginPlaylistError = null;
                            });
                          }
                          ref
                              .read(libraryControllerProvider.notifier)
                              .selectPlaylist(id);
                        },
                        onSelectPlugin: (entry) async {
                          await _selectPluginPlaylist(entry);
                        },
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
                        onRefreshPlugins: _refreshPluginPlaylists,
                        pluginLoading: _loadingPluginPlaylists,
                        pluginError: _pluginPlaylistError,
                        onClose: () =>
                            _updateUi(() => _playlistsPanelOpen = false),
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
            PlaylistsPageHeader(
              title: l10n.playlistSectionTitle,
              panelTooltip: l10n.playlistSectionTitle,
              createTooltip: l10n.playlistCreateTooltip,
              onTogglePanel: togglePlaylistsPanel,
              onCreatePlaylist: createPlaylistFromTopBar,
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
}

extension _PlaylistsDialogLogic on PlaylistsPageState {
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

extension _PlaylistsPluginLogic on PlaylistsPageState {
  Future<void> _refreshPluginPlaylists() async {
    if (_loadingPluginPlaylists) return;
    _updateUi(() {
      _loadingPluginPlaylists = true;
      _pluginPlaylistError = null;
    });
    try {
      final result = await _pluginBridgeService.fetchPlaylists(
        bridge: ref.read(playerBridgeProvider),
        settings: ref.read(settingsStoreProvider),
      );
      final merged = result.entries;
      final validKeys = merged.map((entry) => entry.key).toSet();
      _pluginPlaylistTracksCache.removeWhere(
        (key, _) => !validKeys.contains(key),
      );

      if (!mounted) return;
      _updateUi(() {
        _pluginPlaylists = merged;
        _pluginPlaylistError = result.aggregatedError;
        if (_selectedPluginPlaylistKey != null &&
            !_pluginPlaylists.any((e) => e.key == _selectedPluginPlaylistKey)) {
          _selectedPluginPlaylistKey = null;
          _pluginPlaylistTracks = const <QueueItem>[];
        }
      });
    } catch (e) {
      if (!mounted) return;
      _updateUi(() => _pluginPlaylistError = e.toString());
    } finally {
      if (mounted) {
        _updateUi(() => _loadingPluginPlaylists = false);
      }
    }
  }

  PluginPlaylistEntry? _selectedPluginPlaylist() {
    final key = _selectedPluginPlaylistKey;
    if (key == null || key.isEmpty) return null;
    for (final item in _pluginPlaylists) {
      if (item.key == key) return item;
    }
    return null;
  }

  PluginSparseTrackSource _pluginTrackSourceFor(PluginPlaylistEntry entry) {
    return PluginSparseTrackSource(
      entry: entry,
      pageSize: PlaylistsPageState._pluginPlaylistPageSize,
      eagerLoadThreshold: PlaylistsPageState._pluginPlaylistEagerLoadThreshold,
      fetcher: ({required int offset, int? limit}) =>
          _pluginBridgeService.fetchTrackPage(
            bridge: ref.read(playerBridgeProvider),
            entry: entry,
            pageSize: PlaylistsPageState._pluginPlaylistPageSize,
            offset: offset,
            limit: limit,
          ),
    );
  }

  bool _canContinueEagerLoad(
    SparseTrackSource<QueueItem> source,
    int fetchedRows,
  ) {
    if (!source.eagerPreferred) return false;
    return fetchedRows < source.eagerLoadThreshold;
  }

  void _cachePluginPlaylistTracks(
    SparseTrackSource<QueueItem> source, {
    required PluginPlaylistEntry entry,
    required List<QueueItem> items,
    required int nextOffset,
    required bool hasMore,
  }) {
    _pluginPlaylistTracksCache[source.cacheKey] =
        SparseTrackCacheEntry<QueueItem>(
          items: List<QueueItem>.unmodifiable(items),
          nextOffset: nextOffset,
          hasMore: hasMore,
          pageSize: PlaylistsPageState._pluginPlaylistPageSize,
          knownTotalCount: source.knownTotalCount,
        );
  }

  bool _restorePluginPlaylistTracksFromCache(
    SparseTrackSource<QueueItem> source, {
    required PluginPlaylistEntry entry,
  }) {
    final cached = _pluginPlaylistTracksCache[source.cacheKey];
    if (cached == null) return false;
    if (cached.pageSize != source.pageSize) {
      _pluginPlaylistTracksCache.remove(source.cacheKey);
      logger.d(
        'plugin playlist tracks: cache_invalidate page_size plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} cached_page_size=${cached.pageSize} expected=${source.pageSize}',
      );
      return false;
    }
    if (cached.knownTotalCount != null &&
        source.knownTotalCount != null &&
        cached.knownTotalCount != source.knownTotalCount) {
      _pluginPlaylistTracksCache.remove(source.cacheKey);
      logger.d(
        'plugin playlist tracks: cache_invalidate track_count plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} cached_track_count=${cached.knownTotalCount} latest_track_count=${source.knownTotalCount}',
      );
      return false;
    }
    _updateUi(() {
      _pluginPlaylistError = null;
      _loadingPluginPlaylistTracks = false;
      _loadingPluginPlaylistMore = false;
      _pluginPlaylistTracks = cached.items;
      _pluginPlaylistNextOffset = cached.nextOffset;
      _pluginPlaylistHasMore = cached.hasMore;
    });
    logger.d(
      'plugin playlist tracks: cache_hit plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} tracks=${cached.items.length} offset=${cached.nextOffset} has_more=${cached.hasMore}',
    );
    unawaited(
      _revalidatePluginPlaylistCache(source, entry: entry, cached: cached),
    );
    return true;
  }

  Future<void> _revalidatePluginPlaylistCache(
    SparseTrackSource<QueueItem> source, {
    required PluginPlaylistEntry entry,
    required SparseTrackCacheEntry<QueueItem> cached,
  }) async {
    if (cached.hasMore) return;
    final selectedKeyAtStart = _selectedPluginPlaylistKey;
    if (selectedKeyAtStart != entry.key) return;
    final loadSeq = _pluginTrackLoadSeq;
    try {
      final head = await source.fetchPage(offset: 0, limit: 1);
      final tail = await source.fetchPage(offset: cached.nextOffset, limit: 1);
      if (!mounted ||
          _selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }

      var stale = false;
      var reason = '';
      if (cached.items.isEmpty && head.items.isNotEmpty) {
        stale = true;
        reason = 'empty_cache_but_remote_has_items';
      } else if (cached.items.isNotEmpty && head.items.isNotEmpty) {
        if (cached.items.first.stableTrackKey !=
            head.items.first.stableTrackKey) {
          stale = true;
          reason = 'first_track_changed';
        }
      }
      if (!stale && tail.fetchedCount > 0) {
        stale = true;
        reason = 'tail_has_new_items';
      }
      if (!stale) return;

      logger.d(
        'plugin playlist tracks: cache_stale plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} reason=$reason cached_tracks=${cached.items.length} cached_offset=${cached.nextOffset}',
      );
      _pluginPlaylistTracksCache.remove(source.cacheKey);
      if (!mounted ||
          _selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      await _loadPluginPlaylistTracks(entry);
    } catch (e, s) {
      logger.d(
        'plugin playlist tracks: cache_revalidate_skipped plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} reason=$e',
        error: e,
        stackTrace: s,
      );
    }
  }

  Future<void> _selectPluginPlaylist(PluginPlaylistEntry entry) async {
    if (_selectedPluginPlaylistKey == entry.key) return;
    _updateUi(() {
      _selectedPluginPlaylistKey = entry.key;
      _pluginPlaylistTracks = const <QueueItem>[];
      _pluginPlaylistError = null;
      _pluginPlaylistNextOffset = 0;
      _pluginPlaylistHasMore = false;
      _loadingPluginPlaylistMore = false;
    });
    await _loadPluginPlaylistTracks(entry);
  }

  Future<void> _loadPluginPlaylistTracks(PluginPlaylistEntry entry) async {
    if (_loadingPluginPlaylistTracks) return;
    final loadSeq = ++_pluginTrackLoadSeq;
    final source = _pluginTrackSourceFor(entry);
    if (_restorePluginPlaylistTracksFromCache(source, entry: entry)) {
      return;
    }
    _updateUi(() {
      _loadingPluginPlaylistTracks = true;
      _loadingPluginPlaylistMore = false;
      _pluginPlaylistError = null;
      _pluginPlaylistTracks = const <QueueItem>[];
      _pluginPlaylistNextOffset = 0;
      _pluginPlaylistHasMore = false;
    });
    try {
      final preferEager = source.eagerPreferred;
      final merged = <QueueItem>[];
      final seen = <String>{};
      var offset = 0;
      var hasMore = false;
      logger.d(
        'plugin playlist tracks: request plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} track_count=${entry.trackCount} eager=$preferEager',
      );

      final firstPage = await source.fetchPage(offset: offset);
      if (!mounted) return;
      if (_selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      _appendUniqueQueueItems(merged, firstPage.items, seenKeys: seen);
      offset += firstPage.fetchedCount;
      hasMore = firstPage.hasMore;
      final continueEager =
          hasMore &&
          firstPage.fetchedCount > 0 &&
          _canContinueEagerLoad(source, offset);
      logger.d(
        'plugin playlist tracks: first_page_loaded plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} fetched=${firstPage.fetchedCount} merged=${merged.length} next_offset=$offset has_more=$hasMore continue_eager=$continueEager',
      );
      _updateUi(() {
        _pluginPlaylistTracks = merged;
        _pluginPlaylistNextOffset = offset;
        _pluginPlaylistHasMore = hasMore;
        _loadingPluginPlaylistTracks = false;
        _loadingPluginPlaylistMore = continueEager;
      });
      _cachePluginPlaylistTracks(
        source,
        entry: entry,
        items: merged,
        nextOffset: offset,
        hasMore: hasMore,
      );

      if (!continueEager) {
        if (hasMore) {
          logger.d(
            'plugin playlist tracks: switch_to_paged plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} loaded=${merged.length} next_offset=$offset threshold=${PlaylistsPageState._pluginPlaylistEagerLoadThreshold}',
          );
        } else {
          logger.d(
            'plugin playlist tracks: eager_load_done plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} loaded=${merged.length}',
          );
        }
        return;
      }

      var page = 1;
      while (true) {
        final pageResult = await source.fetchPage(offset: offset);
        if (!mounted) return;
        if (_selectedPluginPlaylistKey != entry.key ||
            loadSeq != _pluginTrackLoadSeq) {
          return;
        }
        _appendUniqueQueueItems(merged, pageResult.items, seenKeys: seen);
        offset += pageResult.fetchedCount;
        hasMore = pageResult.hasMore;
        page += 1;

        logger.d(
          'plugin playlist tracks: eager_page_loaded plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} page=$page fetched=${pageResult.fetchedCount} merged=${merged.length} next_offset=$offset has_more=$hasMore',
        );

        final keepEager =
            hasMore &&
            pageResult.fetchedCount > 0 &&
            _canContinueEagerLoad(source, offset);
        _updateUi(() {
          _pluginPlaylistTracks = List<QueueItem>.from(merged);
          _pluginPlaylistNextOffset = offset;
          _pluginPlaylistHasMore = hasMore;
          _loadingPluginPlaylistMore = keepEager;
        });
        _cachePluginPlaylistTracks(
          source,
          entry: entry,
          items: merged,
          nextOffset: offset,
          hasMore: hasMore,
        );
        if (!keepEager) break;
      }

      if (!mounted) return;
      if (_selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      if (hasMore) {
        logger.d(
          'plugin playlist tracks: switch_to_paged plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} loaded=${merged.length} next_offset=$offset threshold=${PlaylistsPageState._pluginPlaylistEagerLoadThreshold}',
        );
      } else {
        logger.d(
          'plugin playlist tracks: eager_load_done plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} loaded=${merged.length}',
        );
      }
    } catch (e, s) {
      logger.w(
        'plugin playlist tracks failed plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId}',
        error: e,
        stackTrace: s,
      );
      if (!mounted ||
          _selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      _updateUi(() => _pluginPlaylistError = e.toString());
    } finally {
      if (mounted &&
          _selectedPluginPlaylistKey == entry.key &&
          loadSeq == _pluginTrackLoadSeq) {
        _updateUi(() {
          _loadingPluginPlaylistTracks = false;
          _loadingPluginPlaylistMore = false;
        });
      }
    }
  }

  Future<void> _loadMorePluginPlaylistTracks() async {
    final entry = _selectedPluginPlaylist();
    if (entry == null) return;
    if (_loadingPluginPlaylistTracks ||
        _loadingPluginPlaylistMore ||
        !_pluginPlaylistHasMore) {
      return;
    }

    final loadSeq = _pluginTrackLoadSeq;
    final offset = _pluginPlaylistNextOffset;
    final source = _pluginTrackSourceFor(entry);
    _updateUi(() => _loadingPluginPlaylistMore = true);
    try {
      final pageResult = await source.fetchPage(offset: offset);
      final fetchedCount = pageResult.fetchedCount;
      final hasMore = pageResult.hasMore;
      if (!mounted) return;
      if (_selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }

      logger.d(
        'plugin playlist tracks: load_more plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} offset=$offset merged_add=${pageResult.items.length} fetched=$fetchedCount has_more=$hasMore',
      );
      late final List<QueueItem> merged;
      late final int nextOffset;
      _updateUi(() {
        final seen = <String>{
          for (final t in _pluginPlaylistTracks) t.stableTrackKey,
        };
        merged = List<QueueItem>.from(_pluginPlaylistTracks);
        _appendUniqueQueueItems(merged, pageResult.items, seenKeys: seen);
        _pluginPlaylistTracks = merged;
        nextOffset = offset + fetchedCount;
        _pluginPlaylistNextOffset = nextOffset;
        _pluginPlaylistHasMore = hasMore;
      });
      _cachePluginPlaylistTracks(
        source,
        entry: entry,
        items: merged,
        nextOffset: nextOffset,
        hasMore: hasMore,
      );
    } catch (e, s) {
      logger.w(
        'plugin playlist tracks load more failed plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId}',
        error: e,
        stackTrace: s,
      );
      if (!mounted ||
          _selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      _updateUi(() => _pluginPlaylistError = e.toString());
    } finally {
      if (mounted &&
          _selectedPluginPlaylistKey == entry.key &&
          loadSeq == _pluginTrackLoadSeq) {
        _updateUi(() => _loadingPluginPlaylistMore = false);
      }
    }
  }

  List<QueueItem> _filteredPluginTracks(String query) {
    final q = query.trim().toLowerCase();
    if (q.isEmpty) return _pluginPlaylistTracks;
    return _pluginPlaylistTracks.where((item) {
      final title = (item.title ?? '').toLowerCase();
      final artist = (item.artist ?? '').toLowerCase();
      final album = (item.album ?? '').toLowerCase();
      return title.contains(q) || artist.contains(q) || album.contains(q);
    }).toList();
  }

  void _appendUniqueQueueItems(
    List<QueueItem> target,
    Iterable<QueueItem> incoming, {
    required Set<String> seenKeys,
  }) {
    for (final item in incoming) {
      if (seenKeys.add(item.stableTrackKey)) {
        target.add(item);
      }
    }
  }
}
