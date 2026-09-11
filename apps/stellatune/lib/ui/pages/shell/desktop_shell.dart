import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:stellatune/ui/theme/desktop_theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/library/catalog_controller.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/now_playing_bar.dart';
import 'package:window_manager/window_manager.dart';

import 'desktop_frame.dart';

export 'desktop_frame.dart' show DesktopTopBarAction;

class DesktopShell extends ConsumerStatefulWidget {
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
  ConsumerState<DesktopShell> createState() => _DesktopShellState();
}

class _DesktopShellState extends ConsumerState<DesktopShell> {
  final _search = TextEditingController();
  @override
  void initState() {
    super.initState();
    _search.text = ref.read(catalogControllerProvider).search;
  }

  @override
  void dispose() {
    _search.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(catalogControllerProvider.select((s) => s.search), (_, value) {
      if (_search.text != value) {
        _search.value = TextEditingValue(
          text: value,
          selection: TextSelection.collapsed(offset: value.length),
        );
      }
    });
    final source = ref.watch(catalogControllerProvider.select((s) => s.source));
    final search = ref.watch(catalogControllerProvider.select((s) => s.search));
    return ArtworkTheme(
      palette: ref.watch(desktopPaletteProvider),
      child: DesktopFrame(
        selectedIndex: widget.selectedIndex,
        onDestinationSelected: widget.onDestinationSelected,
        topBarActions: widget.topBarActions,
        searchController: _search,
        searchHint: AppLocalizations.of(context)?.catalogSearchHint,
        searchEnabled:
            widget.selectedIndex != 1 ||
            source == null ||
            (source.available && source.searchKinds.isNotEmpty),
        onClearSearch: search.isEmpty
            ? null
            : () {
                _search.clear();
                ref.read(catalogControllerProvider.notifier).setSearch('');
              },
        onSearch: (query) {
          ref.read(catalogControllerProvider.notifier).setSearch(query);
          widget.onDestinationSelected(1);
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
        child: widget.child,
      ),
    );
  }
}
