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
import 'package:stellatune/ui/widgets/track_list.dart';

class LibraryPage extends ConsumerStatefulWidget {
  const LibraryPage({super.key, this.useGlobalTopBar = false});

  final bool useGlobalTopBar;

  @override
  ConsumerState<LibraryPage> createState() => LibraryPageState();
}

class LibraryPageState extends ConsumerState<LibraryPage> {
  static const double _minFoldersPaneWidth = 220.0;
  static const int _playabilityProbeMargin = 40;
  static const int _playabilityCacheMaxEntries = 12000;

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
  int _playabilityRequestSeq = 0;
  int _viewportStart = 0;
  int _viewportEnd = -1;
  String _resultsKey = '';
  final Map<String, String?> _playabilityCache = <String, String?>{};
  Map<int, String> _blockedReasonByTrackId = const <int, String>{};

  bool get foldersPaneCollapsed => _foldersPaneCollapsed;

  void toggleFoldersPane() {
    setState(() {
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

  String _trackCacheKey(TrackLite t) => '${t.id}|${t.path}';

  String _buildResultsKey(List<TrackLite> items) {
    if (items.isEmpty) return '';
    final buf = StringBuffer();
    for (final t in items) {
      buf
        ..write(t.id)
        ..write('|')
        ..write(t.path)
        ..write(';');
    }
    return buf.toString();
  }

  void _evictPlayabilityCacheIfNeeded() {
    while (_playabilityCache.length > _playabilityCacheMaxEntries) {
      if (_playabilityCache.isEmpty) return;
      _playabilityCache.remove(_playabilityCache.keys.first);
    }
  }

  bool _sameBlockedReasonMap(Map<int, String> next) {
    final current = _blockedReasonByTrackId;
    if (identical(current, next)) return true;
    if (current.length != next.length) return false;
    for (final entry in current.entries) {
      if (next[entry.key] != entry.value) {
        return false;
      }
    }
    return true;
  }

  void _rebuildBlockedReasonByTrackId(List<TrackLite> items) {
    final l10n = AppLocalizations.of(context);
    if (l10n == null) return;
    final blocked = <int, String>{};
    for (final t in items) {
      final reason = _playabilityCache[_trackCacheKey(t)];
      if (reason == null) continue;
      blocked[t.id.toInt()] = localizePlayabilityReason(l10n, reason);
    }
    if (_sameBlockedReasonMap(blocked)) return;
    setState(() => _blockedReasonByTrackId = blocked);
  }

  void _onViewportRangeChanged(int startIndex, int endIndex) {
    if (_viewportStart == startIndex && _viewportEnd == endIndex) {
      return;
    }
    _viewportStart = startIndex;
    _viewportEnd = endIndex;
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
    final key = _buildResultsKey(items);
    if (_resultsKey != key) {
      _resultsKey = key;
      _viewportStart = 0;
      _viewportEnd = -1;
    }

    if (items.isEmpty) {
      if (!mounted) return;
      setState(() => _blockedReasonByTrackId = const <int, String>{});
      return;
    }

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _rebuildBlockedReasonByTrackId(items);
    });

    final maxIndex = items.length - 1;
    final initialEnd = (items.length - 1).clamp(0, 19).toInt();
    var probeStart = _viewportEnd >= 0 ? _viewportStart : 0;
    var probeEnd = _viewportEnd >= 0 ? _viewportEnd : initialEnd;
    probeStart = (probeStart - _playabilityProbeMargin)
        .clamp(0, maxIndex)
        .toInt();
    probeEnd = (probeEnd + _playabilityProbeMargin).clamp(0, maxIndex).toInt();
    if (probeEnd < probeStart) {
      probeEnd = probeStart;
    }

    final pending = <(String, String)>[];
    for (var i = probeStart; i <= probeEnd; i++) {
      final t = items[i];
      final cacheKey = _trackCacheKey(t);
      if (!force && _playabilityCache.containsKey(cacheKey)) {
        continue;
      }
      pending.add((cacheKey, t.path));
    }
    if (pending.isEmpty) {
      return;
    }

    final requestSeq = ++_playabilityRequestSeq;
    await _refreshDecoderExtensionSupport();
    final snapshot = DecoderExtensionSupportCache.instance.snapshotOrNull;
    if (snapshot == null) {
      return;
    }
    if (!mounted || requestSeq != _playabilityRequestSeq) return;

    for (final item in pending) {
      _playabilityCache[item.$1] = snapshot.canPlayLocalPath(item.$2)
          ? null
          : 'no_decoder_for_local_track';
    }
    _evictPlayabilityCacheIfNeeded();
    _rebuildBlockedReasonByTrackId(items);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    const minFoldersWidth = _minFoldersPaneWidth;

    // Avoid rebuilding the whole page on unrelated state changes.
    final roots = ref.watch(libraryControllerProvider.select((s) => s.roots));
    final folders = ref.watch(
      libraryControllerProvider.select((s) => s.folders),
    );
    final selectedFolder = ref.watch(
      libraryControllerProvider.select((s) => s.selectedFolder),
    );
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
    final queueSourceLabel = (queueSourceSnapshot ?? '').trim().isEmpty
        ? l10n.queueSourceUnset
        : queueSourceSnapshot!.trim();

    final appBar = AppBar(
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

    final pageBody = Stack(
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
                                          onPressed: () => setState(() {
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
                                setState(() => _foldersPaneCollapsed = false);
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
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        TextField(
                          controller: _searchController,
                          decoration: InputDecoration(
                            prefixIcon: const Icon(Icons.search),
                            hintText: l10n.searchHint,
                            filled: true,
                            fillColor: theme.colorScheme.surfaceContainerLowest
                                .withValues(alpha: 0.72),
                            border: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(14),
                              borderSide: BorderSide(
                                color: theme.colorScheme.onSurface.withValues(
                                  alpha: 0.10,
                                ),
                              ),
                            ),
                            enabledBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(14),
                              borderSide: BorderSide(
                                color: theme.colorScheme.onSurface.withValues(
                                  alpha: 0.10,
                                ),
                              ),
                            ),
                            focusedBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(14),
                              borderSide: BorderSide(
                                color: theme.colorScheme.primary,
                              ),
                            ),
                          ),
                          onChanged: (q) => ref
                              .read(libraryControllerProvider.notifier)
                              .setQuery(q),
                        ),
                        const SizedBox(height: 12),
                        Container(
                          width: double.infinity,
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 10,
                          ),
                          decoration: BoxDecoration(
                            gradient: LinearGradient(
                              begin: Alignment.topLeft,
                              end: Alignment.bottomRight,
                              colors: [
                                theme.colorScheme.surfaceContainerHigh
                                    .withValues(alpha: 0.74),
                                theme.colorScheme.surfaceContainer.withValues(
                                  alpha: 0.58,
                                ),
                              ],
                            ),
                            border: Border.all(
                              color: theme.colorScheme.onSurface.withValues(
                                alpha: 0.08,
                              ),
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
                                      style: theme.textTheme.bodySmall
                                          ?.copyWith(
                                            color: theme
                                                .colorScheme
                                                .onSurfaceVariant,
                                          ),
                                    ),
                                  ],
                                ),
                              ),
                            ],
                          ),
                        ),
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
                                      onChanged: (_) => ref
                                          .read(
                                            libraryControllerProvider.notifier,
                                          )
                                          .toggleIncludeSubfolders(),
                                    ),
                                  ],
                                ),
                              ],
                            ],
                          ),
                        if (selectedFolder.isNotEmpty)
                          const SizedBox(height: 12),
                        if (isScanning || lastFinishedMs != null)
                          _ScanStatusCard(
                            isScanning: isScanning,
                            scanned: progress.scanned,
                            updated: progress.updated,
                            skipped: progress.skipped,
                            errors: progress.errors,
                            durationMs: lastFinishedMs,
                          ),
                        if (lastError != null)
                          Padding(
                            padding: const EdgeInsets.only(top: 8),
                            child: Text(
                              lastError,
                              style: theme.textTheme.bodyMedium?.copyWith(
                                color: theme.colorScheme.error,
                              ),
                            ),
                          ),
                        const SizedBox(height: 12),
                        Expanded(
                          child: TrackList(
                            coverDir: ref.watch(coverDirProvider),
                            items: results,
                            likedTrackIds: likedTrackIds,
                            playlists: playlists,
                            currentPlaylistId: null,
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

    if (widget.useGlobalTopBar) {
      return pageBody;
    }

    return Scaffold(appBar: appBar, body: pageBody);
  }

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

class _ScanStatusCard extends StatelessWidget {
  const _ScanStatusCard({
    required this.isScanning,
    required this.scanned,
    required this.updated,
    required this.skipped,
    required this.errors,
    required this.durationMs,
  });

  final bool isScanning;
  final int scanned;
  final int updated;
  final int skipped;
  final int errors;
  final int? durationMs;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final title = isScanning
        ? l10n.scanStatusScanning
        : l10n.scanStatusFinished;
    final subtitle = durationMs == null
        ? null
        : l10n.scanDurationMs(durationMs!);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Row(
          children: [
            if (isScanning)
              const SizedBox(
                width: 18,
                height: 18,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            else
              const Icon(Icons.check_circle_outline),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: Theme.of(context).textTheme.titleMedium),
                  if (subtitle != null)
                    Text(
                      subtitle,
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                ],
              ),
            ),
            _Stat(label: l10n.scanLabelScanned, value: scanned),
            _Stat(label: l10n.scanLabelUpdated, value: updated),
            _Stat(label: l10n.scanLabelSkipped, value: skipped),
            _Stat(label: l10n.scanLabelErrors, value: errors),
          ],
        ),
      ),
    );
  }
}

class _Stat extends StatelessWidget {
  const _Stat({required this.label, required this.value});

  final String label;
  final int value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(left: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(
            value.toString(),
            style: Theme.of(context).textTheme.titleMedium,
          ),
          Text(label, style: Theme.of(context).textTheme.bodySmall),
        ],
      ),
    );
  }
}
