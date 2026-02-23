import 'dart:io';
import 'dart:ui' show ImageFilter;

import 'package:flutter/material.dart';
import 'package:stellatune/ui/widgets/now_playing_common.dart';

class HomeHeroCard extends StatelessWidget {
  static const double _heroCoverInset = 12;
  static const double _heroCoverSize = 176;

  const HomeHeroCard({
    super.key,
    required this.heading,
    required this.title,
    required this.subtitle,
    required this.durationMs,
    required this.trackId,
    required this.coverDir,
    required this.resumeLabel,
    required this.lyricsLabel,
    required this.onResume,
    required this.onLyrics,
  });

  final String heading;
  final String title;
  final String subtitle;
  final int? durationMs;
  final int? trackId;
  final String coverDir;
  final String resumeLabel;
  final String lyricsLabel;
  final VoidCallback? onResume;
  final VoidCallback onLyrics;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SizedBox(
      width: double.infinity,
      child: Container(
        constraints: const BoxConstraints(minHeight: 212),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(24),
          boxShadow: [
            BoxShadow(
              color: theme.colorScheme.primary.withValues(alpha: 0.12),
              blurRadius: 26,
              offset: const Offset(0, 8),
            ),
          ],
        ),
        clipBehavior: Clip.antiAlias,
        child: LayoutBuilder(
          builder: (context, constraints) {
            final wide = constraints.maxWidth >= 760;
            final contentRightPadding = wide
                ? _heroCoverInset + _heroCoverSize + 22
                : (_heroCoverInset + _heroCoverSize * 0.52).clamp(84.0, 126.0);

            final textArea = Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  heading,
                  style: theme.textTheme.titleLarge?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 12),
                Text(
                  title,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.headlineSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 6),
                Text(
                  subtitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.bodyLarge?.copyWith(
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.78),
                  ),
                ),
                if (durationMs != null) ...[
                  const SizedBox(height: 4),
                  Text(
                    NowPlayingCommon.formatMs(durationMs!),
                    style: theme.textTheme.bodySmall,
                  ),
                ],
                const SizedBox(height: 16),
                Wrap(
                  spacing: 10,
                  runSpacing: 8,
                  children: [
                    FilledButton.tonalIcon(
                      onPressed: onResume,
                      icon: const Icon(Icons.play_arrow),
                      label: Text(resumeLabel),
                    ),
                    OutlinedButton.icon(
                      onPressed: onLyrics,
                      icon: const Icon(Icons.lyrics_outlined),
                      label: Text(lyricsLabel),
                    ),
                  ],
                ),
              ],
            );

            return ConstrainedBox(
              constraints: const BoxConstraints(minHeight: 212),
              child: Stack(
                children: [
                  Positioned.fill(
                    child: BackdropFilter(
                      filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
                      child: ColoredBox(
                        color: theme.colorScheme.surface.withValues(
                          alpha: 0.14,
                        ),
                      ),
                    ),
                  ),
                  Positioned.fill(
                    child: HomeHeroVisualLayer(
                      trackId: trackId,
                      coverDir: coverDir,
                      coverSize: _heroCoverSize,
                      coverInset: _heroCoverInset,
                      coverOpacity: wide ? 1.0 : 0.76,
                    ),
                  ),
                  Positioned.fill(
                    child: IgnorePointer(
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          gradient: LinearGradient(
                            begin: Alignment.topLeft,
                            end: Alignment.bottomRight,
                            colors: [
                              theme.colorScheme.surface.withValues(alpha: 0.12),
                              theme.colorScheme.surface.withValues(alpha: 0.02),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                  Padding(
                    padding: EdgeInsets.fromLTRB(
                      18,
                      18,
                      contentRightPadding,
                      18,
                    ),
                    child: textArea,
                  ),
                ],
              ),
            );
          },
        ),
      ),
    );
  }
}

class HomeHeroVisualLayer extends StatelessWidget {
  const HomeHeroVisualLayer({
    super.key,
    required this.trackId,
    required this.coverDir,
    required this.coverSize,
    required this.coverInset,
    required this.coverOpacity,
  });

  final int? trackId;
  final String coverDir;
  final double coverSize;
  final double coverInset;
  final double coverOpacity;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Stack(
      children: [
        Positioned.fill(
          child: DecoratedBox(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [
                  Color.alphaBlend(
                    theme.colorScheme.primary.withValues(alpha: 0.22),
                    theme.colorScheme.surfaceContainerHigh,
                  ),
                  Color.alphaBlend(
                    theme.colorScheme.secondary.withValues(alpha: 0.15),
                    theme.colorScheme.surfaceContainer,
                  ),
                ],
              ),
            ),
          ),
        ),
        Positioned(
          right: coverInset,
          top: coverInset,
          width: coverSize,
          height: coverSize,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(18),
            child: Opacity(
              opacity: coverOpacity,
              child: HomeCoverImage(trackId: trackId, coverDir: coverDir),
            ),
          ),
        ),
        Positioned.fill(
          child: IgnorePointer(
            child: DecoratedBox(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.centerLeft,
                  end: Alignment.centerRight,
                  stops: const [0.0, 0.68, 1.0],
                  colors: [
                    theme.colorScheme.surface.withValues(alpha: 0.26),
                    theme.colorScheme.surface.withValues(alpha: 0.12),
                    Colors.transparent,
                  ],
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class HomeContinueListeningSquareCard extends StatelessWidget {
  const HomeContinueListeningSquareCard({
    super.key,
    required this.size,
    required this.trackId,
    required this.coverDir,
    required this.title,
    required this.subtitle,
    required this.durationMs,
    required this.onTap,
  });

  final double size;
  final int? trackId;
  final String coverDir;
  final String title;
  final String subtitle;
  final int? durationMs;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return HomeHoverLift(
      child: SizedBox(
        width: size,
        height: size,
        child: Material(
          color: Colors.transparent,
          child: InkWell(
            borderRadius: BorderRadius.circular(16),
            onTap: onTap,
            child: Ink(
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(16),
              ),
              child: ClipRRect(
                borderRadius: BorderRadius.circular(16),
                child: Stack(
                  children: [
                    Positioned.fill(
                      child: HomeCoverImage(
                        trackId: trackId,
                        coverDir: coverDir,
                      ),
                    ),
                    Positioned.fill(
                      child: IgnorePointer(
                        child: DecoratedBox(
                          decoration: BoxDecoration(
                            gradient: LinearGradient(
                              begin: Alignment.topCenter,
                              end: Alignment.bottomCenter,
                              stops: const [0.56, 0.82, 1.0],
                              colors: [
                                Colors.transparent,
                                Color.alphaBlend(
                                  theme.colorScheme.primary.withValues(
                                    alpha: 0.06,
                                  ),
                                  Colors.black.withValues(alpha: 0.08),
                                ),
                                Color.alphaBlend(
                                  theme.colorScheme.secondary.withValues(
                                    alpha: 0.10,
                                  ),
                                  Colors.black.withValues(alpha: 0.24),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ),
                    ),
                    Positioned(
                      left: 0,
                      right: 0,
                      bottom: 0,
                      height: 104,
                      child: ClipRect(
                        child: Stack(
                          children: [
                            Positioned.fill(
                              child: ShaderMask(
                                shaderCallback: (rect) {
                                  return const LinearGradient(
                                    begin: Alignment.topCenter,
                                    end: Alignment.bottomCenter,
                                    stops: [0.0, 0.38, 1.0],
                                    colors: [
                                      Colors.transparent,
                                      Color(0xB3FFFFFF),
                                      Colors.white,
                                    ],
                                  ).createShader(rect);
                                },
                                blendMode: BlendMode.dstIn,
                                child: ImageFiltered(
                                  imageFilter: ImageFilter.blur(
                                    sigmaX: 11,
                                    sigmaY: 11,
                                  ),
                                  child: HomeCoverImage(
                                    trackId: trackId,
                                    coverDir: coverDir,
                                  ),
                                ),
                              ),
                            ),
                            Positioned.fill(
                              child: DecoratedBox(
                                decoration: BoxDecoration(
                                  gradient: LinearGradient(
                                    begin: Alignment.topCenter,
                                    end: Alignment.bottomCenter,
                                    colors: [
                                      Colors.black.withValues(alpha: 0.00),
                                      Color.alphaBlend(
                                        theme.colorScheme.primary.withValues(
                                          alpha: 0.08,
                                        ),
                                        Colors.black.withValues(alpha: 0.20),
                                      ),
                                      Color.alphaBlend(
                                        theme.colorScheme.secondary.withValues(
                                          alpha: 0.10,
                                        ),
                                        Colors.black.withValues(alpha: 0.30),
                                      ),
                                    ],
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
                      right: 0,
                      bottom: 0,
                      height: 74,
                      child: Padding(
                        padding: const EdgeInsets.fromLTRB(10, 8, 10, 8),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisAlignment: MainAxisAlignment.end,
                          children: [
                            Text(
                              title,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.bodyMedium?.copyWith(
                                fontWeight: FontWeight.w700,
                                color: Colors.white.withValues(alpha: 0.96),
                              ),
                            ),
                            const SizedBox(height: 2),
                            Text(
                              subtitle,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: Colors.white.withValues(alpha: 0.86),
                              ),
                            ),
                            if (durationMs != null) ...[
                              const SizedBox(height: 2),
                              Text(
                                NowPlayingCommon.formatMs(durationMs!),
                                style: theme.textTheme.labelSmall?.copyWith(
                                  color: Colors.white.withValues(alpha: 0.82),
                                ),
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
    );
  }
}

class HomeSlotSwapTransition extends StatelessWidget {
  const HomeSlotSwapTransition({
    super.key,
    required this.childKey,
    required this.child,
  });

  final Key childKey;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return AnimatedSwitcher(
      duration: const Duration(milliseconds: 300),
      reverseDuration: const Duration(milliseconds: 180),
      switchInCurve: const Cubic(0.20, 0.85, 0.25, 1.0),
      switchOutCurve: const Cubic(0.45, 0.0, 0.75, 0.3),
      layoutBuilder: (currentChild, previousChildren) {
        return Stack(
          fit: StackFit.passthrough,
          children: [...previousChildren, ..._singleOrEmpty(currentChild)],
        );
      },
      transitionBuilder: (child, animation) {
        final slide = Tween<Offset>(
          begin: const Offset(0.05, 0.0),
          end: Offset.zero,
        ).animate(animation);
        return FadeTransition(
          opacity: animation,
          child: SlideTransition(position: slide, child: child),
        );
      },
      child: KeyedSubtree(key: childKey, child: child),
    );
  }
}

class HomeHoverLift extends StatefulWidget {
  const HomeHoverLift({super.key, required this.child});

  final Widget child;

  @override
  State<HomeHoverLift> createState() => _HomeHoverLiftState();
}

class _HomeHoverLiftState extends State<HomeHoverLift>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  late final Animation<double> _depth;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 260),
      reverseDuration: const Duration(milliseconds: 220),
    );
    _depth = CurvedAnimation(
      parent: _controller,
      // Enter: smooth, slightly elastic depth feel.
      curve: const Cubic(0.22, 1.0, 0.36, 1.0),
      // Exit: crisp settle back without sticky tail.
      reverseCurve: const Cubic(0.40, 0.0, 0.20, 1.0),
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final shadowColor = theme.colorScheme.shadow;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => _controller.forward(),
      onExit: (_) => _controller.reverse(),
      child: AnimatedBuilder(
        animation: _depth,
        builder: (context, child) {
          final t = _depth.value;
          final scale = 1 + (0.016 * t);
          final ambientAlpha = 0.05 + (0.05 * t);
          final ambientBlur = 7 + (7 * t);
          final ambientSpread = 0 + (0.45 * t);
          final keyAlpha = 0.03 + (0.03 * t);
          final keyBlur = 8 + (8 * t);
          final keySpread = 0 + (0.18 * t);
          final keyDx = 0.8 + (0.6 * t);
          final keyDy = 3.2 + (2.2 * t);

          return Transform.scale(
            scale: scale,
            child: Container(
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(16),
                boxShadow: [
                  // Ambient shadow grows outward to suggest depth toward viewer.
                  BoxShadow(
                    color: shadowColor.withValues(alpha: ambientAlpha),
                    blurRadius: ambientBlur,
                    spreadRadius: ambientSpread,
                    offset: Offset.zero,
                  ),
                  // Slight directional key shadow keeps lighting realistic.
                  BoxShadow(
                    color: shadowColor.withValues(alpha: keyAlpha),
                    blurRadius: keyBlur,
                    spreadRadius: keySpread,
                    offset: Offset(keyDx, keyDy),
                  ),
                ],
              ),
              child: child,
            ),
          );
        },
        child: widget.child,
      ),
    );
  }
}

class HomeCoverImage extends StatelessWidget {
  const HomeCoverImage({
    super.key,
    required this.trackId,
    required this.coverDir,
  });

  final int? trackId;
  final String coverDir;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final placeholder = Container(
      color: Color.alphaBlend(
        theme.colorScheme.primary.withValues(alpha: 0.16),
        theme.colorScheme.surfaceContainer,
      ),
      child: Center(
        child: Icon(
          Icons.music_note,
          color: theme.colorScheme.primary,
          size: 30,
        ),
      ),
    );

    if (trackId == null || coverDir.trim().isEmpty) {
      return placeholder;
    }

    final path = '$coverDir${Platform.pathSeparator}$trackId';
    return Image(
      image: ResizeImage(
        FileImage(File(path)),
        width: 360,
        height: 360,
        allowUpscaling: false,
      ),
      fit: BoxFit.cover,
      gaplessPlayback: true,
      errorBuilder: (_, _, _) => placeholder,
    );
  }
}

class HomeSectionHeader extends StatelessWidget {
  const HomeSectionHeader({
    super.key,
    required this.title,
    required this.moreLabel,
    required this.onMore,
  });

  final String title;
  final String moreLabel;
  final VoidCallback onMore;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      children: [
        Text(
          title,
          style: theme.textTheme.titleLarge?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        const Spacer(),
        TextButton.icon(
          onPressed: onMore,
          icon: const Icon(Icons.arrow_forward_ios, size: 14),
          label: Text(moreLabel),
        ),
      ],
    );
  }
}

class HomeBackdrop extends StatelessWidget {
  const HomeBackdrop({super.key, required this.theme});

  final ThemeData theme;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [
            Color.alphaBlend(
              theme.colorScheme.primary.withValues(alpha: 0.16),
              theme.colorScheme.surface,
            ),
            Color.alphaBlend(
              theme.colorScheme.secondary.withValues(alpha: 0.12),
              theme.colorScheme.surfaceContainerLowest,
            ),
          ],
        ),
      ),
      child: Stack(
        children: [
          Positioned(
            left: -120,
            top: -90,
            child: HomeGlowBubble(
              size: 300,
              color: theme.colorScheme.primary.withValues(alpha: 0.17),
            ),
          ),
          Positioned(
            right: -80,
            top: 120,
            child: HomeGlowBubble(
              size: 230,
              color: theme.colorScheme.secondary.withValues(alpha: 0.15),
            ),
          ),
          Positioned(
            right: 120,
            bottom: -120,
            child: HomeGlowBubble(
              size: 260,
              color: theme.colorScheme.tertiary.withValues(alpha: 0.09),
            ),
          ),
        ],
      ),
    );
  }
}

class HomeGlowBubble extends StatelessWidget {
  const HomeGlowBubble({super.key, required this.size, required this.color});

  final double size;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      child: Container(
        width: size,
        height: size,
        decoration: BoxDecoration(shape: BoxShape.circle, color: color),
      ),
    );
  }
}

class HomeEmptySectionHint extends StatelessWidget {
  const HomeEmptySectionHint({super.key, required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      height: 72,
      alignment: Alignment.centerLeft,
      padding: const EdgeInsets.symmetric(horizontal: 14),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(14),
        color: theme.colorScheme.surfaceContainerLowest.withValues(alpha: 0.76),
        border: Border.all(
          color: theme.colorScheme.onSurface.withValues(alpha: 0.08),
        ),
      ),
      child: Text(message, style: theme.textTheme.bodyMedium),
    );
  }
}

Iterable<T> _singleOrEmpty<T>(T? value) sync* {
  if (value != null) {
    yield value;
  }
}
