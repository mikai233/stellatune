import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/ui/widgets/track_list.dart';

class LibraryPage extends ConsumerStatefulWidget {
  const LibraryPage({super.key});

  @override
  ConsumerState<LibraryPage> createState() => _LibraryPageState();
}

class _LibraryPageState extends ConsumerState<LibraryPage> {
  final _searchController = TextEditingController();

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);

    // Avoid rebuilding the whole page on unrelated state changes.
    final roots = ref.watch(libraryControllerProvider.select((s) => s.roots));
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
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _RootsRow(
              roots: roots,
              onRemove: (p) =>
                  ref.read(libraryControllerProvider.notifier).removeRoot(p),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _searchController,
              decoration: InputDecoration(
                prefixIcon: const Icon(Icons.search),
                hintText: l10n.searchHint,
                border: const OutlineInputBorder(),
              ),
              onChanged: (q) =>
                  ref.read(libraryControllerProvider.notifier).setQuery(q),
            ),
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
                onActivate: (index, items) async {
                  final paths = items.map((t) => t.path).toList();
                  await ref
                      .read(playbackControllerProvider.notifier)
                      .setQueueAndPlay(paths, startIndex: index);
                },
                onEnqueue: (track) async {
                  await ref.read(playbackControllerProvider.notifier).enqueue([
                    track.path,
                  ]);
                },
              ),
            ),
          ],
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

class _RootsRow extends StatelessWidget {
  const _RootsRow({required this.roots, required this.onRemove});

  final List<String> roots;
  final void Function(String path) onRemove;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (roots.isEmpty) {
      return Text(l10n.noFoldersHint);
    }

    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        for (final r in roots)
          InputChip(
            label: Text(r, overflow: TextOverflow.ellipsis),
            onDeleted: () => onRemove(r),
          ),
      ],
    );
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
