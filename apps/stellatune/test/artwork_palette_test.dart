import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:stellatune/ui/theme/artwork_palette_provider.dart';

void main() {
  test('extreme artwork keeps backgrounds muted and controls readable', () {
    for (final seed in [
      Colors.green,
      Colors.red,
      Colors.blue,
      Colors.yellow,
      Colors.white,
      Colors.black,
      null,
    ]) {
      final palette = ArtworkPalette.fromSeed(seed);
      for (final color in [
        palette.top,
        palette.bottom,
        palette.detailTop,
        palette.detailBottom,
      ]) {
        expect(HSLColor.fromColor(color).saturation, lessThan(.18));
        expect(1.05 / (color.computeLuminance() + .05), greaterThan(4.5));
      }
      expect(
        1.05 / (palette.accent.computeLuminance() + .05),
        greaterThan(4.5),
      );
      expect(
        (palette.surface.computeLuminance() + .05) /
            (const Color(0xFF373D49).computeLuminance() + .05),
        greaterThan(7),
      );
    }
  });

  test('missing artwork returns the neutral palette', () async {
    expect(
      await loadArtworkPalette((
        trackKey: 'missing',
        localPath: null,
        kind: null,
        value: null,
      )),
      same(ArtworkPalette.neutral),
    );
  });

  test('a late previous cover cannot overwrite the current track', () async {
    const a = (trackKey: 'a', localPath: null, kind: null, value: null);
    const b = (trackKey: 'b', localPath: null, kind: null, value: null);
    final first = Completer<ArtworkPalette>();
    final second = Completer<ArtworkPalette>();
    var calls = 0;
    final loader = artworkPaletteLoaderProvider.overrideWithValue((request) {
      calls++;
      return request.trackKey == 'a' ? first.future : second.future;
    });
    final container = ProviderContainer(
      overrides: [currentArtworkRequestProvider.overrideWithValue(a), loader],
    );
    addTearDown(container.dispose);
    final shell = container.listen(currentArtworkPaletteProvider, (_, _) {});
    final detail = container.listen(currentArtworkPaletteProvider, (_, _) {});
    addTearDown(shell.close);
    addTearDown(detail.close);
    expect(calls, 1); // Both pages share extraction.
    container.updateOverrides([
      currentArtworkRequestProvider.overrideWithValue(b),
      loader,
    ]);
    final latest = container.read(currentArtworkPaletteProvider.future);
    final blue = ArtworkPalette.fromSeed(Colors.blue);
    second.complete(blue);
    expect(await latest, same(blue));
    first.complete(ArtworkPalette.fromSeed(Colors.green));
    await Future<void>.delayed(Duration.zero);
    expect(container.read(currentArtworkPaletteProvider).value, same(blue));
    expect(calls, 2);
  });
}
