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
            _QueueModeButton(
              mode: queue.playMode,
              onCycle: () =>
                  ref.read(queueControllerProvider.notifier).cyclePlayMode(),
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

class _QueueModeButton extends StatelessWidget {
  const _QueueModeButton({required this.mode, required this.onCycle});

  final PlayMode mode;
  final VoidCallback onCycle;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    final label = switch (mode) {
      PlayMode.sequential => l10n.playModeSequential,
      PlayMode.shuffle => l10n.playModeShuffle,
      PlayMode.repeatAll => l10n.playModeRepeatAll,
      PlayMode.repeatOne => l10n.playModeRepeatOne,
    };
    final icon = switch (mode) {
      PlayMode.sequential => Icons.playlist_play,
      PlayMode.shuffle => Icons.shuffle,
      PlayMode.repeatAll => Icons.repeat,
      PlayMode.repeatOne => Icons.repeat_one,
    };

    return Tooltip(
      message: label,
      child: ActionChip(
        avatar: Icon(icon, size: 18),
        onPressed: onCycle,
        label: Text(label),
      ),
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
        final subtitle = item.track.sourceId.toLowerCase() == 'local'
            ? item.path
            : '${item.track.sourceId} • ${item.track.trackId}';
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
            subtitle,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          onTap: () => onActivate(i),
        );
      },
    );
  }
}
