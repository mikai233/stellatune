import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/theme/artwork_palette_provider.dart';
import 'package:stellatune/ui/widgets/custom_title_bar.dart';
import 'package:stellatune/ui/widgets/dynamic_background.dart';

import 'widgets/detail_playback_controls.dart';
import 'widgets/detail_track_content.dart';
import 'widgets/lyrics_more_options.dart';
import 'widgets/queue_drawer_panel.dart';

/// Shared detail surface. Desktop/mobile differ only in their title bar,
/// available layout and queue drawer width.
class MusicDetailScaffold extends ConsumerStatefulWidget {
  const MusicDetailScaffold({super.key, required this.mobile});
  final bool mobile;

  @override
  ConsumerState<MusicDetailScaffold> createState() =>
      _MusicDetailScaffoldState();
}

class _MusicDetailScaffoldState extends ConsumerState<MusicDetailScaffold> {
  bool _queuePanelOpen = false;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final palette =
        ref.watch(currentArtworkPaletteProvider).value ??
        DetailArtworkPalette.neutral;
    return ShaderBackground(
      colors: palette.colors,
      child: Scaffold(
        backgroundColor: Colors.transparent,
        body: TweenAnimationBuilder<Color?>(
          duration: const Duration(milliseconds: 600),
          curve: Curves.easeInOut,
          tween: ColorTween(end: palette.foreground),
          builder: (context, color, child) {
            final effectiveColor = color ?? palette.foreground;
            final width = MediaQuery.sizeOf(context).width;
            final panelWidth = widget.mobile
                ? (width * .86).clamp(280.0, 420.0)
                : (width * .38).clamp(320.0, 460.0);
            return Stack(
              children: [
                SafeArea(
                  child: Column(
                    children: [
                      if (widget.mobile)
                        Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 8,
                            vertical: 4,
                          ),
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.spaceBetween,
                            children: [
                              IconButton(
                                icon: Icon(
                                  Icons.keyboard_arrow_down,
                                  color: effectiveColor,
                                ),
                                tooltip: l10n.tooltipBack,
                                onPressed: () => Navigator.of(context).pop(),
                              ),
                              LyricsMoreMenuButton(
                                foregroundColor: effectiveColor,
                              ),
                            ],
                          ),
                        )
                      else if (Platform.isWindows ||
                          Platform.isLinux ||
                          Platform.isMacOS)
                        CustomTitleBar(
                          foregroundColor: effectiveColor,
                          showTitle: false,
                          height: 50,
                          leading: TitleBarButton(
                            icon: Icons.keyboard_arrow_down,
                            color: effectiveColor,
                            height: 50,
                            tooltip: l10n.tooltipBack,
                            onPressed: () => Navigator.of(context).pop(),
                          ),
                          trailing: LyricsMoreMenuButton(
                            foregroundColor: effectiveColor,
                            height: 50,
                          ),
                        ),
                      Expanded(
                        child: RepaintBoundary(
                          child: DetailTrackContent(
                            foregroundColor: effectiveColor,
                            mobile: widget.mobile,
                          ),
                        ),
                      ),
                      RepaintBoundary(
                        child: DetailPlaybackControls(
                          foregroundColor: effectiveColor,
                          enableVolumeHover: !widget.mobile,
                          onQueuePressed: () => setState(
                            () => _queuePanelOpen = !_queuePanelOpen,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                IgnorePointer(
                  ignoring: !_queuePanelOpen,
                  child: AnimatedOpacity(
                    duration: const Duration(milliseconds: 220),
                    opacity: _queuePanelOpen ? 1.0 : 0.0,
                    child: GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onTap: () => setState(() => _queuePanelOpen = false),
                      child: Container(color: Colors.black26),
                    ),
                  ),
                ),
                Align(
                  alignment: Alignment.centerRight,
                  child: AnimatedSlide(
                    duration: const Duration(milliseconds: 260),
                    curve: Curves.easeOutCubic,
                    offset: _queuePanelOpen
                        ? Offset.zero
                        : const Offset(1.0, 0),
                    child: SizedBox(
                      width: panelWidth,
                      child: const QueueDrawerPanel(),
                    ),
                  ),
                ),
              ],
            );
          },
        ),
      ),
    );
  }
}
