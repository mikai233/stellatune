import 'dart:math';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:palette_generator/palette_generator.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/widgets/dynamic_background.dart';
import 'package:stellatune/ui/widgets/custom_title_bar.dart';
import 'package:stellatune/ui/widgets/audio_format_badge.dart';
import 'package:stellatune/ui/widgets/marquee_text.dart';

/// Full-screen music detail page showing album cover, track info and lyrics placeholder.
class MusicDetailPage extends ConsumerStatefulWidget {
  const MusicDetailPage({super.key});

  @override
  ConsumerState<MusicDetailPage> createState() => _MusicDetailPageState();
}

class _MusicDetailPageState extends ConsumerState<MusicDetailPage> {
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
    final coverDir = ref.watch(coverDirProvider);

    final currentItem = queue.currentItem;
    final title = currentItem?.displayTitle ?? l10n.nowPlayingNone;
    final artist = currentItem?.artist?.trim() ?? '';
    final album = currentItem?.album?.trim() ?? '';
    final subtitle = [artist, album].where((s) => s.isNotEmpty).join(' - ');
    final trackId = currentItem?.id;

    // Determine slide direction when track changes
    if (trackId != _previousTrackId && _previousTrackId != null) {
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
    _previousOrderPos = queue.orderPos;
    _previousTrackId = trackId;

    // Trigger palette update
    if (trackId != null) {
      _updatePalette(coverDir, trackId);
    }

    final isPlaying =
        playback.playerState == PlayerState.playing ||
        playback.playerState == PlayerState.buffering;
    final positionMs = playback.positionMs;
    final durationMs = currentItem?.durationMs ?? 0;

    return ShaderBackground(
      colors: _backgroundColors,
      child: Scaffold(
        backgroundColor: Colors.transparent,
        body: Column(
          children: [
            if (Platform.isWindows || Platform.isLinux || Platform.isMacOS)
              CustomTitleBar(foregroundColor: _foregroundColor),
            // Custom App Bar Row
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  IconButton(
                    icon: Icon(
                      Icons.keyboard_arrow_down,
                      color: _foregroundColor,
                    ),
                    tooltip: l10n.tooltipBack,
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                  IconButton(
                    icon: Icon(Icons.more_vert, color: _foregroundColor),
                    tooltip: l10n.menuMore,
                    onPressed: () {
                      // TODO: Show more options menu
                    },
                  ),
                ],
              ),
            ),
            // Main content area
            Expanded(
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final isWide = constraints.maxWidth > 700;
                  final hasLyrics = (trackId ?? 0) % 2 == 0;

                  if (isWide) {
                    return _WideLayout(
                      coverDir: coverDir,
                      trackId: trackId,
                      title: title,
                      subtitle: subtitle,
                      slideDirection: _slideDirection,
                      foregroundColor: _foregroundColor,
                      currentPath: playback.currentPath,
                      sampleRate: playback.trackInfo?.sampleRate,
                      maxWidth: constraints.maxWidth,
                      maxHeight: constraints.maxHeight,
                      hasLyrics: hasLyrics,
                    );
                  }
                  return _NarrowLayout(
                    coverDir: coverDir,
                    trackId: trackId,
                    title: title,
                    subtitle: subtitle,
                    slideDirection: _slideDirection,
                    foregroundColor: _foregroundColor,
                    currentPath: playback.currentPath,
                    sampleRate: playback.trackInfo?.sampleRate,
                    maxHeight: constraints.maxHeight,
                    hasLyrics: hasLyrics,
                  );
                },
              ),
            ),
            // Bottom playback bar
            _BottomPlaybackBar(
              positionMs: positionMs,
              durationMs: durationMs,
              isPlaying: isPlaying,
              playMode: queue.playMode,
              volume: playback.volume,
              foregroundColor: _foregroundColor,
              currentPath: playback.currentPath,
              sampleRate: playback.trackInfo?.sampleRate,
              onPlayPause: () => isPlaying
                  ? ref.read(playbackControllerProvider.notifier).pause()
                  : ref.read(playbackControllerProvider.notifier).play(),
              onPrevious: () =>
                  ref.read(playbackControllerProvider.notifier).previous(),
              onNext: () =>
                  ref.read(playbackControllerProvider.notifier).next(),
              onSeek: (ms) =>
                  ref.read(playbackControllerProvider.notifier).seekMs(ms),
              onVolumeChanged: (v) =>
                  ref.read(playbackControllerProvider.notifier).setVolume(v),
              onPlayModeChanged: () =>
                  ref.read(queueControllerProvider.notifier).cyclePlayMode(),
            ),
          ],
        ),
      ),
    );
  }
}

/// Wide layout: cover + info on left, lyrics on right.
class _WideLayout extends StatelessWidget {
  const _WideLayout({
    required this.coverDir,
    required this.trackId,
    required this.title,
    required this.subtitle,
    required this.slideDirection,
    required this.foregroundColor,
    this.currentPath,
    this.sampleRate,
    required this.maxWidth,
    required this.maxHeight,
    required this.hasLyrics,
  });

  final String coverDir;
  final int? trackId;
  final String title;
  final String subtitle;
  final int slideDirection; // 1 = forward, -1 = backward, 0 = initial
  final Color foregroundColor;
  final String? currentPath;
  final int? sampleRate;
  final double maxWidth;
  final double maxHeight;
  final bool hasLyrics;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);

    // Dynamic scaling based on available height and width
    // Limit cover size to at most 65% of screen height or width
    final baseSize = min(maxWidth * (hasLyrics ? 0.45 : 0.6), maxHeight * 0.55);
    final coverSize = baseSize.clamp(280.0, 600.0);

    // Font size scaling (relative to cover size)
    final titleFontSize = (coverSize / 11).clamp(22.0, 48.0);
    final subtitleFontSize = (coverSize / 17).clamp(14.0, 24.0);

    final textStyle = theme.textTheme.headlineSmall?.copyWith(
      fontWeight: FontWeight.bold,
      color: foregroundColor,
      fontSize: titleFontSize,
    );
    final subtitleStyle = theme.textTheme.bodyLarge?.copyWith(
      color: foregroundColor.withValues(alpha: 0.7),
      fontSize: subtitleFontSize,
    );

    // Slide offset for the carousel effect (must be > 1.0 for a physical gap)
    final slideOffset = slideDirection >= 0 ? 1.05 : -1.05;

    return Row(
      children: [
        // Left side: Cover + Info
        Expanded(
          child: ShaderMask(
            shaderCallback: (Rect bounds) {
              return const LinearGradient(
                begin: Alignment.centerLeft,
                end: Alignment.centerRight,
                colors: [
                  Colors.transparent,
                  Colors.black,
                  Colors.black,
                  Colors.transparent,
                ],
                stops: [0.0, 0.08, 0.92, 1.0],
              ).createShader(bounds);
            },
            blendMode: BlendMode.dstIn,
            child: ClipRect(
              child: OverflowBox(
                minWidth: 400,
                maxWidth: double.infinity,
                alignment: Alignment.center,
                child: AnimatedPadding(
                  duration: const Duration(milliseconds: 600),
                  curve: Curves.easeInOutCubic,
                  padding: EdgeInsets.symmetric(
                    horizontal: hasLyrics ? 32 : (maxWidth * 0.05),
                  ),
                  child: SyncedTransformSwitcher(
                    slideOffset: slideOffset,
                    moveScale: coverSize,
                    duration: const Duration(milliseconds: 550),
                    child: Padding(
                      key: ValueKey('track-$trackId'),
                      padding: const EdgeInsets.all(24),
                      child: SizedBox(
                        width:
                            (hasLyrics ? (maxWidth / 2) : maxWidth) -
                            (hasLyrics ? 64 : (maxWidth * 0.1)) -
                            48.0,
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            _CoverImage(
                              coverDir: coverDir,
                              trackId: trackId,
                              size: coverSize,
                            ),
                            SizedBox(
                              height: (coverSize / 12).clamp(16.0, 40.0),
                            ),
                            ConstrainedBox(
                              constraints: BoxConstraints(
                                minHeight: (coverSize / 2.5).clamp(
                                  110.0,
                                  220.0,
                                ),
                              ),
                              child: Column(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  MarqueeText(
                                    text: title,
                                    style: textStyle,
                                    textAlign: TextAlign.center,
                                  ),
                                  if (subtitle.isNotEmpty) ...[
                                    SizedBox(
                                      height: (coverSize / 40).clamp(4.0, 12.0),
                                    ),
                                    Row(
                                      mainAxisSize: MainAxisSize.min,
                                      children: [
                                        if (currentPath != null) ...[
                                          AudioFormatBadge(
                                            path: currentPath!,
                                            sampleRate: sampleRate,
                                          ),
                                          const SizedBox(width: 4),
                                        ],
                                        Flexible(
                                          child: MarqueeText(
                                            text: subtitle,
                                            style: subtitleStyle,
                                            textAlign: TextAlign.center,
                                          ),
                                        ),
                                      ],
                                    ),
                                  ],
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
        // Right side: Lyrics area
        ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: (maxWidth - 400).clamp(0.0, maxWidth / 2),
          ),
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 600),
            curve: Curves.easeInOutCubic,
            width: hasLyrics ? (maxWidth / 2) : 0,
            child: ClipRect(
              child: OverflowBox(
                minWidth: maxWidth / 2,
                maxWidth: maxWidth / 2,
                alignment: Alignment.center,
                child: AnimatedOpacity(
                  duration: const Duration(milliseconds: 400),
                  opacity: hasLyrics ? 1.0 : 0.0,
                  child: Center(
                    child: Text(
                      l10n.noLyrics,
                      style: theme.textTheme.titleLarge?.copyWith(
                        color: foregroundColor.withValues(alpha: 0.6),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// Narrow layout: stacked vertically.
class _NarrowLayout extends StatelessWidget {
  const _NarrowLayout({
    required this.coverDir,
    required this.trackId,
    required this.title,
    required this.subtitle,
    required this.slideDirection,
    required this.foregroundColor,
    this.currentPath,
    this.sampleRate,
    required this.maxHeight,
    required this.hasLyrics,
  });

  final String coverDir;
  final int? trackId;
  final String title;
  final String subtitle;
  final int slideDirection; // 1 = forward, -1 = backward, 0 = initial
  final Color foregroundColor;
  final String? currentPath;
  final int? sampleRate;
  final double maxHeight;
  final bool hasLyrics;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);

    // Dynamic scaling for narrow layout
    // Cover size mostly determined by height, but capped by width
    // Use MediaQuery as fallback or just use passed maxHeight if available
    final coverSize = (maxHeight * 0.45).clamp(240.0, 500.0);

    final titleFontSize = (coverSize / 12).clamp(20.0, 42.0);
    final subtitleFontSize = (coverSize / 18).clamp(14.0, 20.0);

    final textStyle = theme.textTheme.headlineSmall?.copyWith(
      fontWeight: FontWeight.bold,
      color: foregroundColor,
      fontSize: titleFontSize,
    );
    final subtitleStyle = theme.textTheme.bodyLarge?.copyWith(
      color: foregroundColor.withValues(alpha: 0.7),
      fontSize: subtitleFontSize,
    );

    // Slide offset for the carousel effect
    final slideOffset = slideDirection >= 0 ? 1.05 : -1.05;

    return ShaderMask(
      shaderCallback: (Rect bounds) {
        return const LinearGradient(
          begin: Alignment.centerLeft,
          end: Alignment.centerRight,
          colors: [
            Colors.transparent,
            Colors.black,
            Colors.black,
            Colors.transparent,
          ],
          stops: [0.0, 0.08, 0.92, 1.0],
        ).createShader(bounds);
      },
      blendMode: BlendMode.dstIn,
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(24),
        child: SyncedTransformSwitcher(
          slideOffset: slideOffset,
          moveScale: coverSize,
          duration: const Duration(milliseconds: 550),
          child: SizedBox(
            width: (MediaQuery.sizeOf(context).width - 48.0),
            child: Column(
              key: ValueKey('track-$trackId'),
              children: [
                _CoverImage(
                  coverDir: coverDir,
                  trackId: trackId,
                  size: coverSize,
                ),
                SizedBox(height: (coverSize / 12).clamp(24.0, 48.0)),
                // Constant height container for text to prevent jitter
                // Use ConstrainedBox with minHeight to avoid overflow
                ConstrainedBox(
                  constraints: BoxConstraints(
                    minHeight: (coverSize * 0.45).clamp(110.0, 180.0),
                  ),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      MarqueeText(
                        text: title,
                        style: textStyle,
                        textAlign: TextAlign.center,
                      ),
                      if (subtitle.isNotEmpty) ...[
                        SizedBox(height: (coverSize / 40).clamp(6.0, 12.0)),
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            if (currentPath != null) ...[
                              AudioFormatBadge(
                                path: currentPath!,
                                sampleRate: sampleRate,
                              ),
                              const SizedBox(width: 4),
                            ],
                            Flexible(
                              child: MarqueeText(
                                text: subtitle,
                                style: subtitleStyle,
                                textAlign: TextAlign.center,
                              ),
                            ),
                          ],
                        ),
                      ],
                    ],
                  ),
                ),
                if (hasLyrics) ...[
                  SizedBox(height: (coverSize / 8).clamp(32.0, 64.0)),
                  Text(
                    l10n.noLyrics,
                    style: theme.textTheme.titleLarge?.copyWith(
                      color: foregroundColor.withValues(alpha: 0.6),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Bottom playback bar with time, controls, progress slider, and volume.
class _BottomPlaybackBar extends StatefulWidget {
  const _BottomPlaybackBar({
    required this.positionMs,
    required this.durationMs,
    required this.isPlaying,
    required this.playMode,
    required this.volume,
    required this.onPlayPause,
    required this.onPrevious,
    required this.onNext,
    required this.onSeek,
    required this.onVolumeChanged,
    required this.onPlayModeChanged,
    required this.foregroundColor,
    this.currentPath,
    this.sampleRate,
  });

  final int positionMs;
  final int durationMs;
  final bool isPlaying;
  final PlayMode playMode;
  final double volume;
  final VoidCallback onPlayPause;
  final VoidCallback onPrevious;
  final VoidCallback onNext;
  final ValueChanged<int> onSeek;
  final ValueChanged<double> onVolumeChanged;
  final VoidCallback onPlayModeChanged;
  final Color foregroundColor;
  final String? currentPath;
  final int? sampleRate;

  @override
  State<_BottomPlaybackBar> createState() => _BottomPlaybackBarState();
}

class _BottomPlaybackBarState extends State<_BottomPlaybackBar> {
  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context)!;

    final playModeIcon = switch (widget.playMode) {
      PlayMode.sequential => Icons.arrow_forward,
      PlayMode.shuffle => Icons.shuffle,
      PlayMode.repeatAll => Icons.repeat,
      PlayMode.repeatOne => Icons.repeat_one,
    };

    final playModeTooltip = switch (widget.playMode) {
      PlayMode.sequential => l10n.playModeSequential,
      PlayMode.shuffle => l10n.playModeShuffle,
      PlayMode.repeatAll => l10n.playModeRepeatAll,
      PlayMode.repeatOne => l10n.playModeRepeatOne,
    };

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        // Progress bar
        _DetailProgressBar(
          positionMs: widget.positionMs,
          durationMs: widget.durationMs,
          isPlaying: widget.isPlaying,
          onSeek: widget.onSeek,
          foregroundColor: widget.foregroundColor,
        ),
        // Controls row
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: Row(
            children: [
              // Left: Time display
              SizedBox(
                width: 60,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      _formatMs(widget.positionMs),
                      style: theme.textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w500,
                        color: widget.foregroundColor,
                      ),
                    ),
                    Text(
                      _formatMs(widget.durationMs),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: widget.foregroundColor.withValues(alpha: 0.6),
                      ),
                    ),
                  ],
                ),
              ),
              // Center: Playback controls
              Expanded(
                child: FittedBox(
                  fit: BoxFit.scaleDown,
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      IconButton(
                        icon: Icon(
                          Icons.equalizer,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 24,
                        onPressed: () {
                          // TODO: Equalizer
                        },
                      ),
                      // Volume button with popup
                      _VolumeButton(
                        volume: widget.volume,
                        onChanged: widget.onVolumeChanged,
                        foregroundColor: widget.foregroundColor,
                      ),
                      const SizedBox(width: 8),
                      IconButton(
                        icon: Icon(
                          Icons.skip_previous,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 32,
                        tooltip: l10n.tooltipPrevious,
                        onPressed: widget.onPrevious,
                      ),
                      const SizedBox(width: 4),
                      IconButton(
                        icon: Icon(
                          widget.isPlaying ? Icons.pause : Icons.play_arrow,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 40,
                        tooltip: widget.isPlaying ? l10n.pause : l10n.play,
                        onPressed: widget.onPlayPause,
                      ),
                      const SizedBox(width: 4),
                      IconButton(
                        icon: Icon(
                          Icons.skip_next,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 32,
                        tooltip: l10n.tooltipNext,
                        onPressed: widget.onNext,
                      ),
                      const SizedBox(width: 8),
                      IconButton(
                        icon: Icon(playModeIcon, color: widget.foregroundColor),
                        iconSize: 24,
                        tooltip: playModeTooltip,
                        onPressed: widget.onPlayModeChanged,
                      ),
                      IconButton(
                        icon: Icon(Icons.menu, color: widget.foregroundColor),
                        iconSize: 24,
                        onPressed: () {
                          // TODO: Queue menu
                        },
                      ),
                    ],
                  ),
                ),
              ),
              // Right: Placeholder for additional controls
              const SizedBox(width: 60),
            ],
          ),
        ),
        const SizedBox(height: 8),
      ],
    );
  }

  String _formatMs(int ms) {
    final totalSeconds = ms ~/ 1000;
    final minutes = totalSeconds ~/ 60;
    final seconds = totalSeconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }
}

/// Cover image with placeholder fallback.
class _CoverImage extends StatelessWidget {
  const _CoverImage({
    required this.coverDir,
    required this.trackId,
    required this.size,
  });

  final String coverDir;
  final int? trackId;
  final double size;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    final placeholder = Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(16),
        color: theme.colorScheme.primary.withValues(alpha: 0.10),
        border: Border.all(
          color: theme.colorScheme.primary.withValues(alpha: 0.18),
        ),
        boxShadow: [
          BoxShadow(
            color: theme.shadowColor.withValues(alpha: 0.15),
            blurRadius: 24,
            offset: const Offset(0, 8),
          ),
        ],
      ),
      child: Icon(
        Icons.music_note,
        size: size * 0.4,
        color: theme.colorScheme.primary,
      ),
    );

    if (trackId == null) return placeholder;

    final coverPath = '$coverDir${Platform.pathSeparator}$trackId';
    final provider = ResizeImage(
      FileImage(File(coverPath)),
      width: (size * 2).toInt(),
      height: (size * 2).toInt(),
      allowUpscaling: false,
    );

    return Container(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(16),
        boxShadow: [
          BoxShadow(
            color: theme.shadowColor.withValues(alpha: 0.2),
            blurRadius: 32,
            offset: const Offset(0, 12),
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(16),
        child: Image(
          image: provider,
          width: size,
          height: size,
          fit: BoxFit.cover,
          gaplessPlayback: true,
          errorBuilder: (context, error, stackTrace) => placeholder,
        ),
      ),
    );
  }
}

/// Volume button with popup slider.
class _VolumeButton extends StatefulWidget {
  const _VolumeButton({
    required this.volume,
    required this.onChanged,
    required this.foregroundColor,
  });

  final double volume;
  final ValueChanged<double> onChanged;
  final Color foregroundColor;

  @override
  State<_VolumeButton> createState() => _VolumeButtonState();
}

class _VolumeButtonState extends State<_VolumeButton> {
  OverlayEntry? _overlayEntry;
  final LayerLink _layerLink = LayerLink();

  @override
  void dispose() {
    _removeOverlay();
    super.dispose();
  }

  void _removeOverlay() {
    _overlayEntry?.remove();
    _overlayEntry = null;
  }

  void _toggleOverlay() {
    if (_overlayEntry != null) {
      _removeOverlay();
      return;
    }

    final overlay = Overlay.maybeOf(context);
    if (overlay == null) return;

    _overlayEntry = OverlayEntry(
      builder: (context) {
        final theme = Theme.of(context);
        return Stack(
          children: [
            // Tap outside to close
            Positioned.fill(
              child: GestureDetector(
                behavior: HitTestBehavior.translucent,
                onTap: _removeOverlay,
                child: Container(color: Colors.transparent),
              ),
            ),
            // Volume popup
            Positioned(
              width: 48,
              height: 160,
              child: CompositedTransformFollower(
                link: _layerLink,
                targetAnchor: Alignment.topCenter,
                followerAnchor: Alignment.bottomCenter,
                offset: const Offset(0, -8),
                child: Material(
                  elevation: 4,
                  borderRadius: BorderRadius.circular(12),
                  color: theme.colorScheme.surfaceContainerHigh,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(vertical: 12),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          '${(widget.volume * 100).round()}%',
                          style: theme.textTheme.labelSmall,
                        ),
                        Expanded(
                          child: RotatedBox(
                            quarterTurns: -1,
                            child: SliderTheme(
                              data: SliderThemeData(
                                trackHeight: 4,
                                thumbShape: const RoundSliderThumbShape(
                                  enabledThumbRadius: 6,
                                ),
                                overlayShape: const RoundSliderOverlayShape(
                                  overlayRadius: 12,
                                ),
                                activeTrackColor: theme.colorScheme.primary,
                                inactiveTrackColor: theme.colorScheme.primary
                                    .withValues(alpha: 0.2),
                                thumbColor: theme.colorScheme.primary,
                              ),
                              child: Slider(
                                value: widget.volume.clamp(0.0, 1.0),
                                onChanged: (v) {
                                  widget.onChanged(v);
                                  _overlayEntry?.markNeedsBuild();
                                },
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ],
        );
      },
    );
    overlay.insert(_overlayEntry!);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final iconData = widget.volume <= 0
        ? Icons.volume_off
        : widget.volume < 0.5
        ? Icons.volume_down
        : Icons.volume_up;

    return CompositedTransformTarget(
      link: _layerLink,
      child: IconButton(
        icon: Icon(iconData, color: widget.foregroundColor),
        iconSize: 24,
        tooltip: l10n.volume,
        onPressed: _toggleOverlay,
      ),
    );
  }
}

/// Custom progress bar with smooth animation, hover effect, and no thumb.
class _DetailProgressBar extends StatefulWidget {
  const _DetailProgressBar({
    required this.positionMs,
    required this.durationMs,
    required this.isPlaying,
    required this.onSeek,
    required this.foregroundColor,
  });

  final int positionMs;
  final int durationMs;
  final bool isPlaying;
  final ValueChanged<int> onSeek;
  final Color foregroundColor;

  @override
  State<_DetailProgressBar> createState() => _DetailProgressBarState();
}

class _DetailProgressBarState extends State<_DetailProgressBar>
    with TickerProviderStateMixin {
  late final AnimationController _controller;
  late final Ticker _ticker;
  bool _dragging = false;
  bool _hovering = false;
  DateTime? _baseAt;
  int _basePosMs = 0;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      lowerBound: 0,
      upperBound: 1,
      duration: const Duration(milliseconds: 180),
    );
    _controller.value = _targetValue();
    _basePosMs = widget.positionMs;
    _baseAt = DateTime.now();
    _ticker = createTicker((_) {
      if (!_shouldTick) return;
      final d = widget.durationMs;
      if (d <= 0) return;
      final at = _baseAt;
      if (at == null) return;
      final elapsed = DateTime.now().difference(at).inMilliseconds;
      final pos = (_basePosMs + elapsed).clamp(0, d);
      _controller.value = (pos / d).clamp(0.0, 1.0);
    });
    _syncTicker();
  }

  @override
  void didUpdateWidget(covariant _DetailProgressBar oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_dragging) return;

    final now = DateTime.now();
    final durationMs = widget.durationMs;

    if (oldWidget.positionMs != widget.positionMs ||
        oldWidget.durationMs != widget.durationMs ||
        oldWidget.isPlaying != widget.isPlaying) {
      _basePosMs = widget.positionMs;
      _baseAt = now;
    }

    _syncTicker();

    if (!_shouldTick) {
      _controller.animateTo(_targetValue(), curve: Curves.easeOutCubic);
    } else if (durationMs > 0) {
      final elapsed = _baseAt != null
          ? now.difference(_baseAt!).inMilliseconds
          : 0;
      final pos = (_basePosMs + elapsed).clamp(0, durationMs);
      _controller.value = (pos / durationMs).clamp(0.0, 1.0);
    }
  }

  bool get _shouldTick {
    if (_dragging) return false;
    if (widget.durationMs <= 0) return false;
    return widget.isPlaying;
  }

  void _syncTicker() {
    if (_shouldTick) {
      if (!_ticker.isActive) _ticker.start();
    } else {
      if (_ticker.isActive) _ticker.stop();
    }
  }

  double _targetValue() {
    final d = widget.durationMs;
    if (d <= 0) return 0;
    return (widget.positionMs / d).clamp(0.0, 1.0);
  }

  double _posToFraction(Offset local, double width) {
    if (width <= 0) return 0;
    return (local.dx / width).clamp(0.0, 1.0);
  }

  int _fractionToMs(double fraction) {
    final d = widget.durationMs;
    if (d <= 0) return 0;
    return (fraction * d).round().clamp(0, d);
  }

  bool get _emphasized => _hovering || _dragging;

  @override
  void dispose() {
    _ticker.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final d = widget.durationMs;

    final trackHeightTarget = _emphasized ? 6.0 : 4.0;
    const double barHeight = 16.0; // Fixed height to avoid layout shift

    return SizedBox(
      height: barHeight,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final w = constraints.maxWidth;

          return MouseRegion(
            cursor: d > 0 ? SystemMouseCursors.click : SystemMouseCursors.basic,
            onEnter: (_) => setState(() => _hovering = true),
            onExit: (_) => setState(() => _hovering = false),
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onTapDown: d > 0
                  ? (details) {
                      final frac = _posToFraction(details.localPosition, w);
                      final ms = _fractionToMs(frac);
                      _controller.stop();
                      _controller.value = frac;
                      _basePosMs = ms;
                      _baseAt = DateTime.now();
                      widget.onSeek(ms);
                    }
                  : null,
              onHorizontalDragStart: d > 0
                  ? (details) {
                      setState(() => _dragging = true);
                      _syncTicker();
                      final frac = _posToFraction(details.localPosition, w);
                      _controller.stop();
                      _controller.value = frac;
                    }
                  : null,
              onHorizontalDragUpdate: d > 0
                  ? (details) {
                      final frac = _posToFraction(details.localPosition, w);
                      _controller.value = frac;
                    }
                  : null,
              onHorizontalDragEnd: d > 0
                  ? (_) {
                      final ms = _fractionToMs(_controller.value);
                      setState(() => _dragging = false);
                      _basePosMs = ms;
                      _baseAt = DateTime.now();
                      _syncTicker();
                      widget.onSeek(ms);
                    }
                  : null,
              onHorizontalDragCancel: () {
                if (!_dragging) return;
                setState(() => _dragging = false);
                _syncTicker();
                _controller.animateTo(
                  _targetValue(),
                  curve: Curves.easeOutCubic,
                );
              },
              child: TweenAnimationBuilder<double>(
                duration: const Duration(milliseconds: 140),
                curve: Curves.easeOutCubic,
                tween: Tween<double>(end: trackHeightTarget),
                builder: (context, trackH, _) {
                  return AnimatedBuilder(
                    animation: _controller,
                    builder: (context, _) {
                      return CustomPaint(
                        size: Size(w, barHeight),
                        painter: _DetailProgressPainter(
                          progress: _controller.value,
                          trackHeight: trackH,
                          centerY: barHeight / 2,
                          trackColor: widget.foregroundColor.withValues(
                            alpha: 0.15,
                          ),
                          fillColor: widget.foregroundColor,
                        ),
                      );
                    },
                  );
                },
              ),
            ),
          );
        },
      ),
    );
  }
}

class _DetailProgressPainter extends CustomPainter {
  const _DetailProgressPainter({
    required this.progress,
    required this.trackHeight,
    required this.centerY,
    required this.trackColor,
    required this.fillColor,
  });

  final double progress;
  final double trackHeight;
  final double centerY;
  final Color trackColor;
  final Color fillColor;

  @override
  void paint(Canvas canvas, Size size) {
    final trackH = trackHeight;
    final maxY = (size.height - trackH).clamp(0.0, size.height);
    final y = (centerY - trackH / 2).clamp(0.0, maxY);
    final trackRect = Rect.fromLTWH(0, y, size.width, trackH);
    canvas.drawRect(trackRect, Paint()..color = trackColor);

    final w = (size.width * progress).clamp(0.0, size.width);
    if (w <= 0) return;
    final fillRect = Rect.fromLTWH(0, y, w, trackH);
    canvas.drawRect(fillRect, Paint()..color = fillColor);
  }

  @override
  bool shouldRepaint(covariant _DetailProgressPainter oldDelegate) {
    return oldDelegate.progress != progress ||
        oldDelegate.trackHeight != trackHeight ||
        oldDelegate.centerY != centerY ||
        oldDelegate.trackColor != trackColor ||
        oldDelegate.fillColor != fillColor;
  }
}

/// A switcher that synchronizes the movement of two children to create
/// a perfectly coordinated carousel/film-strip effect.
class SyncedTransformSwitcher extends StatefulWidget {
  const SyncedTransformSwitcher({
    super.key,
    required this.child,
    required this.slideOffset, // e.g. 1.2 or -1.2
    required this.moveScale,
    this.duration = const Duration(milliseconds: 550),
  });

  final Widget child;
  final double slideOffset;
  final double moveScale;
  final Duration duration;

  @override
  State<SyncedTransformSwitcher> createState() =>
      _SyncedTransformSwitcherState();
}

class _SyncedTransformSwitcherState extends State<SyncedTransformSwitcher>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  late Animation<double> _animation;
  Widget? _lastChild;
  double _lastSlideOffset = 0;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(vsync: this, duration: widget.duration);

    // Physics Model: Accelerating Collision
    // 1. Accelerate towards target (0.0 -> 0.50)
    // 2. High-velocity impact & overshoot (0.50 -> 0.65)
    // 3. Gradual snap back to center (0.65 -> 1.0)
    _animation = TweenSequence<double>([
      // 0.0 -> 0.50: Accelerating approach (easeIn for a punchy start)
      TweenSequenceItem(
        tween: Tween<double>(
          begin: 0.0,
          end: 1.0,
        ).chain(CurveTween(curve: Curves.easeIn)),
        weight: 50,
      ),
      // 0.50 -> 0.65: Momentum carry-over (Overshoot to 1.08)
      TweenSequenceItem(
        tween: Tween<double>(
          begin: 1.0,
          end: 1.08,
        ).chain(CurveTween(curve: Curves.easeOutCubic)),
        weight: 15,
      ),
      // 0.65 -> 1.0: Slower, viscous snap-back
      TweenSequenceItem(
        tween: Tween<double>(
          begin: 1.08,
          end: 1.0,
        ).chain(CurveTween(curve: Curves.easeOutBack)),
        weight: 35,
      ),
    ]).animate(_controller);

    // Initial state is finished
    _controller.value = 1.0;
  }

  @override
  void didUpdateWidget(SyncedTransformSwitcher oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.child.key != oldWidget.child.key) {
      _lastChild = oldWidget.child;
      _lastSlideOffset = widget.slideOffset;
      _controller.forward(from: 0.0);
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        final controllerValue = _controller.value;
        final animValue = _animation.value;
        final isFinished = controllerValue >= 1.0;
        final moveScale = widget.moveScale;

        // Incoming child follows the main animation sequence (glide + snap)
        final incomingMoveValue = 1.0 - animValue;
        final incomingOpacity = (controllerValue / 0.3).clamp(0.0, 1.0);

        // Outgoing child follows the glide until impact at 50%, then "launched"
        double outgoingMoveValue;
        if (controllerValue <= 0.50) {
          outgoingMoveValue = animValue;
        } else {
          // Explosive momentum transfer at 0.50
          final t = ((controllerValue - 0.50) / 0.50).clamp(0.0, 1.0);
          outgoingMoveValue = 1.0 + Curves.easeInExpo.transform(t) * 4.5;
        }

        return Stack(
          clipBehavior: Clip.none,
          alignment: Alignment.center,
          children: [
            // Outgoing child
            if (!isFinished && _lastChild != null)
              Transform.translate(
                offset: Offset(
                  -_lastSlideOffset * outgoingMoveValue * moveScale,
                  0,
                ),
                child: _lastChild,
              ),
            // Incoming child
            Transform.translate(
              offset: Offset(
                _lastSlideOffset * incomingMoveValue * moveScale,
                0,
              ),
              child: Opacity(opacity: incomingOpacity, child: widget.child),
            ),
          ],
        );
      },
    );
  }
}
