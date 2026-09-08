import 'package:flutter/material.dart';

/// Desktop theme tokens. Presets supply literal colors; [ArtworkPalette.fromSeed]
/// provides the neutral fallback. Playback details use DetailArtworkPalette.
class ArtworkPalette extends ThemeExtension<ArtworkPalette> {
  const ArtworkPalette({
    required this.top,
    required this.bottom,
    required this.glow,
    required this.detailTop,
    required this.detailBottom,
    required this.accent,
    required this.surface,
    required this.playerSurface,
    this.backgroundAsset,
    this.homeBannerAsset = 'assets/images/banners/daylight.png',
    this.backgroundTint = const Color(0x665F6570),
    this.onBackdrop = Colors.white,
    this.onSurface = const Color(0xFF373D49),
    this.onSurfaceVariant = const Color(0xFF636A76),
    this.onAccent = Colors.white,
  });
  final String? backgroundAsset;
  final String homeBannerAsset;
  final Color backgroundTint;
  final Color onBackdrop;
  final Color onSurface, onSurfaceVariant, onAccent;

  Brightness get brightness =>
      surface.computeLuminance() < .35 ? Brightness.dark : Brightness.light;
  Color get outline => onSurface.withValues(alpha: .12);
  Color get controlSurface =>
      Color.alphaBlend(onSurface.withValues(alpha: .05), surface);

  /// Keep text, controls and overlay routes on the same surface palette.
  ThemeData applyTo(ThemeData base) {
    final scheme =
        ColorScheme.fromSeed(
          seedColor: accent,
          brightness: brightness,
        ).copyWith(
          primary: accent,
          onPrimary: onAccent,
          secondary: accent,
          onSecondary: onAccent,
          surface: surface,
          onSurface: onSurface,
          onSurfaceVariant: onSurfaceVariant,
          surfaceContainerLowest: surface,
          surfaceContainerLow: controlSurface,
          surfaceContainer: controlSurface,
          surfaceContainerHigh: controlSurface,
          surfaceContainerHighest: Color.alphaBlend(
            onSurface.withValues(alpha: .09),
            surface,
          ),
          surfaceTint: Colors.transparent,
          outline: onSurfaceVariant,
          outlineVariant: outline,
        );
    return base.copyWith(
      colorScheme: scheme,
      canvasColor: surface,
      cardColor: surface,
      dividerColor: outline,
      textTheme: base.textTheme.apply(
        bodyColor: onSurface,
        displayColor: onSurface,
      ),
      iconTheme: IconThemeData(color: onSurfaceVariant, size: 20),
      inputDecorationTheme: InputDecorationThemeData(
        filled: true,
        fillColor: controlSurface,
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(9),
          borderSide: BorderSide(color: outline),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(9),
          borderSide: BorderSide(color: outline),
        ),
      ),
      extensions: [
        ...base.extensions.values.where((e) => e is! ArtworkPalette),
        this,
      ],
    );
  }

  final Color top,
      bottom,
      glow,
      detailTop,
      detailBottom,
      accent,
      surface,
      playerSurface;
  static final neutral = ArtworkPalette.fromSeed(null);
  static ArtworkPalette of(BuildContext context) =>
      Theme.of(context).extension<ArtworkPalette>() ?? neutral;

  factory ArtworkPalette.fromSeed(Color? seed) {
    final source = HSLColor.fromColor(seed ?? const Color(0xFF747985));
    final colorful = seed != null && source.saturation > .08;
    final hue = colorful ? source.hue : 220.0;
    final saturation = colorful ? source.saturation.clamp(0.0, .16) : 0.025;
    Color tone(double lightness, double amount) =>
        HSLColor.fromAHSL(1, hue, amount, lightness).toColor();
    return ArtworkPalette(
      top: tone(.235, saturation * .55),
      bottom: tone(.335, saturation * .65),
      glow: tone(.39, saturation),
      detailTop: tone(.14, saturation * .8),
      detailBottom: tone(.235, saturation),
      accent: tone(.36, colorful ? source.saturation.clamp(.16, .32) : .16),
      surface: tone(.94, colorful ? .035 : .015),
      playerSurface: tone(.885, colorful ? .04 : .015),
    );
  }

  @override
  ArtworkPalette copyWith({
    Color? top,
    Color? bottom,
    Color? glow,
    Color? detailTop,
    Color? detailBottom,
    Color? accent,
    Color? surface,
    Color? playerSurface,
    String? backgroundAsset,
    String? homeBannerAsset,
    Color? backgroundTint,
    Color? onBackdrop,
    Color? onSurface,
    Color? onSurfaceVariant,
    Color? onAccent,
  }) => ArtworkPalette(
    top: top ?? this.top,
    bottom: bottom ?? this.bottom,
    glow: glow ?? this.glow,
    detailTop: detailTop ?? this.detailTop,
    detailBottom: detailBottom ?? this.detailBottom,
    accent: accent ?? this.accent,
    surface: surface ?? this.surface,
    playerSurface: playerSurface ?? this.playerSurface,
    backgroundAsset: backgroundAsset ?? this.backgroundAsset,
    homeBannerAsset: homeBannerAsset ?? this.homeBannerAsset,
    backgroundTint: backgroundTint ?? this.backgroundTint,
    onBackdrop: onBackdrop ?? this.onBackdrop,
    onSurface: onSurface ?? this.onSurface,
    onSurfaceVariant: onSurfaceVariant ?? this.onSurfaceVariant,
    onAccent: onAccent ?? this.onAccent,
  );

  @override
  ArtworkPalette lerp(covariant ArtworkPalette? other, double t) {
    if (other == null) return this;
    return ArtworkPalette(
      top: Color.lerp(top, other.top, t)!,
      bottom: Color.lerp(bottom, other.bottom, t)!,
      glow: Color.lerp(glow, other.glow, t)!,
      detailTop: Color.lerp(detailTop, other.detailTop, t)!,
      detailBottom: Color.lerp(detailBottom, other.detailBottom, t)!,
      accent: Color.lerp(accent, other.accent, t)!,
      surface: Color.lerp(surface, other.surface, t)!,
      playerSurface: Color.lerp(playerSurface, other.playerSurface, t)!,
      backgroundAsset: other.backgroundAsset,
      homeBannerAsset: other.homeBannerAsset,
      backgroundTint: Color.lerp(backgroundTint, other.backgroundTint, t)!,
      onBackdrop: Color.lerp(onBackdrop, other.onBackdrop, t)!,
      onSurface: Color.lerp(onSurface, other.onSurface, t)!,
      onSurfaceVariant: Color.lerp(
        onSurfaceVariant,
        other.onSurfaceVariant,
        t,
      )!,
      onAccent: Color.lerp(onAccent, other.onAccent, t)!,
    );
  }
}

class _PaletteTween extends Tween<ArtworkPalette> {
  _PaletteTween({required ArtworkPalette end}) : super(begin: end, end: end);
  @override
  ArtworkPalette lerp(double t) => begin!.lerp(end, t);
}

class ArtworkTheme extends StatelessWidget {
  const ArtworkTheme({super.key, required this.palette, required this.child});
  final ArtworkPalette palette;
  final Widget child;
  @override
  Widget build(BuildContext context) => TweenAnimationBuilder<ArtworkPalette>(
    tween: _PaletteTween(end: palette),
    duration: const Duration(milliseconds: 800),
    curve: Curves.easeInOut,
    child: child,
    builder: (context, value, child) =>
        Theme(data: value.applyTo(Theme.of(context)), child: child!),
  );
}

/// No continuously running animation: only the shared palette transition repaints.
class ArtworkBackdrop extends StatelessWidget {
  const ArtworkBackdrop({
    super.key,
    this.detail = false,
    this.child = const SizedBox.expand(),
  });
  final bool detail;
  final Widget child;
  @override
  Widget build(BuildContext context) {
    final palette = ArtworkPalette.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: detail
              ? [palette.detailTop, palette.detailBottom]
              : [palette.top, palette.bottom],
        ),
      ),
      child: DecoratedBox(
        decoration: BoxDecoration(
          gradient: RadialGradient(
            center: const Alignment(.65, -.45),
            radius: 1.25,
            colors: [
              palette.glow.withValues(alpha: detail ? .18 : .30),
              palette.glow.withValues(alpha: 0),
            ],
          ),
        ),
        child: Stack(
          fit: StackFit.passthrough,
          children: [
            if (!detail)
              Positioned.fill(
                child: IgnorePointer(
                  child: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 800),
                    child: palette.backgroundAsset == null
                        ? const SizedBox.expand()
                        : Image.asset(
                            palette.backgroundAsset!,
                            key: ValueKey(palette.backgroundAsset),
                            fit: BoxFit.cover,
                            width: double.infinity,
                            height: double.infinity,
                            cacheWidth: 1536,
                            excludeFromSemantics: true,
                            color: palette.backgroundTint,
                            colorBlendMode: BlendMode.srcATop,
                            errorBuilder: (_, _, _) => const SizedBox.expand(),
                          ),
                  ),
                ),
              ),
            child,
          ],
        ),
      ),
    );
  }
}
