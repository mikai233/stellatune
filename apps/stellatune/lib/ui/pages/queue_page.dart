import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

class QueuePage extends ConsumerWidget {
  const QueuePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final queue = ref.watch(queueControllerProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.queueTitle)),
      body: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
        child: Column(
          children: [
            _QueueModeRow(
              shuffle: queue.shuffle,
              repeatMode: queue.repeatMode,
              onToggleShuffle: () =>
                  ref.read(queueControllerProvider.notifier).toggleShuffle(),
              onCycleRepeat: () =>
                  ref.read(queueControllerProvider.notifier).cycleRepeatMode(),
            ),
            const SizedBox(height: 12),
            Expanded(
              child: _QueueList(
                items: queue.items,
                currentIndex: queue.currentIndex,
                onActivate: (i) =>
                    ref.read(playbackControllerProvider.notifier).playIndex(i),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _QueueModeRow extends StatelessWidget {
  const _QueueModeRow({
    required this.shuffle,
    required this.repeatMode,
    required this.onToggleShuffle,
    required this.onCycleRepeat,
  });

  final bool shuffle;
  final RepeatMode repeatMode;
  final VoidCallback onToggleShuffle;
  final VoidCallback onCycleRepeat;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final repeatLabel = switch (repeatMode) {
      RepeatMode.off => l10n.repeatOff,
      RepeatMode.all => l10n.repeatAll,
      RepeatMode.one => l10n.repeatOne,
    };

    return Row(
      children: [
        FilterChip(
          selected: shuffle,
          onSelected: (_) => onToggleShuffle(),
          label: Text(l10n.queueShuffle),
        ),
        const SizedBox(width: 8),
        ActionChip(onPressed: onCycleRepeat, label: Text(repeatLabel)),
      ],
    );
  }
}

class _QueueList extends StatelessWidget {
  const _QueueList({
    required this.items,
    required this.currentIndex,
    required this.onActivate,
  });

  final List<QueueItem> items;
  final int? currentIndex;
  final void Function(int index) onActivate;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (items.isEmpty) return Center(child: Text(l10n.queueEmpty));

    return ListView.separated(
      itemCount: items.length,
      separatorBuilder: (context, index) => const Divider(height: 1),
      itemBuilder: (context, i) {
        final item = items[i];
        final selected = currentIndex == i;
        return ListTile(
          selected: selected,
          leading: selected
              ? const Icon(Icons.play_arrow)
              : const SizedBox(width: 24),
          title: Text(
            item.displayTitle,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          subtitle: Text(
            item.path,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          onTap: () => onActivate(i),
        );
      },
    );
  }
}
