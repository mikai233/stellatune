import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/library_page.dart';
import 'package:stellatune/ui/pages/queue_page.dart';
import 'package:stellatune/ui/widgets/now_playing_bar.dart';

class ShellPage extends ConsumerStatefulWidget {
  const ShellPage({super.key});

  @override
  ConsumerState<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends ConsumerState<ShellPage> {
  int _index = 0;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final destinations = <NavigationRailDestination>[
      NavigationRailDestination(
        icon: const Icon(Icons.library_music_outlined),
        selectedIcon: const Icon(Icons.library_music),
        label: Text(l10n.navLibrary),
      ),
      NavigationRailDestination(
        icon: const Icon(Icons.queue_music_outlined),
        selectedIcon: const Icon(Icons.queue_music),
        label: Text(l10n.navQueue),
      ),
    ];

    final body = switch (_index) {
      0 => const LibraryPage(),
      _ => const QueuePage(),
    };

    return Scaffold(
      body: Row(
        children: [
          NavigationRail(
            selectedIndex: _index,
            onDestinationSelected: (v) => setState(() => _index = v),
            destinations: destinations,
            labelType: NavigationRailLabelType.all,
            groupAlignment: -1,
          ),
          const VerticalDivider(width: 1),
          Expanded(
            child: Column(
              children: [
                Expanded(child: body),
                const NowPlayingBar(),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
