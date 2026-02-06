import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/lyrics/lyrics_controller.dart';
import 'package:stellatune/l10n/app_localizations.dart';

enum _LyricsMoreAction { toggleLyrics, chooseCandidate }

class LyricsMoreMenuButton extends ConsumerWidget {
  const LyricsMoreMenuButton({super.key, required this.foregroundColor});

  final Color foregroundColor;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final enabled = ref.watch(
      lyricsControllerProvider.select((s) => s.enabled),
    );

    return PopupMenuButton<_LyricsMoreAction>(
      tooltip: l10n.menuMore,
      icon: Icon(Icons.more_vert, color: foregroundColor),
      onSelected: (action) async {
        switch (action) {
          case _LyricsMoreAction.toggleLyrics:
            ref.read(lyricsControllerProvider.notifier).setEnabled(!enabled);
            break;
          case _LyricsMoreAction.chooseCandidate:
            await showLyricsCandidatePicker(context: context, ref: ref);
            break;
        }
      },
      itemBuilder: (context) => [
        PopupMenuItem<_LyricsMoreAction>(
          value: _LyricsMoreAction.toggleLyrics,
          child: Row(
            children: [
              Icon(enabled ? Icons.lyrics : Icons.lyrics_outlined, size: 18),
              const SizedBox(width: 8),
              Text(
                enabled ? l10n.lyricsMoreHideLyrics : l10n.lyricsMoreShowLyrics,
              ),
            ],
          ),
        ),
        PopupMenuItem<_LyricsMoreAction>(
          value: _LyricsMoreAction.chooseCandidate,
          child: Row(
            children: [
              const Icon(Icons.playlist_play, size: 18),
              const SizedBox(width: 8),
              Text(l10n.lyricsMoreChooseCandidate),
            ],
          ),
        ),
      ],
    );
  }
}

Future<void> showLyricsCandidatePicker({
  required BuildContext context,
  required WidgetRef ref,
}) async {
  final controller = ref.read(lyricsControllerProvider.notifier);
  var future = controller.searchCandidatesForCurrent();
  final l10n = AppLocalizations.of(context)!;

  await showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    builder: (sheetContext) {
      return StatefulBuilder(
        builder: (context, setState) {
          return SafeArea(
            child: SizedBox(
              height: MediaQuery.sizeOf(context).height * 0.72,
              child: FutureBuilder<List<LyricsSearchCandidate>>(
                future: future,
                builder: (context, snap) {
                  if (snap.connectionState != ConnectionState.done) {
                    return const Center(child: CircularProgressIndicator());
                  }
                  if (snap.hasError) {
                    return Center(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(l10n.lyricsCandidatesLoadFailed),
                          const SizedBox(height: 8),
                          FilledButton(
                            onPressed: () {
                              setState(() {
                                future = controller
                                    .searchCandidatesForCurrent();
                              });
                            },
                            child: Text(l10n.refresh),
                          ),
                        ],
                      ),
                    );
                  }

                  final items = snap.data ?? const <LyricsSearchCandidate>[];
                  if (items.isEmpty) {
                    return Center(child: Text(l10n.lyricsCandidatesEmpty));
                  }

                  return ListView.separated(
                    itemCount: items.length,
                    separatorBuilder: (_, _) => const Divider(height: 1),
                    itemBuilder: (context, index) {
                      final item = items[index];
                      final info = <String>[
                        if ((item.artist ?? '').trim().isNotEmpty)
                          item.artist!.trim(),
                        if ((item.album ?? '').trim().isNotEmpty)
                          item.album!.trim(),
                        item.source,
                      ].join(' • ');
                      final preview = (item.preview ?? '').trim();
                      return ListTile(
                        title: Text(item.title),
                        subtitle: Text(
                          preview.isNotEmpty ? '$info\n$preview' : info,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                        ),
                        trailing: item.isSynced
                            ? const Icon(Icons.schedule)
                            : const Icon(Icons.notes),
                        onTap: () async {
                          try {
                            await controller.applyCandidate(item);
                            if (context.mounted) {
                              ScaffoldMessenger.of(context).showSnackBar(
                                SnackBar(
                                  content: Text(l10n.lyricsCandidateApplied),
                                ),
                              );
                            }
                          } catch (_) {
                            if (context.mounted) {
                              ScaffoldMessenger.of(context).showSnackBar(
                                SnackBar(
                                  content: Text(
                                    l10n.lyricsCandidateApplyFailed,
                                  ),
                                ),
                              );
                            }
                          }
                          if (sheetContext.mounted) {
                            Navigator.of(sheetContext).pop();
                          }
                        },
                      );
                    },
                  );
                },
              ),
            ),
          );
        },
      );
    },
  );
}
