import 'package:flutter/material.dart';

class PlaylistsPageHeader extends StatelessWidget {
  const PlaylistsPageHeader({
    super.key,
    required this.title,
    required this.panelTooltip,
    required this.createTooltip,
    required this.onTogglePanel,
    required this.onCreatePlaylist,
  });

  final String title;
  final String panelTooltip;
  final String createTooltip;
  final VoidCallback onTogglePanel;
  final VoidCallback onCreatePlaylist;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(10, 6, 10, 6),
      child: SizedBox(
        height: 48,
        child: Row(
          children: [
            IconButton(
              tooltip: panelTooltip,
              icon: const Icon(Icons.playlist_play),
              onPressed: onTogglePanel,
            ),
            Expanded(
              child: Text(
                title,
                style: theme.textTheme.headlineSmall?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            IconButton(
              tooltip: createTooltip,
              icon: const Icon(Icons.playlist_add_outlined),
              onPressed: onCreatePlaylist,
            ),
          ],
        ),
      ),
    );
  }
}
