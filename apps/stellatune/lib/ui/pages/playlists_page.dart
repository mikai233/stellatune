import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
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
import 'package:stellatune/ui/pages/playlists/logic/plugin_playlists_controller.dart';
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
  String? _catalogCollectionId;
  int _catalogPlayGeneration = 0;
  CatalogBridge? _catalogBridge;

  void _cancelCatalogPlay() {
    final id = _catalogCollectionId;
    _catalogPlayGeneration++;
    if (mounted) setState(() => _catalogCollectionId = null);
    if (id != null) unawaited(_catalogBridge?.cancel(id));
  }

  Future<void> _playAllPluginTracks() async {
    final entry = ref.read(pluginPlaylistsControllerProvider).selection?.entry;
    if (entry == null) return;
    _cancelCatalogPlay();
    final generation = ++_catalogPlayGeneration;
    final id = 'playlist-${DateTime.now().microsecondsSinceEpoch}';
    final bridge = ref.read(catalogBridgeProvider);
    _catalogBridge = bridge;
    setState(() => _catalogCollectionId = id);
    try {
      final rows = await bridge.collect(
        CatalogQuery(
          sourceInstanceId: entry.sourceId,
          kind: MediaKind.track,
          parent: MediaRef(
            sourceInstanceId: entry.sourceId,
            kind: MediaKind.playlist,
            id: entry.playlistId,
          ),
          search: '',
          sort: CatalogSort.default_,
          limit: 200,
        ),
        id,
      );
      if (!mounted ||
          generation != _catalogPlayGeneration ||
          ref.read(pluginPlaylistsControllerProvider).selection?.entry.key !=
              entry.key) {
        return;
      }
      final items = await bridge.prepare(rows);
      if (!mounted ||
          generation != _catalogPlayGeneration ||
          ref.read(pluginPlaylistsControllerProvider).selection?.entry.key !=
              entry.key) {
        return;
      }
      await ref
          .read(playbackControllerProvider.notifier)
          .setQueueAndPlayItems(
            items,
            source: QueueSource(
              type: QueueSourceType.catalog,
              sourceInstanceId: entry.sourceId,
              mediaKind: 'playlist',
              mediaId: entry.playlistId,
              label: entry.title,
            ),
          );
    } catch (e) {
      if (mounted && generation == _catalogPlayGeneration) {
        DiagnosticsService.instance.report(e, operation: 'playlist');
      }
    } finally {
      if (mounted && generation == _catalogPlayGeneration) {
        setState(() => _catalogCollectionId = null);
      }
    }
  }

  final _librarySearchController = TextEditingController();
  final _pluginSearchController = TextEditingController();
  bool _playlistsPanelOpen = false;
  bool _autoSelecting = false;
  final TrackPlayabilityProbe _playabilityProbe = TrackPlayabilityProbe();
  Map<int, String> _blockedReasonByTrackId = const <int, String>{};
  bool get isPlaylistsPanelOpen => _playlistsPanelOpen;

  void togglePlaylistsPanel() {
    _updateUi(() => _playlistsPanelOpen = !_playlistsPanelOpen);
  }

  Future<void> createPlaylistFromTopBar() => _createPlaylist(context);

  @override
  void initState() {
    super.initState();
    ref.listenManual(pluginPlaylistsControllerProvider, (previous, next) {
      if (_catalogCollectionId != null &&
          previous?.selection?.entry.key != next.selection?.entry.key) {
        _cancelCatalogPlay();
      }
    });
    unawaited(_refreshDecoderExtensionSupport());
    unawaited(
      Future<void>.microtask(() async {
        if (!mounted) return;
        await ref.read(pluginPlaylistsControllerProvider.notifier).refresh();
      }),
    );
  }

  @override
  void dispose() {
    _catalogPlayGeneration++;
    if (_catalogCollectionId != null) {
      unawaited(_catalogBridge?.cancel(_catalogCollectionId!));
    }
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
    final localTracks = ref.watch(
      libraryControllerProvider.select((s) => s.results),
    );
    unawaited(_refreshTrackPlayability(localTracks));
    final likedTrackIds = ref.watch(
      libraryControllerProvider.select((s) => s.likedTrackIds),
    );
    final queueSourceSnapshot = ref.watch(
      queueControllerProvider.select((s) => s.sourceLabel),
    );
    final pluginState = ref.watch(pluginPlaylistsControllerProvider);
    final pluginController = ref.read(
      pluginPlaylistsControllerProvider.notifier,
    );
    final pluginSelection = pluginState.selection;
    final selectedPluginPlaylist = pluginSelection?.entry;
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
    final pluginVisibleTracks =
        pluginSelection?.filter(_pluginSearchController.text) ??
        const <QueueItem>[];

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
                          bridge: ref.read(playerBridgeProvider),
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
                          onPlayAll: _playAllPluginTracks,
                          onCancelPlayAll: _catalogCollectionId == null
                              ? null
                              : _cancelCatalogPlay,
                          key: ValueKey(selectedPluginPlaylist.key),
                          searchController: _pluginSearchController,
                          queueSourceLabel: queueSourceLabel,
                          selectedLabel:
                              '${selectedPluginPlaylist.sourceLabel} - ${selectedPluginPlaylist.title}',
                          sourceLabel: selectedPluginPlaylist.sourceLabel,
                          tracks: pluginVisibleTracks,
                          loading: pluginSelection!.loading,
                          loadingMore: pluginSelection.loadingMore,
                          hasMore: pluginSelection.hasMore,
                          filterActive: pluginFilterActive,
                          error: pluginSelection.error,
                          onSearchChanged: (_) => _updateUi(() {}),
                          onLoadMore: pluginController.loadMore,
                          onRetry: () => pluginSelection.tracks.isEmpty
                              ? pluginController.select(
                                  selectedPluginPlaylist,
                                  reload: true,
                                )
                              : pluginController.loadMore(),
                          onActivate: (index, items) async {
                            final source = QueueSource(
                              type: QueueSourceType.catalog,
                              sourceInstanceId: selectedPluginPlaylist.sourceId,
                              mediaKind: 'playlist',
                              mediaId: selectedPluginPlaylist.playlistId,
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
                        pluginPlaylists: pluginState.entries,
                        selectedPluginPlaylistKey: selectedPluginPlaylist?.key,
                        onSelect: (id) {
                          pluginController.clearSelection();
                          ref
                              .read(libraryControllerProvider.notifier)
                              .selectPlaylist(id);
                        },
                        onSelectPlugin: pluginController.select,
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
                        onRefreshPlugins: pluginController.refresh,
                        pluginLoading: pluginState.refreshing,
                        pluginError: pluginState.listError,
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
