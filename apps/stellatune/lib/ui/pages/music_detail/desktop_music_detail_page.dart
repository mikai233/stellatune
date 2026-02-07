import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:palette_generator/palette_generator.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/lyrics/lyrics_controller.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/ui/widgets/custom_title_bar.dart';
import 'package:stellatune/ui/widgets/dynamic_background.dart';

import 'widgets/bottom_playback_bar.dart';
import 'widgets/layouts.dart';
import 'widgets/lyrics_more_options.dart';
import 'widgets/queue_drawer_panel.dart';

/// Full-screen music detail page showing album cover, track info and lyrics placeholder.
class DesktopMusicDetailPage extends ConsumerStatefulWidget {
  const DesktopMusicDetailPage({super.key});

  @override
  ConsumerState<DesktopMusicDetailPage> createState() =>
      _DesktopMusicDetailPageState();
}

class _DesktopMusicDetailPageState
    extends ConsumerState<DesktopMusicDetailPage> {
  int? _previousTrackId;
  int? _previousOrderPos;
  int _slideDirection = 0;
  List<Color> _backgroundColors = [
    Colors.grey.shade900,
    Colors.grey.shade800,
    Colors.black,
    Colors.blueGrey.shade900,
  ];
  Color _foregroundColor = Colors.white;
  String? _lastLoadedCover;
  bool? _renderedHasLyrics;
  Timer? _lyricDelayTimer;
  bool _queuePanelOpen = false;

  @override
  void dispose() {
    _lyricDelayTimer?.cancel();
    _lyricDelayTimer = null;
    super.dispose();
  }

  /// Updates the internal state for lyrics transitions, handling conflict animations.
  void _updateLyricsState(bool targetHasLyrics, bool trackChanged) {
    if (_renderedHasLyrics == null) {
      _renderedHasLyrics = targetHasLyrics;
      return;
    }

    if (trackChanged) {
      // Conflict Detection:
      // Case 1: HasLyrics -> NoLyrics (Expand Right) + Next (Slide Left)
      // Case 2: NoLyrics -> HasLyrics (Contract Left) + Previous (Slide Right)
      final isExpansionConflict =
          _renderedHasLyrics == true &&
          targetHasLyrics == false &&
          _slideDirection == 1;
      final isContractionConflict =
          _renderedHasLyrics == false &&
          targetHasLyrics == true &&
          _slideDirection == -1;

      if (isExpansionConflict || isContractionConflict) {
        _lyricDelayTimer?.cancel();
        _lyricDelayTimer = Timer(const Duration(milliseconds: 350), () {
          _lyricDelayTimer = null;
          if (mounted) {
            setState(() => _renderedHasLyrics = targetHasLyrics);
          }
        });
      } else {
        // Harmonized or No Change: Immediate update
        _lyricDelayTimer?.cancel();
        _lyricDelayTimer = null;
        _renderedHasLyrics = targetHasLyrics;
      }
      return;
    }

    if (_renderedHasLyrics != targetHasLyrics) {
      // If not changing tracks but lyrics status changed (e.g. loaded), update immediately
      _lyricDelayTimer?.cancel();
      _lyricDelayTimer = null;
      _renderedHasLyrics = targetHasLyrics;
    }
  }

  void _updatePalette(String coverDir, int? trackId) async {
    if (coverDir.isEmpty || trackId == null) return;
    final coverPath = '$coverDir${Platform.pathSeparator}$trackId';
    if (_lastLoadedCover == coverPath) return;
    _lastLoadedCover = coverPath;

    final file = File(coverPath);
    if (!await file.exists()) {
      debugPrint('Cover file not found for palette: $coverPath');
      return;
    }

    try {
      // Optimize: Only decode a tiny version of the image for palette extraction
      final imageProvider = ResizeImage(
        FileImage(file),
        width: 100,
        height: 100,
      );
      final palette = await PaletteGenerator.fromImageProvider(
        imageProvider,
        maximumColorCount: 24,
      );

      if (mounted) {
        final dominantColor = palette.dominantColor?.color ?? Colors.blueGrey;
        // Calculate contrast color based on luminance
        final foregroundColor = dominantColor.computeLuminance() > 0.5
            ? Colors.black
            : Colors.white;

        // Sort all swatches by population (pixel count) to find the most representative colors
        final sortedSwatches = List<PaletteColor>.from(palette.paletteColors);
        sortedSwatches.sort((a, b) => b.population.compareTo(a.population));

        // Pick the top 4 most common colors for the gradient
        final List<Color> weightedColors = [];
        for (int i = 0; i < 4; i++) {
          if (i < sortedSwatches.length) {
            weightedColors.add(sortedSwatches[i].color);
          } else {
            // Fallback to dominant or preset colors if not enough swatches
            weightedColors.add(i == 0 ? dominantColor : Colors.black);
          }
        }

        setState(() {
          _backgroundColors = weightedColors;
          _foregroundColor = foregroundColor;
        });
      }
    } catch (e) {
      debugPrint('Error generating palette: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final playback = ref.watch(playbackControllerProvider);
    final queue = ref.watch(queueControllerProvider);
    final lyrics = ref.watch(lyricsControllerProvider);
    final coverDir = ref.watch(coverDirProvider);

    final currentItem = queue.currentItem;
    final title = currentItem?.displayTitle ?? l10n.nowPlayingNone;
    final artist = currentItem?.artist?.trim() ?? '';
    final album = currentItem?.album?.trim() ?? '';
    final subtitle = [artist, album].where((s) => s.isNotEmpty).join(' - ');
    final trackId = currentItem?.id;

    final trackChanged = trackId != _previousTrackId;
    // Determine slide direction when track changes
    if (trackChanged && _previousTrackId != null) {
      final len = queue.order.length;
      if (len > 1 && _previousOrderPos != null) {
        // Handle wrap-around cases (looping)
        if (queue.orderPos == 0 && _previousOrderPos == len - 1) {
          _slideDirection = 1; // Wrapped from end to start
        } else if (queue.orderPos == len - 1 && _previousOrderPos == 0) {
          _slideDirection = -1; // Wrapped from start to end
        } else {
          _slideDirection = queue.orderPos > _previousOrderPos! ? 1 : -1;
        }
      }
    }

    // Trigger palette update
    if (trackId != null) {
      _updatePalette(coverDir, trackId);
    }

    final isPlaying =
        playback.playerState == PlayerState.playing ||
        playback.playerState == PlayerState.buffering;
    final positionMs = playback.positionMs;
    final durationMs = currentItem?.durationMs ?? 0;

    final hasLyrics = lyrics.enabled && lyrics.hasLyrics;
    _updateLyricsState(hasLyrics, trackChanged);

    // Finally update previous states for next build
    _previousOrderPos = queue.orderPos;
    _previousTrackId = trackId;

    return ShaderBackground(
      colors: _backgroundColors,
      child: Scaffold(
        backgroundColor: Colors.transparent,
        body: TweenAnimationBuilder<Color?>(
          duration: const Duration(milliseconds: 600),
          curve: Curves.easeInOut,
          tween: ColorTween(end: _foregroundColor),
          builder: (context, color, child) {
            final effectiveColor = color ?? _foregroundColor;
            final panelWidth = (MediaQuery.sizeOf(context).width * 0.38).clamp(
              320.0,
              460.0,
            );
            return Stack(
              children: [
                SafeArea(
                  child: Column(
                    children: [
                      if (Platform.isWindows ||
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
                      // Main content area
                      Expanded(
                        child: LayoutBuilder(
                          builder: (context, constraints) {
                            final isWide = constraints.maxWidth > 700;
                            final effectiveHasLyrics =
                                _renderedHasLyrics ?? hasLyrics;

                            if (isWide) {
                              return WideLayout(
                                coverDir: coverDir,
                                trackId: trackId,
                                title: title,
                                subtitle: subtitle,
                                slideDirection: _slideDirection,
                                foregroundColor: effectiveColor,
                                currentPath: playback.currentPath,
                                sampleRate: playback.trackInfo?.sampleRate,
                                maxWidth: constraints.maxWidth,
                                maxHeight: constraints.maxHeight,
                                hasLyrics: effectiveHasLyrics,
                                lyricLines: lyrics.lines,
                                currentLyricLineIndex: lyrics.currentLineIndex,
                              );
                            }
                            return NarrowLayout(
                              coverDir: coverDir,
                              trackId: trackId,
                              title: title,
                              subtitle: subtitle,
                              slideDirection: _slideDirection,
                              foregroundColor: effectiveColor,
                              currentPath: playback.currentPath,
                              sampleRate: playback.trackInfo?.sampleRate,
                              maxHeight: constraints.maxHeight,
                              hasLyrics: effectiveHasLyrics,
                              lyricLines: lyrics.lines,
                              currentLyricLineIndex: lyrics.currentLineIndex,
                            );
                          },
                        ),
                      ),
                      // Bottom playback bar
                      BottomPlaybackBar(
                        positionMs: positionMs,
                        durationMs: durationMs,
                        isPlaying: isPlaying,
                        playMode: queue.playMode,
                        volume: playback.volume,
                        foregroundColor: effectiveColor,
                        currentPath: playback.currentPath,
                        sampleRate: playback.trackInfo?.sampleRate,
                        onPlayPause: () => isPlaying
                            ? ref
                                  .read(playbackControllerProvider.notifier)
                                  .pause()
                            : ref
                                  .read(playbackControllerProvider.notifier)
                                  .play(),
                        onPrevious: () => ref
                            .read(playbackControllerProvider.notifier)
                            .previous(),
                        onNext: () => ref
                            .read(playbackControllerProvider.notifier)
                            .next(),
                        onSeek: (ms) => ref
                            .read(playbackControllerProvider.notifier)
                            .seekMs(ms),
                        onVolumeChanged: (v) => ref
                            .read(playbackControllerProvider.notifier)
                            .setVolume(v),
                        onToggleMute: () => ref
                            .read(playbackControllerProvider.notifier)
                            .toggleMute(),
                        enableVolumeHover: true,
                        onPlayModeChanged: () => ref
                            .read(queueControllerProvider.notifier)
                            .cyclePlayMode(),
                        onQueuePressed: () {
                          setState(() => _queuePanelOpen = !_queuePanelOpen);
                        },
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
