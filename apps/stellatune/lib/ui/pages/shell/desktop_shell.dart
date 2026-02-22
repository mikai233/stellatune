import 'dart:math' as math;
import 'dart:ui' show ImageFilter;

import 'package:animations/animations.dart';
import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/custom_title_bar.dart' show WindowButton;
import 'package:stellatune/ui/widgets/now_playing_bar.dart';
import 'package:window_manager/window_manager.dart';

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

class DesktopShell extends StatelessWidget {
  const DesktopShell({
    super.key,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.topBarActions,
    required this.child,
  });

  static const double _contentBarTop = 0;
  static const double _contentBarHeight = 56;
  static const double _sidebarExtendedWidth = 177;
  static const double _sidebarItemHeight = 40;
  static const double _sidebarItemGap = 8;
  static const double _sidebarCapsuleHorizontalInset = 4;
  static const double _nonHomeContentTopInset =
      _contentBarTop + _contentBarHeight;

  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final List<DesktopTopBarAction> topBarActions;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context)!;
    final destinations = <_SidebarDestination>[
      _SidebarDestination(
        icon: Icons.home_outlined,
        selectedIcon: Icons.home,
        label: l10n.navHome,
      ),
      _SidebarDestination(
        icon: Icons.library_music_outlined,
        selectedIcon: Icons.library_music,
        label: l10n.navLibrary,
      ),
      _SidebarDestination(
        icon: Icons.playlist_play_outlined,
        selectedIcon: Icons.playlist_play,
        label: l10n.navPlaylists,
      ),
      _SidebarDestination(
        icon: Icons.settings_outlined,
        selectedIcon: Icons.settings,
        label: l10n.navSettings,
      ),
    ];

    final contentTopInset = selectedIndex == 0 ? 0.0 : _nonHomeContentTopInset;

    return Scaffold(
      backgroundColor: theme.colorScheme.surface,
      body: Column(
        children: [
          Expanded(
            child: Row(
              children: [
                SizedBox(
                  width: _sidebarExtendedWidth,
                  child: ClipRect(
                    child: Stack(
                      children: [
                        Positioned.fill(
                          child: BackdropFilter(
                            filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
                            child: DecoratedBox(
                              decoration: BoxDecoration(
                                gradient: LinearGradient(
                                  begin: Alignment.topLeft,
                                  end: Alignment.bottomRight,
                                  colors: [
                                    Color.alphaBlend(
                                      theme.colorScheme.primary.withValues(
                                        alpha: 0.12,
                                      ),
                                      theme.colorScheme.surface.withValues(
                                        alpha: 0.88,
                                      ),
                                    ),
                                    Color.alphaBlend(
                                      theme.colorScheme.tertiary.withValues(
                                        alpha: 0.08,
                                      ),
                                      theme.colorScheme.surfaceContainerLowest
                                          .withValues(alpha: 0.80),
                                    ),
                                  ],
                                ),
                                border: Border(
                                  right: BorderSide(
                                    color: theme.colorScheme.onSurface
                                        .withValues(alpha: 0.08),
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
                        Positioned(
                          left: -34,
                          top: -24,
                          child: IgnorePointer(
                            child: _SidebarGlowOrb(
                              size: 112,
                              color: theme.colorScheme.primary.withValues(
                                alpha: 0.16,
                              ),
                            ),
                          ),
                        ),
                        Positioned(
                          right: -44,
                          top: 186,
                          child: IgnorePointer(
                            child: _SidebarGlowOrb(
                              size: 126,
                              color: theme.colorScheme.secondary.withValues(
                                alpha: 0.11,
                              ),
                            ),
                          ),
                        ),
                        Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            _SidebarBrand(title: l10n.appTitle),
                            const SizedBox(height: 8),
                            Padding(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 10,
                              ),
                              child: _DesktopSidebarNav(
                                items: destinations,
                                selectedIndex: selectedIndex,
                                itemHeight: _sidebarItemHeight,
                                itemGap: _sidebarItemGap,
                                horizontalInset: _sidebarCapsuleHorizontalInset,
                                onDestinationSelected: onDestinationSelected,
                              ),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ),
                Expanded(
                  child: Stack(
                    children: [
                      Positioned.fill(
                        child: Padding(
                          padding: EdgeInsets.only(top: contentTopInset),
                          child: DecoratedBox(
                            decoration: BoxDecoration(
                              gradient: LinearGradient(
                                begin: Alignment.topCenter,
                                end: Alignment.bottomCenter,
                                colors: [
                                  theme.colorScheme.surface,
                                  theme.colorScheme.surfaceContainerLowest
                                      .withValues(alpha: 0.45),
                                ],
                              ),
                            ),
                            child: PageTransitionSwitcher(
                              duration: const Duration(milliseconds: 300),
                              reverse: false,
                              transitionBuilder:
                                  (child, animation, secondaryAnimation) {
                                    return FadeThroughTransition(
                                      animation: animation,
                                      secondaryAnimation: secondaryAnimation,
                                      child: child,
                                    );
                                  },
                              child: KeyedSubtree(
                                key: ValueKey(selectedIndex),
                                child: child,
                              ),
                            ),
                          ),
                        ),
                      ),
                      Positioned(
                        left: 0,
                        right: 0,
                        top: _contentBarTop,
                        child: _DesktopContentAppBar(
                          searchHint: l10n.searchHint,
                          actions: topBarActions,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          const NowPlayingBar(),
        ],
      ),
    );
  }
}

class _SidebarBrand extends StatelessWidget {
  const _SidebarBrand({required this.title});

  final String title;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DragToMoveArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 10, 16, 14),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 24,
              height: 24,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(8),
                color: Color.alphaBlend(
                  theme.colorScheme.primary.withValues(alpha: 0.35),
                  theme.colorScheme.primaryContainer,
                ),
              ),
              child: Icon(
                Icons.music_note_rounded,
                size: 16,
                color: theme.colorScheme.primary,
              ),
            ),
            const SizedBox(width: 10),
            Text(
              title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.titleMedium?.copyWith(
                fontWeight: FontWeight.w700,
                color: theme.colorScheme.onSurface.withValues(alpha: 0.76),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _SidebarDestination {
  const _SidebarDestination({
    required this.icon,
    required this.selectedIcon,
    required this.label,
  });

  final IconData icon;
  final IconData selectedIcon;
  final String label;
}

class _SidebarGlowOrb extends StatelessWidget {
  const _SidebarGlowOrb({required this.size, required this.color});

  final double size;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return ImageFiltered(
      imageFilter: ImageFilter.blur(sigmaX: 14, sigmaY: 14),
      child: Container(
        width: size,
        height: size,
        decoration: BoxDecoration(shape: BoxShape.circle, color: color),
      ),
    );
  }
}

class _DesktopSidebarNav extends StatelessWidget {
  const _DesktopSidebarNav({
    required this.items,
    required this.selectedIndex,
    required this.itemHeight,
    required this.itemGap,
    required this.horizontalInset,
    required this.onDestinationSelected,
  });

  final List<_SidebarDestination> items;
  final int selectedIndex;
  final double itemHeight;
  final double itemGap;
  final double horizontalInset;
  final ValueChanged<int> onDestinationSelected;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final step = itemHeight + itemGap;
    final safeIndex = selectedIndex.clamp(0, items.length - 1).toInt();
    final capsuleGradientTop = Color.alphaBlend(
      Colors.white.withValues(alpha: 0.48),
      theme.colorScheme.primary.withValues(alpha: 0.20),
    );
    final capsuleGradientBottom = Color.alphaBlend(
      theme.colorScheme.secondary.withValues(alpha: 0.12),
      theme.colorScheme.surface.withValues(alpha: 0.40),
    );

    return SizedBox(
      height: step * items.length - itemGap,
      child: Stack(
        children: [
          AnimatedPositioned(
            duration: const Duration(milliseconds: 380),
            curve: const Cubic(0.22, 1.0, 0.36, 1.0),
            left: horizontalInset,
            right: horizontalInset,
            top: step * safeIndex + 1,
            child: IgnorePointer(
              child: ClipRRect(
                borderRadius: BorderRadius.circular(999),
                child: BackdropFilter(
                  filter: ImageFilter.blur(sigmaX: 12, sigmaY: 12),
                  child: Container(
                    height: itemHeight - 2,
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(999),
                      gradient: LinearGradient(
                        begin: Alignment.topCenter,
                        end: Alignment.bottomCenter,
                        colors: [capsuleGradientTop, capsuleGradientBottom],
                      ),
                      border: Border.all(
                        color: Colors.white.withValues(alpha: 0.32),
                      ),
                      boxShadow: [
                        BoxShadow(
                          color: theme.colorScheme.primary.withValues(
                            alpha: 0.16,
                          ),
                          blurRadius: 16,
                          spreadRadius: 0.2,
                          offset: const Offset(0.8, 3.0),
                        ),
                        BoxShadow(
                          color: theme.colorScheme.shadow.withValues(
                            alpha: 0.06,
                          ),
                          blurRadius: 8,
                          spreadRadius: 0,
                          offset: const Offset(0, 1),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
          AnimatedPositioned(
            duration: const Duration(milliseconds: 380),
            curve: const Cubic(0.22, 1.0, 0.36, 1.0),
            left: 14,
            right: 16,
            top: step * safeIndex + 4,
            child: IgnorePointer(
              child: Container(
                height: 8,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(999),
                  gradient: LinearGradient(
                    begin: Alignment.topCenter,
                    end: Alignment.bottomCenter,
                    colors: [
                      Colors.white.withValues(alpha: 0.34),
                      Colors.white.withValues(alpha: 0.05),
                    ],
                  ),
                ),
              ),
            ),
          ),
          Column(
            children: [
              for (var i = 0; i < items.length; i++) ...[
                _SidebarNavItem(
                  item: items[i],
                  selected: i == safeIndex,
                  height: itemHeight,
                  horizontalInset: horizontalInset,
                  onTap: () => onDestinationSelected(i),
                ),
                if (i != items.length - 1) SizedBox(height: itemGap),
              ],
            ],
          ),
        ],
      ),
    );
  }
}

class _SidebarNavItem extends StatelessWidget {
  const _SidebarNavItem({
    required this.item,
    required this.selected,
    required this.height,
    required this.horizontalInset,
    required this.onTap,
  });

  final _SidebarDestination item;
  final bool selected;
  final double height;
  final double horizontalInset;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final iconColor = selected
        ? theme.colorScheme.onSecondaryContainer.withValues(alpha: 0.90)
        : theme.colorScheme.onSurface.withValues(alpha: 0.66);
    final labelColor = selected
        ? theme.colorScheme.onSecondaryContainer.withValues(alpha: 0.92)
        : theme.colorScheme.onSurface.withValues(alpha: 0.72);

    return Padding(
      padding: EdgeInsets.symmetric(horizontal: horizontalInset),
      child: SizedBox(
        height: height,
        child: Material(
          type: MaterialType.transparency,
          child: InkWell(
            borderRadius: BorderRadius.circular(999),
            onTap: onTap,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 15),
              child: Row(
                children: [
                  Icon(
                    selected ? item.selectedIcon : item.icon,
                    size: 18,
                    color: iconColor,
                  ),
                  const SizedBox(width: 11),
                  Expanded(
                    child: Text(
                      item.label,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.titleMedium?.copyWith(
                        fontSize: 14,
                        color: labelColor,
                        height: 1.02,
                        letterSpacing: selected ? 0.08 : 0.0,
                        fontWeight: selected
                            ? FontWeight.w600
                            : FontWeight.w500,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _DesktopContentAppBar extends StatelessWidget {
  const _DesktopContentAppBar({
    required this.searchHint,
    required this.actions,
  });

  final String searchHint;
  final List<DesktopTopBarAction> actions;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context)!;

    return ClipRect(
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 12, sigmaY: 12),
        child: Container(
          height: 56,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(
            color: Color.alphaBlend(
              theme.colorScheme.surfaceContainer.withValues(alpha: 0.30),
              theme.colorScheme.surface.withValues(alpha: 0.70),
            ),
          ),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final actionWidth = actions.length * 41.0;
              final reservedRight = 156.0;
              final available =
                  constraints.maxWidth - actionWidth - reservedRight - 28;
              final searchWidth = math.max(220.0, math.min(320.0, available));

              return Stack(
                children: [
                  Positioned.fill(
                    child: Row(
                      children: [
                        ...actions.map(
                          (action) => _TopBarActionButton(
                            icon: action.icon,
                            tooltip: action.tooltip,
                            onPressed: action.onPressed,
                          ),
                        ),
                        Expanded(
                          child: DragToMoveArea(child: const SizedBox.expand()),
                        ),
                        WindowButton(
                          icon: Icons.minimize,
                          onPressed: () => windowManager.minimize(),
                          color: theme.colorScheme.onSurface,
                          height: 40,
                          tooltip: l10n.tooltipMinimize,
                        ),
                        WindowButton(
                          icon: Icons.crop_square,
                          onPressed: () async {
                            if (await windowManager.isMaximized()) {
                              windowManager.restore();
                            } else {
                              windowManager.maximize();
                            }
                          },
                          color: theme.colorScheme.onSurface,
                          height: 40,
                          tooltip: l10n.tooltipMaximize,
                        ),
                        WindowButton(
                          icon: Icons.close,
                          onPressed: () => windowManager.close(),
                          color: theme.colorScheme.onSurface,
                          isClose: true,
                          height: 40,
                          tooltip: l10n.tooltipClose,
                        ),
                      ],
                    ),
                  ),
                  Align(
                    alignment: Alignment.center,
                    child: SizedBox(
                      width: searchWidth,
                      child: SizedBox(
                        height: 38,
                        child: TextField(
                          textAlignVertical: TextAlignVertical.center,
                          decoration: InputDecoration(
                            hintText: searchHint,
                            prefixIcon: const Icon(
                              Icons.search_rounded,
                              size: 18,
                            ),
                            prefixIconConstraints: const BoxConstraints(
                              minWidth: 34,
                              minHeight: 34,
                            ),
                            isDense: true,
                            filled: true,
                            fillColor: Color.alphaBlend(
                              theme.colorScheme.surfaceContainerHighest
                                  .withValues(alpha: 0.52),
                              theme.colorScheme.surface.withValues(alpha: 0.78),
                            ),
                            hintStyle: theme.textTheme.bodyMedium?.copyWith(
                              color: theme.colorScheme.onSurface.withValues(
                                alpha: 0.60,
                              ),
                            ),
                            border: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(24),
                              borderSide: BorderSide(
                                color: theme.colorScheme.onSurface.withValues(
                                  alpha: 0.14,
                                ),
                              ),
                            ),
                            enabledBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(24),
                              borderSide: BorderSide(
                                color: theme.colorScheme.onSurface.withValues(
                                  alpha: 0.14,
                                ),
                              ),
                            ),
                            focusedBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(24),
                              borderSide: BorderSide(
                                color: theme.colorScheme.primary.withValues(
                                  alpha: 0.48,
                                ),
                              ),
                            ),
                            contentPadding: const EdgeInsets.symmetric(
                              vertical: 8,
                              horizontal: 12,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}

class _TopBarActionButton extends StatefulWidget {
  const _TopBarActionButton({
    required this.icon,
    required this.tooltip,
    this.onPressed,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback? onPressed;

  @override
  State<_TopBarActionButton> createState() => _TopBarActionButtonState();
}

class _TopBarActionButtonState extends State<_TopBarActionButton> {
  bool _isHovered = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final isEnabled = widget.onPressed != null;

    final backgroundColor = _isHovered
        ? theme.colorScheme.primary.withValues(alpha: 0.12)
        : Colors.transparent;

    final borderColor = _isHovered
        ? theme.colorScheme.primary.withValues(alpha: 0.35)
        : Colors.transparent;

    final borderWidth = _isHovered ? 1.5 : 0.0;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 1.5),
      child: Tooltip(
        message: widget.tooltip,
        waitDuration: const Duration(milliseconds: 600),
        child: MouseRegion(
          onEnter: (_) => setState(() => _isHovered = true),
          onExit: (_) => setState(() => _isHovered = false),
          cursor: isEnabled
              ? SystemMouseCursors.click
              : SystemMouseCursors.basic,
          child: GestureDetector(
            onTap: widget.onPressed,
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 240),
              curve: Curves.easeOutCubic,
              width: 38,
              height: 28,
              decoration: BoxDecoration(
                color: isEnabled ? backgroundColor : Colors.transparent,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(
                  color: isEnabled ? borderColor : Colors.transparent,
                  width: isEnabled ? borderWidth : 0.0,
                ),
              ),
              child: Icon(
                widget.icon,
                size: 17,
                color: isEnabled
                    ? theme.colorScheme.onSurface.withValues(
                        alpha: _isHovered ? 1.0 : 0.8,
                      )
                    : theme.colorScheme.onSurface.withValues(alpha: 0.25),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
