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

  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final List<DesktopTopBarAction> topBarActions;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context)!;
    final destinations = <NavigationRailDestination>[
      NavigationRailDestination(
        icon: const Icon(Icons.library_music_outlined),
        selectedIcon: const Icon(Icons.library_music),
        label: Text(l10n.navLibrary),
      ),
      NavigationRailDestination(
        icon: const Icon(Icons.playlist_play_outlined),
        selectedIcon: const Icon(Icons.playlist_play),
        label: Text(l10n.navPlaylists),
      ),
      NavigationRailDestination(
        icon: const Icon(Icons.settings_outlined),
        selectedIcon: const Icon(Icons.settings),
        label: Text(l10n.navSettings),
      ),
    ];

    const topBarHeight = 50.0;
    return Scaffold(
      backgroundColor: theme.colorScheme.surface,
      body: Column(
        children: [
          Expanded(
            child: Stack(
              children: [
                Positioned.fill(
                  child: Padding(
                    padding: const EdgeInsets.only(top: topBarHeight),
                    child: Row(
                      children: [
                        Container(
                          decoration: BoxDecoration(
                            gradient: LinearGradient(
                              begin: Alignment.topCenter,
                              end: Alignment.bottomCenter,
                              colors: [
                                theme.colorScheme.surfaceContainerLow,
                                theme.colorScheme.surfaceContainerLowest,
                              ],
                            ),
                            border: Border(
                              right: BorderSide(
                                color: theme.colorScheme.onSurface.withValues(
                                  alpha: 0.10,
                                ),
                              ),
                            ),
                          ),
                          child: NavigationRail(
                            selectedIndex: selectedIndex,
                            onDestinationSelected: onDestinationSelected,
                            destinations: destinations,
                            labelType: NavigationRailLabelType.all,
                            groupAlignment: -1,
                            backgroundColor: Colors.transparent,
                            indicatorColor: theme.colorScheme.secondaryContainer
                                .withValues(alpha: 0.72),
                            useIndicator: true,
                          ),
                        ),
                        Expanded(
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
                      ],
                    ),
                  ),
                ),
                Positioned(
                  left: 0,
                  top: 0,
                  right: 0,
                  child: _DesktopGlobalTopBar(actions: topBarActions),
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

class _DesktopGlobalTopBar extends StatelessWidget {
  const _DesktopGlobalTopBar({required this.actions});

  final List<DesktopTopBarAction> actions;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context)!;

    final topTintStrong =
        Color.lerp(
          theme.colorScheme.primaryContainer,
          theme.colorScheme.surfaceContainerLow,
          0.38,
        ) ??
        theme.colorScheme.surfaceContainerLow;
    final topTintSoft =
        Color.lerp(
          theme.colorScheme.secondaryContainer,
          theme.colorScheme.surfaceContainerLowest,
          0.55,
        ) ??
        theme.colorScheme.surfaceContainerLowest;

    return Container(
      height: 50,
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [topTintStrong, topTintSoft],
        ),
        border: Border(
          bottom: BorderSide(
            color: theme.colorScheme.primary.withValues(alpha: 0.24),
          ),
        ),
      ),
      child: Row(
        children: [
          SizedBox(
            width: 236,
            child: DragToMoveArea(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: Row(
                    children: [
                      Icon(
                        Icons.music_note_rounded,
                        size: 20,
                        color: theme.colorScheme.onSurface.withValues(
                          alpha: 0.66,
                        ),
                      ),
                      const SizedBox(width: 8),
                      Text(
                        l10n.appTitle,
                        maxLines: 1,
                        overflow: TextOverflow.fade,
                        softWrap: false,
                        style: theme.textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.w600,
                          color: theme.colorScheme.onSurface.withValues(
                            alpha: 0.66,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
          const SizedBox(width: 8),
          if (actions.isNotEmpty)
            Container(
              height: 34,
              padding: const EdgeInsets.symmetric(horizontal: 4),
              decoration: BoxDecoration(
                color: Color.alphaBlend(
                  theme.colorScheme.primary.withValues(alpha: 0.10),
                  theme.colorScheme.primaryContainer.withValues(alpha: 0.66),
                ),
                borderRadius: BorderRadius.circular(12),
                border: Border.all(
                  color: theme.colorScheme.primary.withValues(alpha: 0.26),
                ),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: actions.map((action) {
                  return _TopBarActionButton(
                    icon: action.icon,
                    tooltip: action.tooltip,
                    onPressed: action.onPressed,
                  );
                }).toList(),
              ),
            ),
          Expanded(child: DragToMoveArea(child: const SizedBox.expand())),
          WindowButton(
            icon: Icons.minimize,
            onPressed: () => windowManager.minimize(),
            color: theme.colorScheme.onSurface,
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
            tooltip: l10n.tooltipMaximize,
          ),
          WindowButton(
            icon: Icons.close,
            onPressed: () => windowManager.close(),
            color: theme.colorScheme.onSurface,
            isClose: true,
            tooltip: l10n.tooltipClose,
          ),
        ],
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
