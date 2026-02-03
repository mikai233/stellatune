import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/library/library_controller.dart';
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
    final theme = Theme.of(context);
    final library = ref.watch(libraryControllerProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Library'),
        actions: [
          IconButton(
            tooltip: 'Add folder',
            onPressed: () => _pickAndAddFolder(context),
            icon: const Icon(Icons.create_new_folder_outlined),
          ),
          IconButton(
            tooltip: 'Scan',
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
              roots: library.roots,
              onRemove: (p) =>
                  ref.read(libraryControllerProvider.notifier).removeRoot(p),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _searchController,
              decoration: const InputDecoration(
                prefixIcon: Icon(Icons.search),
                hintText: 'Search title / artist / album / path',
                border: OutlineInputBorder(),
              ),
              onChanged: (q) =>
                  ref.read(libraryControllerProvider.notifier).setQuery(q),
            ),
            const SizedBox(height: 12),
            if (library.isScanning || library.lastFinishedMs != null)
              _ScanStatusCard(
                isScanning: library.isScanning,
                scanned: library.progress.scanned,
                updated: library.progress.updated,
                skipped: library.progress.skipped,
                errors: library.progress.errors,
                durationMs: library.lastFinishedMs,
              ),
            if (library.lastError != null)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  library.lastError!,
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: theme.colorScheme.error,
                  ),
                ),
              ),
            const SizedBox(height: 12),
            Expanded(
              child: TrackList(
                items: library.results,
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
    final dir = await FilePicker.platform.getDirectoryPath(
      dialogTitle: 'Select music folder',
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
    if (roots.isEmpty) {
      return const Text(
        'No folders yet. Click “Add folder” to start scanning.',
      );
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
    final title = isScanning ? 'Scanning…' : 'Scan finished';
    final subtitle = durationMs == null ? null : '${durationMs}ms';

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
            _Stat(label: 'scanned', value: scanned),
            _Stat(label: 'updated', value: updated),
            _Stat(label: 'skipped', value: skipped),
            _Stat(label: 'errors', value: errors),
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
