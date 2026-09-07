import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:stellatune/ui/theme/desktop_theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/ui/widgets/now_playing_bar.dart';
import 'package:window_manager/window_manager.dart';

import 'desktop_frame.dart';

export 'desktop_frame.dart' show DesktopTopBarAction;

class DesktopShell extends ConsumerWidget {
  const DesktopShell({
    super.key,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.topBarActions,
    required this.child,
  });
  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final List<DesktopTopBarAction> topBarActions;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) => ArtworkTheme(
    palette: ref.watch(desktopPaletteProvider),
    child: DesktopFrame(
      selectedIndex: selectedIndex,
      onDestinationSelected: onDestinationSelected,
      topBarActions: topBarActions,
      onSearch: (query) {
        ref.read(libraryControllerProvider.notifier).setQuery(query);
        onDestinationSelected(1);
      },
      onMinimize: () => windowManager.minimize(),
      onMaximize: () async {
        if (await windowManager.isMaximized()) {
          await windowManager.unmaximize();
        } else {
          await windowManager.maximize();
        }
      },
      onClose: () => windowManager.close(),
      onDrag: () => windowManager.startDragging(),
      playerBar: const NowPlayingBar(),
      child: child,
    ),
  );
}
