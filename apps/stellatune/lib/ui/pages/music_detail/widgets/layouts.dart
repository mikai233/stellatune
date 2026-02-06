import 'dart:math';

import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart'; // Check if I need this? Yes for noLyrics text.
import 'package:stellatune/ui/widgets/audio_format_badge.dart';
import 'package:stellatune/ui/widgets/marquee_text.dart';

import 'cover_image.dart';

/// Wide layout: cover + info on left, lyrics on right.
class WideLayout extends StatelessWidget {
  const WideLayout({
    super.key,
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
                stops: [0.0, 0.06, 0.94, 1.0],
              ).createShader(bounds);
            },
            blendMode: BlendMode.dstIn,
            child: ClipRect(
              clipBehavior: Clip.antiAlias,
              child: OverflowBox(
                minWidth: 400,
                maxWidth: double.infinity,
                alignment: Alignment.center,
                child: AnimatedPadding(
                  duration: const Duration(milliseconds: 800),
                  curve: Curves.easeOutQuart,
                  padding: EdgeInsets.symmetric(
                    horizontal: hasLyrics ? 32 : (maxWidth * 0.05),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.all(24),
                    child: SizedBox(
                      width:
                          (hasLyrics ? (maxWidth / 2) : maxWidth) -
                          (hasLyrics ? 64 : (maxWidth * 0.1)) -
                          48.0,
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          SyncedTransformSwitcher(
                            slideOffset: slideOffset,
                            moveScale: coverSize,
                            duration: const Duration(milliseconds: 550),
                            crossFade: false,
                            child: CoverImage(
                              key: ValueKey('cover-$trackId'),
                              coverDir: coverDir,
                              trackId: trackId,
                              size: coverSize,
                            ),
                          ),
                          SizedBox(height: (coverSize / 12).clamp(16.0, 40.0)),
                          SyncedTransformSwitcher(
                            slideOffset: slideOffset,
                            moveScale: coverSize,
                            duration: const Duration(milliseconds: 550),
                            crossFade: true,
                            child: ConstrainedBox(
                              key: ValueKey('text-$trackId'),
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
        // Right side: Lyrics area
        ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: (maxWidth - 400).clamp(0.0, maxWidth / 2),
          ),
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 800),
            curve: Curves.easeOutQuart,
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
                      l10n.noLyrics, // Using l10n
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
class NarrowLayout extends StatelessWidget {
  const NarrowLayout({
    super.key,
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
          stops: [0.0, 0.06, 0.94, 1.0],
        ).createShader(bounds);
      },
      blendMode: BlendMode.dstIn,
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(24),
        child: SizedBox(
          width: (MediaQuery.sizeOf(context).width - 48.0),
          child: Column(
            children: [
              SyncedTransformSwitcher(
                slideOffset: slideOffset,
                moveScale: coverSize,
                duration: const Duration(milliseconds: 550),
                crossFade: false,
                child: CoverImage(
                  key: ValueKey('cover-$trackId'),
                  coverDir: coverDir,
                  trackId: trackId,
                  size: coverSize,
                ),
              ),
              SizedBox(height: (coverSize / 12).clamp(24.0, 48.0)),
              SyncedTransformSwitcher(
                slideOffset: slideOffset,
                moveScale: coverSize,
                duration: const Duration(milliseconds: 550),
                crossFade: true,
                child: ConstrainedBox(
                  key: ValueKey('text-$trackId'),
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
    );
  }
}
