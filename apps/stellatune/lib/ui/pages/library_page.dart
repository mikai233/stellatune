import 'dart:async';

import 'package:file_picker/file_picker.dart';
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
import 'package:stellatune/ui/widgets/folder_tree.dart';
import 'package:stellatune/player/track_playability_utils.dart';
import 'package:stellatune/ui/pages/library/widgets/library_tracks_content.dart';

class LibraryPage extends ConsumerStatefulWidget {
  const LibraryPage({super.key, this.useGlobalTopBar = false});

  final bool useGlobalTopBar;

  @override
  ConsumerState<LibraryPage> createState() => LibraryPageState();
}

class LibraryPageState extends ConsumerState<LibraryPage> {
  static const double _minFoldersPaneWidth = 220.0;

  final _searchController = TextEditingController();
  bool _foldersPaneCollapsed = false;
  final ValueNotifier<double> _foldersPaneWidth = ValueNotifier(
    _minFoldersPaneWidth,
  );
  final ValueNotifier<bool> _isResizingFoldersPane = ValueNotifier(false);
  bool _foldersEditMode = false;

  bool _dividerHovering = false;
  bool _dividerRearmPending = false;
  double _dividerDragLastX = 0.0;
  final TrackPlayabilityProbe _playabilityProbe = TrackPlayabilityProbe();
  Map<int, String> _blockedReasonByTrackId = const <int, String>{};

  void _updateUi(VoidCallback updater) => setState(updater);

  void _syncSearchController(String query) {
    if (_searchController.text == query) return;
    _searchController.value = TextEditingValue(
      text: query,
      selection: TextSelection.collapsed(offset: query.length),
    );
  }

  bool get foldersPaneCollapsed => _foldersPaneCollapsed;

  void toggleFoldersPane() {
    _updateUi(() {
      _foldersPaneCollapsed = !_foldersPaneCollapsed;
      if (!_foldersPaneCollapsed && _foldersPaneWidth.value <= 0) {
        _foldersPaneWidth.value = _minFoldersPaneWidth;
      }
    });
  }

  Future<void> addFolderFromTopBar() => _pickAndAddFolder(context);

  Future<void> scanFromTopBar({bool force = false}) async {
    await ref.read(libraryControllerProvider.notifier).scanAll(force: force);
  }

  @override
  void initState() {
    super.initState();
    unawaited(_refreshDecoderExtensionSupport());
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final selectedPlaylistId = ref
          .read(libraryControllerProvider)
          .selectedPlaylistId;
      if (selectedPlaylistId != null) {
        ref.read(libraryControllerProvider.notifier).selectAllMusic();
      }
    });
  }

  @override
  void dispose() {
    _searchController.dispose();
    _foldersPaneWidth.dispose();
    _isResizingFoldersPane.dispose();
    super.dispose();
  }

  void _applyBlockedReasonByTrackId(Map<int, String> blocked) {
    if (hasSameTrackBlockedReasons(_blockedReasonByTrackId, blocked)) return;
    _updateUi(() => _blockedReasonByTrackId = blocked);
  }

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
  Widget build(BuildContext context) => _buildLibraryPage(context);

  Future<void> _pickAndAddFolder(BuildContext context) async {
    final l10n = AppLocalizations.of(context)!;
    final dir = await FilePicker.platform.getDirectoryPath(
      dialogTitle: l10n.dialogSelectMusicFolder,
    );
    if (dir == null || dir.trim().isEmpty) return;
    await ref
        .read(libraryControllerProvider.notifier)
        .addRoot(dir, scanAfter: true);
  }
}

extension _LibraryLayout on LibraryPageState {
  Widget _buildLibraryPage(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);

    // Avoid rebuilding the whole page on unrelated state changes.
    final roots = ref.watch(libraryControllerProvider.select((s) => s.roots));
    final folders = ref.watch(
      libraryControllerProvider.select((s) => s.folders),
    );
    final selectedFolder = ref.watch(
      libraryControllerProvider.select((s) => s.selectedFolder),
    );
    final query = ref.watch(libraryControllerProvider.select((s) => s.query));
    _syncSearchController(query);
    final playlists = ref.watch(
      libraryControllerProvider.select((s) => s.playlists),
    );
    final likedTrackIds = ref.watch(
      libraryControllerProvider.select((s) => s.likedTrackIds),
    );
    final queueSourceSnapshot = ref.watch(
      queueControllerProvider.select((s) => s.sourceLabel),
    );
    final excludedFolders = ref.watch(
      libraryControllerProvider.select((s) => s.excludedFolders),
    );
    final includeSubfolders = ref.watch(
      libraryControllerProvider.select((s) => s.includeSubfolders),
    );
    final hasSubfolders =
        selectedFolder.isNotEmpty &&
        folders.any((f) => f.startsWith('$selectedFolder/'));
    final results = ref.watch(
      libraryControllerProvider.select((s) => s.results),
    );
    unawaited(_refreshTrackPlayability(results));
    final isScanning = ref.watch(
      libraryControllerProvider.select((s) => s.isScanning),
    );
    final progress = ref.watch(
      libraryControllerProvider.select((s) => s.progress),
    );
    final lastFinishedMs = ref.watch(
      libraryControllerProvider.select((s) => s.lastFinishedMs),
    );
    final lastError = ref.watch(
      libraryControllerProvider.select((s) => s.lastError),
    );

    final selectionSourceLabel = selectedFolder.isEmpty
        ? l10n.libraryAllMusic
        : includeSubfolders
        ? '$selectedFolder • ${l10n.includeSubfolders}'
        : selectedFolder;
    final queueSourceLabelRaw = (queueSourceSnapshot ?? '').trim();
    final queueSourceLabel = queueSourceLabelRaw.isEmpty
        ? l10n.queueSourceUnset
        : queueSourceLabelRaw;

    final appBar = _buildLibraryAppBar(l10n: l10n, isScanning: isScanning);
    final pageBody = _buildLibraryBody(
      l10n: l10n,
      theme: theme,
      roots: roots,
      folders: folders,
      excludedFolders: excludedFolders,
      selectedFolder: selectedFolder,
      includeSubfolders: includeSubfolders,
      hasSubfolders: hasSubfolders,
      queueSourceLabel: queueSourceLabel,
      results: results,
      playlists: playlists,
      likedTrackIds: likedTrackIds,
      selectionSourceLabel: selectionSourceLabel,
      isScanning: isScanning,
      scanned: progress.scanned,
      updated: progress.updated,
      skipped: progress.skipped,
      errors: progress.errors,
      lastFinishedMs: lastFinishedMs,
      lastError: lastError,
    );

    if (widget.useGlobalTopBar) {
      return pageBody;
    }
    return Scaffold(appBar: appBar, body: pageBody);
  }

  AppBar _buildLibraryAppBar({
    required AppLocalizations l10n,
    required bool isScanning,
  }) {
    return AppBar(
      automaticallyImplyLeading: false,
      leading: IconButton(
        tooltip: _foldersPaneCollapsed ? l10n.expand : l10n.collapse,
        icon: Icon(
          _foldersPaneCollapsed ? Icons.chevron_right : Icons.chevron_left,
        ),
        onPressed: toggleFoldersPane,
      ),
      title: Text(l10n.libraryTitle),
      actions: [
        IconButton(
          tooltip: l10n.tooltipAddFolder,
          onPressed: () => _pickAndAddFolder(context),
          icon: const Icon(Icons.create_new_folder_outlined),
        ),
        IconButton(
          tooltip: l10n.tooltipScan,
          onPressed: isScanning
              ? null
              : () => ref.read(libraryControllerProvider.notifier).scanAll(),
          icon: const Icon(Icons.refresh),
        ),
        IconButton(
          tooltip: l10n.tooltipForceScan,
          onPressed: isScanning
              ? null
              : () => ref
                    .read(libraryControllerProvider.notifier)
                    .scanAll(force: true),
          icon: const Icon(Icons.restart_alt),
        ),
        const SizedBox(width: 8),
      ],
    );
  }

  Widget _buildLibraryBody({
    required AppLocalizations l10n,
    required ThemeData theme,
    required List<String> roots,
    required List<String> folders,
    required List<String> excludedFolders,
    required String selectedFolder,
    required bool includeSubfolders,
    required bool hasSubfolders,
    required String queueSourceLabel,
    required List<TrackLite> results,
    required List<PlaylistLite> playlists,
    required Set<int> likedTrackIds,
    required String selectionSourceLabel,
    required bool isScanning,
    required int scanned,
    required int updated,
    required int skipped,
    required int errors,
    required int? lastFinishedMs,
    required String? lastError,
  }) {
    const minFoldersWidth = LibraryPageState._minFoldersPaneWidth;

    return Stack(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
          child: LayoutBuilder(
            builder: (context, constraints) {
              const dividerWidthExpanded = 24.0;
              const minContentWidth = 360.0;
              final maxFoldersWidth =
                  (constraints.maxWidth -
                          dividerWidthExpanded -
                          minContentWidth)
                      .clamp(0.0, 520.0)
                      .toDouble();
              final effectiveMinFoldersWidth = maxFoldersWidth <= 0
                  ? 0.0
                  : minFoldersWidth.clamp(0.0, maxFoldersWidth).toDouble();

              final canShowFoldersPane = maxFoldersWidth >= minFoldersWidth;
              final showFoldersPane =
                  !_foldersPaneCollapsed && canShowFoldersPane;

              return Row(
                children: [
                  AnimatedBuilder(
                    animation: Listenable.merge(<Listenable>[
                      _foldersPaneWidth,
                      _isResizingFoldersPane,
                    ]),
                    child: canShowFoldersPane
                        ? ClipRect(
                            child: Align(
                              alignment: Alignment.centerLeft,
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  SizedBox(
                                    height: 44,
                                    child: Row(
                                      children: [
                                        const Spacer(),
                                        IconButton(
                                          visualDensity: VisualDensity.compact,
                                          icon: Icon(
                                            _foldersEditMode
                                                ? Icons.check
                                                : Icons.edit_outlined,
                                          ),
                                          onPressed: () => _updateUi(() {
                                            _foldersEditMode =
                                                !_foldersEditMode;
                                          }),
                                        ),
                                      ],
                                    ),
                                  ),
                                  Text(
                                    l10n.foldersSectionTitle,
                                    style: theme.textTheme.titleSmall,
                                  ),
                                  const SizedBox(height: 8),
                                  Expanded(
                                    flex: 5,
                                    child: DecoratedBox(
                                      decoration: BoxDecoration(
                                        color: theme
                                            .colorScheme
                                            .surfaceContainerLowest
                                            .withValues(alpha: 0.62),
                                        borderRadius: BorderRadius.circular(14),
                                        border: Border.all(
                                          color: theme.colorScheme.onSurface
                                              .withValues(alpha: 0.08),
                                        ),
                                      ),
                                      child: Padding(
                                        padding: const EdgeInsets.symmetric(
                                          horizontal: 6,
                                          vertical: 6,
                                        ),
                                        child: FolderTree(
                                          roots: roots,
                                          folders: folders,
                                          excludedFolders: excludedFolders,
                                          selectedFolder: selectedFolder,
                                          isEditing: _foldersEditMode,
                                          onDeleteFolder: (p) => ref
                                              .read(
                                                libraryControllerProvider
                                                    .notifier,
                                              )
                                              .deleteFolder(p),
                                          onRestoreFolder: (p) => ref
                                              .read(
                                                libraryControllerProvider
                                                    .notifier,
                                              )
                                              .restoreFolder(p),
                                          onSelectAll: () => ref
                                              .read(
                                                libraryControllerProvider
                                                    .notifier,
                                              )
                                              .selectAllMusic(),
                                          onSelectFolder: (p) => ref
                                              .read(
                                                libraryControllerProvider
                                                    .notifier,
                                              )
                                              .selectFolder(p),
                                        ),
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          )
                        : const SizedBox.shrink(),
                    builder: (context, foldersPaneChild) {
                      final paneWidth = canShowFoldersPane
                          ? _foldersPaneWidth.value
                                .clamp(
                                  effectiveMinFoldersWidth,
                                  maxFoldersWidth,
                                )
                                .toDouble()
                          : 0.0;
                      final visibleWidth = showFoldersPane ? paneWidth : 0.0;
                      final animDuration = _isResizingFoldersPane.value
                          ? Duration.zero
                          : const Duration(milliseconds: 180);

                      return AnimatedContainer(
                        width: visibleWidth,
                        duration: animDuration,
                        curve: Curves.easeInOut,
                        child: ClipRect(
                          child: Align(
                            alignment: Alignment.centerLeft,
                            child: SizedBox(
                              width: paneWidth,
                              child: foldersPaneChild,
                            ),
                          ),
                        ),
                      );
                    },
                  ),
                  AnimatedBuilder(
                    animation: _isResizingFoldersPane,
                    builder: (context, _) {
                      final dividerWidth = showFoldersPane
                          ? dividerWidthExpanded
                          : 0.0;
                      final animDuration = _isResizingFoldersPane.value
                          ? Duration.zero
                          : const Duration(milliseconds: 180);
                      return AnimatedContainer(
                        width: dividerWidth,
                        duration: animDuration,
                        curve: Curves.easeInOut,
                        child: MouseRegion(
                          cursor: SystemMouseCursors.resizeColumn,
                          onEnter: (e) {
                            _dividerHovering = true;
                            if (_isResizingFoldersPane.value &&
                                _dividerRearmPending) {
                              _dividerDragLastX = e.position.dx;
                              _dividerRearmPending = false;
                            }
                          },
                          onExit: (_) => _dividerHovering = false,
                          child: GestureDetector(
                            behavior: HitTestBehavior.opaque,
                            onHorizontalDragStart: (details) {
                              _isResizingFoldersPane.value = true;
                              _dividerHovering = true;
                              _dividerRearmPending = false;
                              _dividerDragLastX = details.globalPosition.dx;
                              if (_foldersPaneCollapsed) {
                                _updateUi(() => _foldersPaneCollapsed = false);
                              }
                              if (_foldersPaneWidth.value <= 0) {
                                _foldersPaneWidth.value =
                                    effectiveMinFoldersWidth;
                              }
                            },
                            onHorizontalDragUpdate: (details) {
                              final x = details.globalPosition.dx;
                              if (_dividerDragLastX == 0.0 ||
                                  _dividerRearmPending) {
                                _dividerDragLastX = x;
                                _dividerRearmPending = false;
                                return;
                              }

                              final dx = x - _dividerDragLastX;
                              _dividerDragLastX = x;

                              const eps = 0.5;
                              final w = _foldersPaneWidth.value;
                              final atMin =
                                  (w - effectiveMinFoldersWidth).abs() <= eps ||
                                  w <= effectiveMinFoldersWidth + eps;
                              final atMax =
                                  (w - maxFoldersWidth).abs() <= eps ||
                                  w >= maxFoldersWidth - eps;
                              final atEdge = atMin || atMax;

                              if (!_dividerHovering && atEdge) {
                                _dividerRearmPending = true;
                                return;
                              }

                              final next = (_foldersPaneWidth.value + dx)
                                  .clamp(0.0, maxFoldersWidth)
                                  .toDouble();
                              final desired = next < effectiveMinFoldersWidth
                                  ? effectiveMinFoldersWidth
                                  : next;
                              if (_foldersPaneWidth.value != desired) {
                                _foldersPaneWidth.value = desired;
                              }
                            },
                            onHorizontalDragEnd: (_) {
                              _isResizingFoldersPane.value = false;
                              _dividerHovering = false;
                              _dividerRearmPending = false;
                              _dividerDragLastX = 0.0;
                              _foldersPaneWidth.value = _foldersPaneWidth.value
                                  .clamp(0.0, maxFoldersWidth)
                                  .toDouble();
                            },
                            child: const VerticalDivider(
                              width: dividerWidthExpanded,
                            ),
                          ),
                        ),
                      );
                    },
                  ),
                  Expanded(
                    child: LibraryTracksContent(
                      l10n: l10n,
                      searchController: _searchController,
                      onSearchChanged: (q) => ref
                          .read(libraryControllerProvider.notifier)
                          .setQuery(q),
                      queueSourceLabel: queueSourceLabel,
                      selectedFolder: selectedFolder,
                      hasSubfolders: hasSubfolders,
                      includeSubfolders: includeSubfolders,
                      onToggleIncludeSubfolders: () => ref
                          .read(libraryControllerProvider.notifier)
                          .toggleIncludeSubfolders(),
                      isScanning: isScanning,
                      scanned: scanned,
                      updated: updated,
                      skipped: skipped,
                      errors: errors,
                      lastFinishedMs: lastFinishedMs,
                      lastError: lastError,
                      coverDir: ref.watch(coverDirProvider),
                      results: results,
                      likedTrackIds: likedTrackIds,
                      playlists: playlists,
                      selectionSourceLabel: selectionSourceLabel,
                      onActivate: (index, items) async {
                        final source = QueueSource(
                          type: selectedFolder.isEmpty
                              ? QueueSourceType.all
                              : QueueSourceType.folder,
                          folderPath: selectedFolder,
                          includeSubfolders: includeSubfolders,
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
                      blockedReasonByTrackId: _blockedReasonByTrackId,
                      onViewportRangeChanged: _onViewportRangeChanged,
                    ),
                  ),
                ],
              );
            },
          ),
        ),
        ValueListenableBuilder<bool>(
          valueListenable: _isResizingFoldersPane,
          builder: (context, resizing, _) {
            if (!resizing) return const SizedBox.shrink();
            return Positioned.fill(
              child: MouseRegion(
                cursor: SystemMouseCursors.resizeColumn,
                opaque: false,
                child: const SizedBox.expand(),
              ),
            );
          },
        ),
      ],
    );
  }
}
