import 'package:animations/animations.dart';
import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/mobile_now_playing_bar.dart';

class MobileShell extends StatelessWidget {
  const MobileShell({
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
    final destinations = <NavigationDestination>[
      NavigationDestination(
        icon: const Icon(Icons.library_music_outlined),
        selectedIcon: const Icon(Icons.library_music),
        label: l10n.navLibrary,
      ),
      NavigationDestination(
        icon: const Icon(Icons.playlist_play_outlined),
        selectedIcon: const Icon(Icons.playlist_play),
        label: l10n.navPlaylists,
      ),
      NavigationDestination(
        icon: const Icon(Icons.settings_outlined),
        selectedIcon: const Icon(Icons.settings),
        label: l10n.navSettings,
      ),
    ];

    return Scaffold(
      appBar: AppBar(title: Text(l10n.appTitle)),
      drawer: Drawer(
        child: ListView(
          padding: EdgeInsets.zero,
          children: [
            DrawerHeader(
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.primary,
              ),
              child: Text(
                l10n.appTitle,
                style: TextStyle(
                  color: Theme.of(context).colorScheme.onPrimary,
                  fontSize: 24,
                ),
              ),
            ),
            // Placeholder items
            ListTile(
              leading: const Icon(Icons.info),
              title: const Text('About'),
              onTap: () {
                Navigator.pop(context);
                // TODO: Navigate to About
              },
            ),
          ],
        ),
      ),
      body: Column(
        children: [
          Expanded(
            child: PageTransitionSwitcher(
              duration: const Duration(milliseconds: 300),
              reverse: false,
              transitionBuilder: (child, animation, secondaryAnimation) {
                return FadeThroughTransition(
                  animation: animation,
                  secondaryAnimation: secondaryAnimation,
                  child: child,
                );
              },
              child: KeyedSubtree(key: ValueKey(selectedIndex), child: child),
            ),
          ),
          const MobileNowPlayingBar(),
        ],
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: selectedIndex,
        onDestinationSelected: onDestinationSelected,
        destinations: destinations,
      ),
    );
  }
}
