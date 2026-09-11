import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:stellatune/ui/theme/desktop_theme.dart';

class _DelayedBackdropBundle extends CachingAssetBundle {
  _DelayedBackdropBundle(this.path, this.bytes);
  final String path;
  final ByteData bytes;
  Completer<ByteData>? reload;
  int reads = 0;

  @override
  Future<ByteData> load(String key) async {
    if (key != path) return rootBundle.load(key);
    reads++;
    return reload?.future ?? bytes;
  }
}

void main() {
  testWidgets(
    'returning from an opaque detail route retains the background after cache eviction',
    (tester) async {
      final palette = DesktopThemePreset.daylight.palette;
      final path = palette.backgroundAsset!;
      final bytes = await tester.runAsync(() => rootBundle.load(path));
      final bundle = _DelayedBackdropBundle(path, bytes!);
      final navigator = GlobalKey<NavigatorState>();
      await tester.pumpWidget(
        DefaultAssetBundle(
          bundle: bundle,
          child: MaterialApp(
            navigatorKey: navigator,
            home: ArtworkTheme(
              palette: palette,
              child: const Scaffold(body: ArtworkBackdrop()),
            ),
          ),
        ),
      );
      final backdrop = find.byType(ArtworkBackdrop, skipOffstage: false);
      final imageFinder = find.descendant(
        of: backdrop,
        matching: find.byType(Image, skipOffstage: false),
      );
      final provider = tester.widget<Image>(imageFinder).image;
      final imageContext = tester.element(imageFinder);
      await tester.runAsync(() => precacheImage(provider, imageContext));
      await tester.pumpAndSettle();
      final raw = find.descendant(
        of: backdrop,
        matching: find.byType(RawImage, skipOffstage: false),
      );
      expect(tester.widget<RawImage>(raw).image, isNotNull);
      final key = await provider.obtainKey(
        createLocalImageConfiguration(imageContext),
      );
      unawaited(
        navigator.currentState!.push(
          MaterialPageRoute<void>(
            builder: (_) => const Scaffold(body: Text('Detail')),
          ),
        ),
      );
      await tester.pumpAndSettle();
      // Hidden routes stop listening to still-image streams. A busy library can
      // evict their cached entry while the page itself remains mounted.
      PaintingBinding.instance.imageCache.evict(key, includeLive: false);
      bundle.reload = Completer<ByteData>();
      final readsBeforeReturn = bundle.reads;
      navigator.currentState!.pop();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(bundle.reads, greaterThan(readsBeforeReturn));
      expect(
        tester.widget<RawImage>(raw).image,
        isNotNull,
        reason: 'the existing background must remain painted until re-decoding finishes',
      );
      bundle.reload!.complete(bytes);
      await tester.pump();
      await tester.runAsync(() => precacheImage(provider, imageContext));
      await tester.pumpAndSettle();
      expect(tester.widget<RawImage>(raw).image, isNotNull);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      PaintingBinding.instance.imageCache.clear();
    },
  );
}
