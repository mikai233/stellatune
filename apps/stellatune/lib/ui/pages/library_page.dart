import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/ui/widgets/folder_tree.dart';
import 'package:stellatune/ui/widgets/track_list.dart';

class LibraryPage extends ConsumerStatefulWidget {
  const LibraryPage({super.key});

  @override
  ConsumerState<LibraryPage> createState() => _LibraryPageState();
}

class _LibraryPageState extends ConsumerState<LibraryPage> {
  final _searchController = TextEditingController();
  bool _foldersPaneCollapsed = false;
  double _foldersPaneWidth = 280;
  bool _isResizingFoldersPane = false;
  bool _foldersEditMode = false;

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    const minFoldersWidth = 220.0;

    // Avoid rebuilding the whole page on unrelated state changes.
    final roots = ref.watch(libraryControllerProvider.select((s) => s.roots));
    final folders = ref.watch(
      libraryControllerProvider.select((s) => s.folders),
    );
    final selectedFolder = ref.watch(
      libraryControllerProvider.select((s) => s.selectedFolder),
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

    return Scaffold(
      appBar: AppBar(
        automaticallyImplyLeading: false,
        leading: IconButton(
          tooltip: _foldersPaneCollapsed ? l10n.expand : l10n.collapse,
          icon: Icon(
            _foldersPaneCollapsed ? Icons.chevron_right : Icons.chevron_left,
          ),
          onPressed: () => setState(() {
            _foldersPaneCollapsed = !_foldersPaneCollapsed;
            if (!_foldersPaneCollapsed && _foldersPaneWidth <= 0) {
              _foldersPaneWidth = minFoldersWidth;
            }
          }),
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
            onPressed: () =>
                ref.read(libraryControllerProvider.notifier).scanAll(),
            icon: const Icon(Icons.refresh),
          ),
          const SizedBox(width: 8),
        ],
      ),
      body: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
        child: LayoutBuilder(
          builder: (context, constraints) {
            const dividerWidthExpanded = 24.0;
            const minContentWidth = 360.0;
            final maxFoldersWidth =
                (constraints.maxWidth - dividerWidthExpanded - minContentWidth)
                    .clamp(0.0, 520.0)
                    .toDouble();

            final showFoldersPane =
                !_foldersPaneCollapsed && maxFoldersWidth > 0.0;
            final foldersWidth = showFoldersPane
                ? _foldersPaneWidth.clamp(0.0, maxFoldersWidth).toDouble()
                : 0.0;
            final dividerWidth = showFoldersPane ? dividerWidthExpanded : 0.0;

            return Row(
              children: [
                AnimatedContainer(
                  width: foldersWidth,
                  duration: _isResizingFoldersPane
                      ? Duration.zero
                      : const Duration(milliseconds: 180),
                  curve: Curves.easeInOut,
                  child: showFoldersPane
                      ? ClipRect(
                          child: Align(
                            alignment: Alignment.centerLeft,
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                SizedBox(
                                  height: 44,
                                  child: LayoutBuilder(
                                    builder: (context, constraints) {
                                      const buttonExtent = 40.0;
                                      final showButton =
                                          constraints.maxWidth >= buttonExtent;

                                      return Row(
                                        children: [
                                          const Spacer(),
                                          if (showButton)
                                            IconButton(
                                              constraints:
                                                  const BoxConstraints.tightFor(
                                                    width: buttonExtent,
                                                    height: buttonExtent,
                                                  ),
                                              padding: EdgeInsets.zero,
                                              visualDensity:
                                                  VisualDensity.compact,
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
                                      );
                                    },
                                  ),
                                ),
                                const SizedBox(height: 8),
                                Expanded(
                                  child: FolderTree(
                                    roots: roots,
                                    folders: folders,
                                    excludedFolders: excludedFolders,
                                    selectedFolder: selectedFolder,
                                    isEditing: _foldersEditMode,
                                    onDeleteFolder: (p) => ref
                                        .read(
                                          libraryControllerProvider.notifier,
                                        )
                                        .deleteFolder(p),
                                    onRestoreFolder: (p) => ref
                                        .read(
                                          libraryControllerProvider.notifier,
                                        )
                                        .restoreFolder(p),
                                    onSelectAll: () => ref
                                        .read(
                                          libraryControllerProvider.notifier,
                                        )
                                        .selectAllMusic(),
                                    onSelectFolder: (p) => ref
                                        .read(
                                          libraryControllerProvider.notifier,
                                        )
                                        .selectFolder(p),
                                  ),
                                ),
                              ],
                            ),
                          ),
                        )
                      : const SizedBox.shrink(),
                ),
                AnimatedContainer(
                  width: dividerWidth,
                  duration: _isResizingFoldersPane
                      ? Duration.zero
                      : const Duration(milliseconds: 180),
                  curve: Curves.easeInOut,
                  child: MouseRegion(
                    cursor: SystemMouseCursors.resizeColumn,
                    child: GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onHorizontalDragStart: (_) => setState(() {
                        _isResizingFoldersPane = true;
                        _foldersPaneCollapsed = false;
                        if (_foldersPaneWidth <= 0) {
                          _foldersPaneWidth = minFoldersWidth;
                        }
                      }),
                      onHorizontalDragUpdate: (details) => setState(() {
                        _foldersPaneCollapsed = false;
                        _foldersPaneWidth =
                            (_foldersPaneWidth + details.delta.dx)
                                .clamp(0.0, maxFoldersWidth)
                                .toDouble();
                        if (_foldersPaneWidth < minFoldersWidth) {
                          _foldersPaneWidth = minFoldersWidth;
                        }
                      }),
                      onHorizontalDragEnd: (_) => setState(() {
                        _isResizingFoldersPane = false;
                        _foldersPaneWidth = _foldersPaneWidth.clamp(
                          0.0,
                          maxFoldersWidth,
                        );
                      }),
                      child: dividerWidth > 0
                          ? const VerticalDivider(width: dividerWidthExpanded)
                          : const SizedBox.shrink(),
                    ),
                  ),
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
                          border: const OutlineInputBorder(),
                        ),
                        onChanged: (q) => ref
                            .read(libraryControllerProvider.notifier)
                            .setQuery(q),
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
                      if (selectedFolder.isNotEmpty) const SizedBox(height: 12),
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
                          onActivate: (index, items) async {
                            final paths = items.map((t) => t.path).toList();
                            await ref
                                .read(playbackControllerProvider.notifier)
                                .setQueueAndPlay(paths, startIndex: index);
                          },
                          onEnqueue: (track) async {
                            await ref
                                .read(playbackControllerProvider.notifier)
                                .enqueue([track.path]);
                          },
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
    );
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
