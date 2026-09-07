import 'package:flutter/material.dart';

import 'package:stellatune/ui/theme/artwork_palette.dart';

class HomeHero extends StatelessWidget {
  const HomeHero({super.key, required this.onResume});
  final VoidCallback onResume;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final palette = ArtworkPalette.of(context);
      final foreground = palette.onBackdrop;
      final scrim = palette.surface;
      final wide = constraints.maxWidth >= 900;
      final textScale = MediaQuery.textScalerOf(context).scale(21) / 21;
      final height =
          (constraints.maxWidth * .185).clamp(178.0, 214.0) +
          (textScale - 1).clamp(0.0, 1.0) * 110;
      return SizedBox(
        height: height,
        child: Row(
          children: [
            Expanded(
              flex: 31,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(14),
                child: Stack(
                  fit: StackFit.expand,
                  children: [
                    _BannerImage(asset: palette.homeBannerAsset),
                    DecoratedBox(
                      decoration: BoxDecoration(
                        gradient: LinearGradient(
                          colors: [
                            scrim.withValues(alpha: .30),
                            scrim.withValues(alpha: .02),
                            scrim.withValues(alpha: .58),
                          ],
                        ),
                      ),
                    ),
                    Positioned(
                      left: 34,
                      top: 26,
                      child: Transform.rotate(
                        angle: -.13,
                        child: Text(
                          'Music\n   Lives in\n      The Real World.',
                          style: TextStyle(
                            fontFamily: 'Caveat',
                            fontSize: 32,
                            height: 1.16,
                            fontStyle: FontStyle.italic,
                            color: foreground,
                            fontWeight: FontWeight.w300,
                          ),
                        ),
                      ),
                    ),
                    Align(
                      alignment: Alignment.centerRight,
                      child: Padding(
                        padding: const EdgeInsets.only(right: 34),
                        child: SizedBox(
                          width: wide ? 250 : 230,
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                '在平凡的日子里\n遇见更好的自己',
                                style: TextStyle(
                                  fontSize: 21,
                                  height: 1.55,
                                  color: foreground,
                                  fontWeight: FontWeight.w400,
                                ),
                              ),
                              const SizedBox(height: 10),
                              Text(
                                '让喜欢的音乐，继续陪伴你。',
                                style: TextStyle(
                                  fontSize: 13,
                                  color: foreground.withValues(alpha: .85),
                                ),
                              ),
                              const SizedBox(height: 18),
                              FilledButton.icon(
                                key: const ValueKey('home-resume'),
                                onPressed: onResume,
                                style: FilledButton.styleFrom(
                                  backgroundColor: palette.accent,
                                  foregroundColor: palette.onAccent,
                                  minimumSize: const Size(122, 36),
                                  padding: const EdgeInsets.symmetric(
                                    horizontal: 18,
                                  ),
                                ),
                                icon: const Icon(
                                  Icons.play_arrow_rounded,
                                  size: 21,
                                ),
                                label: Text('继续播放'),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                    IgnorePointer(
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(14),
                          border: Border.all(
                            color: foreground.withValues(alpha: .14),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
            if (wide) ...[
              const SizedBox(width: 16),
              Expanded(
                flex: 10,
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(14),
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      _BannerImage(
                        asset: palette.homeBannerAsset,
                        alignment: const Alignment(.45, 0),
                      ),
                      DecoratedBox(
                        decoration: BoxDecoration(
                          gradient: LinearGradient(
                            begin: Alignment.topCenter,
                            end: Alignment.bottomCenter,
                            colors: [
                              scrim.withValues(alpha: .08),
                              scrim.withValues(alpha: .78),
                            ],
                          ),
                        ),
                      ),
                      Align(
                        alignment: Alignment.bottomRight,
                        child: Padding(
                          padding: const EdgeInsets.all(25),
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.end,
                            children: [
                              Text(
                                '“音乐是时光的备份，\n也是情绪的出口。”',
                                textAlign: TextAlign.right,
                                style: TextStyle(
                                  color: foreground,
                                  fontSize: 14,
                                  height: 1.8,
                                  letterSpacing: .6,
                                ),
                              ),
                              SizedBox(height: 12),
                              Text(
                                '—— Stellatune',
                                style: TextStyle(
                                  fontSize: 11,
                                  color: foreground.withValues(alpha: .75),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ],
        ),
      );
    },
  );
}

/// Only the artwork fades; the resume button and its semantics stay mounted.
class _BannerImage extends StatelessWidget {
  const _BannerImage({required this.asset, this.alignment = Alignment.center});
  final String asset;
  final Alignment alignment;

  @override
  Widget build(BuildContext context) => AnimatedSwitcher(
    duration: const Duration(milliseconds: 800),
    child: Image.asset(
      asset,
      key: ValueKey(asset),
      width: double.infinity,
      height: double.infinity,
      fit: BoxFit.cover,
      alignment: alignment,
      cacheWidth: 1536,
      excludeFromSemantics: true,
      errorBuilder: (_, _, _) => ColoredBox(
        color: ArtworkPalette.of(context).surface,
        child: const SizedBox.expand(),
      ),
    ),
  );
}
