import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:flutter/material.dart';
import 'package:stellatune/ui/widgets/desktop_window_controls.dart';

class DesktopTopBarAction {
  const DesktopTopBarAction({
    required this.icon,
    required this.tooltip,
    required this.onPressed,
  });
  final IconData icon;
  final String tooltip;
  final VoidCallback? onPressed;
}

/// Shared by the app and visual fixtures; native actions are injected.
class DesktopFrame extends StatelessWidget {
  const DesktopFrame({
    super.key,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.child,
    required this.playerBar,
    this.topBarActions = const [],
    this.onSearch,
    this.searchController,
    this.searchHint,
    this.searchEnabled = true,
    this.onClearSearch,
    this.onMinimize,
    this.onMaximize,
    this.onClose,
    this.onDrag,
  });
  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final Widget child, playerBar;
  final List<DesktopTopBarAction> topBarActions;
  final ValueChanged<String>? onSearch;
  final TextEditingController? searchController;
  final String? searchHint;
  final bool searchEnabled;
  final VoidCallback? onClearSearch;
  final VoidCallback? onMinimize, onMaximize, onClose, onDrag;

  @override
  Widget build(BuildContext context) => Scaffold(
    backgroundColor: ArtworkPalette.of(context).top,
    body: Stack(
      children: [
        const Positioned.fill(child: ArtworkBackdrop()),
        ColoredBox(
          color: ArtworkPalette.of(context).onBackdrop.withValues(alpha: .035),
          child: Column(
            children: [
              Expanded(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    SizedBox(
                      width: 188,
                      child: _Sidebar(
                        selected: selectedIndex,
                        onSelect: onDestinationSelected,
                        onDrag: onDrag,
                      ),
                    ),
                    Container(
                      width: 1,
                      margin: const EdgeInsets.symmetric(vertical: 20),
                      color: ArtworkPalette.of(context).onBackdrop
                          .withValues(alpha: .09),
                    ),
                    Expanded(
                      child: Column(
                        children: [
                          SizedBox(
                            key: const ValueKey('desktop-title-bar'),
                            height: 60,
                            child: Stack(
                              children: [
                                Positioned.fill(
                                  child: GestureDetector(
                                    behavior: HitTestBehavior.translucent,
                                    onPanStart: (_) => onDrag?.call(),
                                    onDoubleTap: onMaximize,
                                  ),
                                ),
                                Positioned(
                                  top: 12,
                                  right: 14,
                                  left: selectedIndex == 2 ? 14 : null,
                                  child: _TopBar(
                                    actions: topBarActions,
                                    onSearch: onSearch,
                                    searchController: searchController,
                                    searchHint: searchHint,
                                    searchEnabled: searchEnabled,
                                    onClearSearch: onClearSearch,
                                    onMinimize: onMinimize,
                                    onMaximize: onMaximize,
                                    onClose: onClose,
                                    home: selectedIndex != 2,
                                    showSearch: selectedIndex != 3,
                                  ),
                                ),
                              ],
                            ),
                          ),
                          Expanded(
                            child: ClipRect(
                              key: const ValueKey('desktop-content-viewport'),
                              child: ShaderMask(
                                blendMode: BlendMode.dstIn,
                                shaderCallback: (bounds) =>
                                    const LinearGradient(
                                      begin: Alignment.topCenter,
                                      end: Alignment.bottomCenter,
                                      colors: [
                                        Color(0x00FFFFFF),
                                        Color(0x28FFFFFF),
                                        Color(0xC8FFFFFF),
                                        Colors.white,
                                      ],
                                      stops: [0, .25, .7, 1],
                                    ).createShader(
                                      // Fade content into the shared backdrop, without tinting it.
                                      Rect.fromLTWH(
                                        bounds.left,
                                        bounds.top,
                                        bounds.width,
                                        18,
                                      ),
                                    ),
                                child: child,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              // Progress changes should repaint only the player, not the
              // static page, artwork and background above it.
              RepaintBoundary(child: playerBar),
            ],
          ),
        ),
      ],
    ),
  );
}

class _Sidebar extends StatelessWidget {
  const _Sidebar({required this.selected, required this.onSelect, this.onDrag});
  final int selected;
  final ValueChanged<int> onSelect;
  final VoidCallback? onDrag;
  @override
  Widget build(BuildContext context) => DecoratedBox(
    decoration: BoxDecoration(
      gradient: LinearGradient(
        begin: Alignment.topLeft,
        end: Alignment.bottomRight,
        colors: [
          Colors.black.withValues(alpha: .10),
          Colors.black.withValues(alpha: .02),
        ],
      ),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        GestureDetector(
          behavior: HitTestBehavior.opaque,
          onPanStart: (_) => onDrag?.call(),
          child: Padding(
            padding: EdgeInsets.fromLTRB(26, 26, 12, 22),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Stellatune',
                  style: TextStyle(
                    fontSize: 18,
                    color: ArtworkPalette.of(context).onBackdrop,
                    fontWeight: FontWeight.w500,
                    letterSpacing: -.3,
                  ),
                ),
                SizedBox(height: 8),
                Text(
                  'Music for\na calmer tomorrow.',
                  style: TextStyle(
                    color: ArtworkPalette.of(context).onBackdrop
                        .withValues(alpha: .54),
                    fontSize: 12,
                    height: 1.45,
                  ),
                ),
              ],
            ),
          ),
        ),
        for (var i = 0; i < 4; i++)
          Padding(
            padding: const EdgeInsets.fromLTRB(14, 0, 14, 4),
            child: Material(
              color: selected == i
                  ? ArtworkPalette.of(context).onBackdrop.withValues(alpha: .19)
                  : Colors.transparent,
              borderRadius: BorderRadius.circular(13),
              child: InkWell(
                key: ValueKey('desktop-nav-$i'),
                onTap: () => onSelect(i),
                borderRadius: BorderRadius.circular(13),
                child: SizedBox(
                  height: 47,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    child: Row(
                      children: [
                        Icon(
                          [
                            Icons.home_rounded,
                            Icons.music_note_rounded,
                            Icons.queue_music_rounded,
                            Icons.settings_outlined,
                          ][i],
                          color: ArtworkPalette.of(context).onBackdrop,
                          size: 21,
                        ),
                        SizedBox(width: 16),
                        Text(
                          ['首页', '音乐库', '歌单', '设置'][i],
                          style: TextStyle(
                            fontSize: 14,
                            color: ArtworkPalette.of(context).onBackdrop,
                            fontWeight: selected == i
                                ? FontWeight.w600
                                : FontWeight.w400,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        const Spacer(),
        Padding(
          padding: EdgeInsets.fromLTRB(34, 20, 14, 34),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(
                Icons.auto_awesome,
                size: 34,
                color: ArtworkPalette.of(context).onBackdrop
                    .withValues(alpha: .70),
              ),
              SizedBox(height: 14),
              Text(
                'Good Music.\nA Brighter You.',
                style: TextStyle(
                  color: ArtworkPalette.of(context).onBackdrop
                      .withValues(alpha: .54),
                  fontSize: 12,
                  height: 1.7,
                ),
              ),
            ],
          ),
        ),
      ],
    ),
  );
}

class _TopBar extends StatelessWidget {
  const _TopBar({
    required this.actions,
    required this.home,
    this.showSearch = true,
    this.onSearch,
    this.searchController,
    this.searchHint,
    this.searchEnabled = true,
    this.onClearSearch,
    this.onMinimize,
    this.onMaximize,
    this.onClose,
  });
  final List<DesktopTopBarAction> actions;
  final bool home;
  final bool showSearch;
  final TextEditingController? searchController;
  final String? searchHint;
  final bool searchEnabled;
  final VoidCallback? onClearSearch;
  final ValueChanged<String>? onSearch;
  final VoidCallback? onMinimize, onMaximize, onClose;
  @override
  Widget build(BuildContext context) {
    final compact = MediaQuery.sizeOf(context).width < 1200;
    return Row(
      mainAxisSize: home ? MainAxisSize.min : MainAxisSize.max,
      children: [
        if (!home) ...[
          for (final action in actions)
            _TopBarAction(
              icon: action.icon,
              label: action.tooltip,
              onTap: action.onPressed,
            ),
          const Spacer(),
        ],
        if (showSearch)
          SizedBox(
            width: compact ? 210 : 310,
            height: 39,
            child: TextField(
              key: const ValueKey('desktop-search'),
              controller: searchController,
              enabled: searchEnabled,
              onSubmitted: onSearch,
              style: TextStyle(
                color: ArtworkPalette.of(context).onBackdrop,
                fontSize: 12,
              ),
              cursorColor: ArtworkPalette.of(context).onBackdrop,
              decoration: InputDecoration(
                hintText: searchHint ?? '搜索歌曲、专辑、艺术家…',
                suffixIcon: onClearSearch == null
                    ? null
                    : IconButton(
                        onPressed: onClearSearch,
                        icon: const Icon(Icons.close, size: 16),
                      ),
                hintStyle: TextStyle(
                  color: ArtworkPalette.of(context).onBackdrop
                      .withValues(alpha: .60),
                  fontSize: 12,
                ),
                prefixIcon: Icon(
                  Icons.search_rounded,
                  color: ArtworkPalette.of(context).onBackdrop
                      .withValues(alpha: .70),
                  size: 20,
                ),
                filled: true,
                fillColor: ArtworkPalette.of(context).onBackdrop
                    .withValues(alpha: .07),
                isDense: true,
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 10,
                ),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(24),
                  borderSide: BorderSide(
                    color: ArtworkPalette.of(context).onBackdrop
                        .withValues(alpha: .2),
                  ),
                ),
                enabledBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(24),
                  borderSide: BorderSide(
                    color: ArtworkPalette.of(context).onBackdrop
                        .withValues(alpha: .16),
                  ),
                ),
                focusedBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(24),
                  borderSide: BorderSide(
                    color: ArtworkPalette.of(context).onBackdrop
                        .withValues(alpha: .5),
                  ),
                ),
              ),
            ),
          ),
        if (showSearch) SizedBox(width: compact ? 16 : 48),
        DesktopWindowControls(
          foreground: ArtworkPalette.of(context).onBackdrop,
          onMinimize: onMinimize,
          onMaximize: onMaximize,
          onClose: onClose,
        ),
      ],
    );
  }
}

class _TopBarAction extends StatelessWidget {
  const _TopBarAction({required this.icon, required this.label, this.onTap});

  final IconData icon;
  final String label;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: 42,
    height: 34,
    child: IconButton(
      tooltip: label,
      onPressed: onTap,
      padding: EdgeInsets.zero,
      icon: Icon(
        icon,
        size: 19,
        color: ArtworkPalette.of(context).onBackdrop
            .withValues(alpha: onTap == null ? .35 : .9),
      ),
    ),
  );
}
