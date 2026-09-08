import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:palette_generator/palette_generator.dart';
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

  test(
    'detail colors preserve the original population order and luminance rule',
    () {
      final palette = DetailArtworkPalette.fromSwatches([
        PaletteColor(Colors.red, 10),
        PaletteColor(Colors.black, 100),
        PaletteColor(Colors.green, 50),
        PaletteColor(Colors.white, 80),
        PaletteColor(Colors.blue, 20),
      ], dominant: Colors.black);
      expect(palette.colors, [
        Colors.black,
        Colors.white,
        Colors.green,
        Colors.blue,
      ]);
      expect(palette.foreground, Colors.white);
      final bright = DetailArtworkPalette.fromSwatches([
        PaletteColor(Colors.white, 1),
      ], dominant: Colors.white);
      expect(bright.colors, [
        Colors.white,
        Colors.black,
        Colors.black,
        Colors.black,
      ]);
      expect(bright.foreground, Colors.black);
    },
  );

  test(
    'a track with no artwork returns neutral; missing files remain retryable',
    () async {
      expect(
        await loadDetailArtworkPalette(_request('none')),
        same(DetailArtworkPalette.neutral),
      );
      await expectLater(
        loadDetailArtworkPalette((
          trackKey: 'missing',
          localPath: 'nonexistent-test-cover',
          kind: null,
          value: null,
        )),
        throwsA(isA<Exception>()),
      );
    },
  );

  test(
    'late previous cover cannot overwrite the shared current palette',
    () async {
      final first = Completer<DetailArtworkPalette>();
      final second = Completer<DetailArtworkPalette>();
      var calls = 0;
      final loader = artworkPaletteLoaderProvider.overrideWithValue((request) {
        calls++;
        return request.trackKey == 'a' ? first.future : second.future;
      });
      final container = ProviderContainer(
        overrides: [
          currentArtworkRequestProvider.overrideWithValue(_request('a')),
          loader,
        ],
      );
      addTearDown(container.dispose);
      final desktop = container.listen(
        currentArtworkPaletteProvider,
        (_, _) {},
      );
      final mobile = container.listen(currentArtworkPaletteProvider, (_, _) {});
      addTearDown(desktop.close);
      addTearDown(mobile.close);
      expect(calls, 1);
      container.updateOverrides([
        currentArtworkRequestProvider.overrideWithValue(_request('b')),
        loader,
      ]);
      final latest = container.read(currentArtworkPaletteProvider.future);
      final blue = _palette(Colors.blue);
      second.complete(blue);
      expect(await latest, same(blue));
      first.complete(_palette(Colors.red));
      await Future<void>.delayed(Duration.zero);
      expect(container.read(currentArtworkPaletteProvider).value, same(blue));
      expect(calls, 2);
    },
  );

  test('failure is not cached and the same cover can be retried', () async {
    var attempts = 0;
    final green = _palette(Colors.green);
    final container = ProviderContainer(
      overrides: [
        currentArtworkRequestProvider.overrideWithValue(_request('same')),
        artworkPaletteLoaderProvider.overrideWithValue((_) async {
          attempts++;
          if (attempts == 1) throw StateError('temporary decode error');
          return green;
        }),
      ],
    );
    addTearDown(container.dispose);
    final subscription = container.listen(
      currentArtworkPaletteProvider,
      (_, _) {},
    );
    addTearDown(subscription.close);
    await expectLater(
      container.read(currentArtworkPaletteProvider.future),
      throwsStateError,
    );
    container.invalidate(currentArtworkPaletteProvider);
    expect(
      await container.read(currentArtworkPaletteProvider.future),
      same(green),
    );
    expect(attempts, 2);
    container.invalidate(currentArtworkPaletteProvider);
    expect(
      await container.read(currentArtworkPaletteProvider.future),
      same(green),
    );
    expect(attempts, 2, reason: 'successful extraction is cached');
  });

  test('cache shares in-flight extraction and has a bounded history', () async {
    var attempts = 0;
    final cache = DetailArtworkPaletteCache((_) async {
      attempts++;
      return DetailArtworkPalette.neutral;
    });
    final first = cache.load(_request('0'));
    expect(cache.load(_request('0')), same(first));
    await first;
    expect(attempts, 1);
    for (var index = 1; index <= 32; index++) {
      await cache.load(_request('$index'));
    }
    await cache.load(_request('0'));
    expect(attempts, 34);
  });
}

ArtworkRequest _request(String key) =>
    (trackKey: key, localPath: null, kind: null, value: null);

DetailArtworkPalette _palette(Color color) => DetailArtworkPalette(
  colors: List<Color>.filled(4, color),
  foreground: Colors.white,
);
