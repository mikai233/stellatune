import 'dart:io';

import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/custom_title_bar.dart';
import 'package:stellatune/ui/widgets/now_playing_bar.dart';

class DesktopShell extends StatelessWidget {
  const DesktopShell({
    super.key,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.child,
  });

  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final Widget child;

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
      NavigationRailDestination(
        icon: const Icon(Icons.settings_outlined),
        selectedIcon: const Icon(Icons.settings),
        label: Text(l10n.navSettings),
      ),
    ];

    return Scaffold(
      body: Column(
        children: [
          if (Platform.isWindows || Platform.isLinux || Platform.isMacOS)
            CustomTitleBar(
              foregroundColor: Theme.of(context).colorScheme.onSurface,
              backgroundColor: Theme.of(
                context,
              ).colorScheme.surfaceContainerLow,
            ),
          Expanded(
            child: Row(
              children: [
                NavigationRail(
                  selectedIndex: selectedIndex,
                  onDestinationSelected: onDestinationSelected,
                  destinations: destinations,
                  labelType: NavigationRailLabelType.all,
                  groupAlignment: -1,
                ),
                const VerticalDivider(width: 1),
                Expanded(
                  child: Column(
                    children: [
                      Expanded(child: child),
                      const NowPlayingBar(),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
