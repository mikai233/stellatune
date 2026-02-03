import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';

class TrackList extends StatelessWidget {
  const TrackList({
    super.key,
    required this.items,
    required this.onActivate,
    required this.onEnqueue,
  });

  final List<TrackLite> items;
  final Future<void> Function(int index, List<TrackLite> items) onActivate;
  final Future<void> Function(TrackLite track) onEnqueue;

  @override
  Widget build(BuildContext context) {
    if (items.isEmpty) {
      return const Center(
        child: Text('No results. Add a folder and scan, then search.'),
      );
    }

    return ListView.separated(
      itemCount: items.length,
      separatorBuilder: (context, index) => const Divider(height: 1),
      itemBuilder: (context, i) {
        final t = items[i];
        final title = (t.title ?? '').trim();
        final artist = (t.artist ?? '').trim();
        final album = (t.album ?? '').trim();

        final line1 = title.isNotEmpty ? title : _basename(t.path);
        final line2 = [artist, album].where((s) => s.isNotEmpty).join(' • ');

        return ListTile(
          dense: true,
          title: Text(line1, maxLines: 1, overflow: TextOverflow.ellipsis),
          subtitle: Text(
            line2.isNotEmpty ? line2 : t.path,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          trailing: PopupMenuButton<_TrackAction>(
            onSelected: (action) async {
              if (action == _TrackAction.enqueue) {
                await onEnqueue(t);
              } else if (action == _TrackAction.play) {
                await onActivate(i, items);
              }
            },
            itemBuilder: (context) => const [
              PopupMenuItem(value: _TrackAction.play, child: Text('Play')),
              PopupMenuItem(
                value: _TrackAction.enqueue,
                child: Text('Enqueue'),
              ),
            ],
          ),
          onTap: () => onActivate(i, items),
        );
      },
    );
  }

  static String _basename(String path) {
    final parts = path.split(RegExp(r'[\\/]+'));
    return parts.isEmpty ? path : parts.last;
  }
}

enum _TrackAction { play, enqueue }
