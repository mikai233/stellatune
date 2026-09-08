import 'package:stellatune/ui/theme/desktop_theme.dart';

import 'dart:convert';

import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/music_detail/widgets/layouts.dart';
import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:stellatune/ui/theme/artwork_palette_provider.dart';

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/ui/pages/home/home_placeholders.dart';
import 'package:stellatune/ui/pages/home/home_view_data.dart';
import 'package:stellatune/ui/preview/home_preview.dart';
import 'package:stellatune/ui/preview/library_preview.dart';
import 'package:stellatune/ui/pages/shell/desktop_frame.dart';
import 'package:stellatune/ui/widgets/track_list.dart';
import 'package:stellatune/ui/widgets/dynamic_background.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    for (final entry in {
      'NotoSansSC': 'assets/fonts/NotoSansSC-Regular.ttf',
      'Caveat': 'assets/fonts/caveat/Caveat.ttf',
      'MaterialIcons': 'fonts/MaterialIcons-Regular.otf',
    }.entries) {
      await (FontLoader(
        entry.key,
      )..addFont(rootBundle.load(entry.value))).load();
    }
  });

  Future<void> capture(WidgetTester tester, String name) async {
    final boundary = tester.renderObject<RenderRepaintBoundary>(
      find.byKey(const ValueKey('visual-root')),
    );
    await tester.runAsync(() async {
      final image = await boundary.toImage(pixelRatio: 1);
      final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
      image.dispose();
      final dir = await Directory('build/visual-review')
          .create(recursive: true);
      await File('${dir.path}/$name.png')
          .writeAsBytes(bytes!.buffer.asUint8List());
    });
  }

  Future<void> mount(
    WidgetTester tester,
    Size size, {
    HomeViewData data = HomePlaceholders.data,
    double textScale = 1,
    bool library = false,
    bool settings = false,
    DesktopThemePreset desktopTheme = DesktopThemePreset.dusk,
    Brightness brightness = Brightness.light,
  }) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = size;
    await tester.runAsync(() async {
      await tester.pumpWidget(
        RepaintBoundary(
          key: const ValueKey('visual-root'),
          child: HomeVisualPreview(
            key: ValueKey('preview-$settings-$library-${desktopTheme.name}'),
            data: data,
            desktopTheme: desktopTheme,
            brightness: brightness,
            textScale: textScale,
            initialDestination: settings
                ? 3
                : library
                ? 1
                : 0,
            contentBuilder: library
                ? (_) => const LibraryVisualPreview()
                : null,
          ),
        ),
      );
    });
    final context = tester.element(find.byType(HomeVisualPreview));
    await tester.runAsync(() async {
      final background = desktopTheme.palette.backgroundAsset;
      if (background != null) {
        await precacheImage(
          ResizeImage(AssetImage(background), width: 1536),
          context,
        );
      }
      await precacheImage(
        ResizeImage(
          AssetImage(desktopTheme.palette.homeBannerAsset),
          width: 1536,
        ),
        context,
      );
      for (final asset in {
        ...HomePlaceholders.artwork,
        HomePlaceholders.flowers,
      }) {
        await precacheImage(AssetImage(asset), context);
      }
    });
    await tester.pumpAndSettle();
  }

  for (final preset in DesktopThemePreset.values) {
    for (final brightness in Brightness.values) {
      testWidgets(
        'settings dropdown contrast ${preset.name} on ${brightness.name} app theme',
        (tester) async {
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          await mount(
            tester,
            const Size(984, 711),
            settings: true,
            brightness: brightness,
            desktopTheme: preset,
          );
          await tester.tap(find.byType(DropdownButtonFormField<Locale?>));
          await tester.pumpAndSettle();
          final english = find.text('英文').last;
          final label = tester.renderObject<RenderParagraph>(english);
          final foreground = label.text.style!.color!;
          final sample = tester.getTopLeft(english) - const Offset(4, 0);
          final boundary = tester.renderObject<RenderRepaintBoundary>(
            find.byKey(const ValueKey('visual-root')),
          );
          final background = await tester.runAsync(() async {
            final image = await boundary.toImage(pixelRatio: 1);
            final bytes = (await image.toByteData(
              format: ui.ImageByteFormat.rawRgba,
            ))!;
            final offset =
                (sample.dy.round() * image.width + sample.dx.round()) * 4;
            final color = Color.fromARGB(
              255,
              bytes.getUint8(offset),
              bytes.getUint8(offset + 1),
              bytes.getUint8(offset + 2),
            );
            image.dispose();
            return color;
          });
          await capture(
            tester,
            'settings-dropdown-${preset.name}-${brightness.name}',
          );
          final luminances = [
            background!.computeLuminance(),
            foreground.computeLuminance(),
          ]..sort();
          expect(
            background.computeLuminance(),
            preset.palette.brightness == Brightness.dark
                ? lessThan(.2)
                : greaterThan(.7),
          );
          expect(
            (luminances.last + .05) / (luminances.first + .05),
            greaterThan(4.5),
          );
          await tester.tap(english);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
        },
      );
    }
  }

  for (final preset in DesktopThemePreset.values) {
    testWidgets('fixed desktop theme: ${preset.name}', (tester) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await mount(tester, const Size(1536, 1024), desktopTheme: preset);
      expect(
        find.byKey(ValueKey(preset.palette.homeBannerAsset)),
        findsNWidgets(2),
      );
      await capture(tester, 'home-theme-${preset.name}');
      await tester.tap(find.byTooltip('下一首'));
      await tester.pumpAndSettle();
      expect(
        ArtworkPalette.of(tester.element(find.byType(DesktopFrame))).top,
        preset.palette.top,
      );
      await mount(
        tester,
        const Size(1536, 1024),
        settings: true,
        desktopTheme: preset,
      );
      expect(find.text('主题模式'), findsNothing);
      await capture(tester, 'settings-theme-${preset.name}');
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('settings preset switch recolors the shared shell', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await mount(tester, const Size(1536, 1024), settings: true);
    await tester.tap(find.text('暮色'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('雾蓝').last);
    await tester.pumpAndSettle();
    expect(
      ArtworkPalette.of(tester.element(find.byType(DesktopFrame))).top,
      DesktopThemePreset.mist.palette.top,
    );
  });

  for (final artwork in ['forest', 'moon']) {
    testWidgets('detail artwork palette: $artwork', (tester) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final palette = await tester.runAsync(
        () => detailPaletteFromImage(
          AssetImage('assets/images/home/$artwork.png'),
        ),
      );
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1024, 720);
      final bytes = await rootBundle.load('assets/images/home/$artwork.png');
      final cover = QueueCover(
        kind: QueueCoverKind.data,
        value: base64Encode(bytes.buffer.asUint8List()),
      );
      await tester.runAsync(() async {
        await tester.pumpWidget(
          RepaintBoundary(
            key: const ValueKey('visual-root'),
            child: MaterialApp(
              debugShowCheckedModeBanner: false,
              theme: ThemeData(fontFamily: 'NotoSansSC'),
              home: ShaderBackground(
                colors: palette!.colors,
                animate: false,
                child: LayoutBuilder(
                  builder: (context, constraints) => WideLayout(
                    coverDir: '',
                    trackId: null,
                    trackIdentityKey: artwork,
                    cover: cover,
                    title: artwork == 'forest'
                        ? 'Sunny Mornings'
                        : 'The Moment',
                    subtitle: 'Artwork palette preview',
                    slideDirection: 0,
                    foregroundColor: palette.foreground,
                    maxWidth: constraints.maxWidth,
                    maxHeight: constraints.maxHeight,
                    hasLyrics: false,
                  ),
                ),
              ),
            ),
          ),
        );
        final context = tester.element(find.byType(WideLayout));
        final coverWidget = tester.widget<Image>(find.byType(Image));
        await precacheImage(coverWidget.image, context);
      });
      await tester.pumpAndSettle();
      // Shader readiness/color animation can rebuild the cover with new decoded
      // memory bytes. Wait for the image owned by the final settled frame.
      await tester.runAsync(
        () => precacheImage(
          tester.widget<Image>(find.byType(Image)).image,
          tester.element(find.byType(WideLayout)),
        ),
      );
      await tester.pump();
      expect(tester.widget<RawImage>(find.byType(RawImage)).image, isNotNull);
      await capture(tester, 'detail-palette-$artwork');
      expect(tester.takeException(), isNull);
    });
  }

  for (final size in [
    const Size(1536, 1024),
    const Size(1280, 800),
    const Size(1024, 720),
    const Size(900, 700),
  ]) {
    testWidgets('homepage at ${size.width.toInt()}x${size.height.toInt()}', (
      tester,
    ) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await mount(tester, size);
      await capture(
        tester,
        'home-${size.width.toInt()}x${size.height.toInt()}',
      );
      if (size.width == 1536) {
        final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
        await mouse.addPointer(location: Offset.zero);
        await mouse.moveTo(
          tester.getCenter(find.byKey(const ValueKey('继续聆听-1'))),
        );
        await tester.pumpAndSettle();
        await capture(tester, 'home-hover');
        await mouse.removePointer();
        await tester.tap(find.byKey(const ValueKey('player-play-pause')));
        await tester.pumpAndSettle();
        expect(find.byTooltip('播放'), findsOneWidget);
        await capture(tester, 'home-paused');
      }
      if (size.width == 900) {
        await tester.drag(
          find.byKey(const ValueKey('home-scroll')),
          const Offset(0, -1000),
        );
        await tester.pumpAndSettle();
        await capture(tester, 'home-900-scrolled');
      }
      expect(tester.takeException(), isNull);
    });
  }

  for (final settings in [true, false]) {
    testWidgets('scroll stays below window controls (settings: $settings)', (
      tester,
    ) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await mount(tester, const Size(1024, 720), settings: settings);
      final bar = find.byKey(const ValueKey('desktop-title-bar'));
      final viewport = find.byKey(const ValueKey('desktop-content-viewport'));
      final barBefore = tester.getRect(bar);
      expect(tester.getTopLeft(viewport).dy, barBefore.bottom);
      Future<List<int>> topPixels() async {
        final boundary = tester.renderObject<RenderRepaintBoundary>(
          find.byKey(const ValueKey('visual-root')),
        );
        return (await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          final bytes = await image.toByteData(
            format: ui.ImageByteFormat.rawRgba,
          );
          image.dispose();
          return bytes!.buffer.asUint8List(0, 1024 * 60 * 4).toList();
        }))!;
      }

      final before = await topPixels();
      await tester.drag(
        find.byKey(ValueKey(settings ? 'settings-scroll' : 'home-scroll')),
        const Offset(0, -320),
      );
      await tester.pumpAndSettle();
      expect(tester.getRect(bar), barBefore);
      expect(await topPixels(), orderedEquals(before));
      await capture(
        tester,
        settings ? 'settings-scroll-title-bar' : 'home-scroll-title-bar',
      );
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('title bar drags across its height and leaves controls usable', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1200, 800);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    var drags = 0;
    var closes = 0;
    var actions = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: DesktopFrame(
          selectedIndex: 2,
          onDestinationSelected: (_) {},
          onDrag: () => drags++,
          onClose: () => closes++,
          topBarActions: [
            DesktopTopBarAction(
              icon: Icons.add,
              tooltip: '管理',
              onPressed: () => actions++,
            ),
          ],
          playerBar: const SizedBox(height: 78),
          child: const SizedBox.expand(),
        ),
      ),
    );
    for (final y in [10.0, 35.0, 55.0]) {
      await tester.dragFrom(Offset(500, y), const Offset(50, 0));
      await tester.pump();
    }
    await tester.dragFrom(const Offset(80, 50), const Offset(50, 0));
    await tester.pump();
    expect(drags, 4);
    await tester.tap(find.byTooltip('管理'));
    await tester.tap(find.byTooltip('关闭'));
    await tester.enterText(find.byKey(const ValueKey('desktop-search')), '歌曲');
    expect(actions, 1);
    expect(closes, 1);
    expect(find.text('歌曲'), findsOneWidget);
    expect(drags, 4);
  });

  testWidgets('long titles, missing cover and larger text remain bounded', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final item = HomeCardData(
      title: '这是一首名字特别长的歌曲 / A very long music title',
      subtitle: '一位名字同样很长的艺术家',
      artwork: HomePlaceholders.hero,
      coverPath: 'missing-visual-fixture-cover',
    );
    await mount(
      tester,
      const Size(1024, 720),
      data: HomeViewData(
        continueListening: List.filled(6, item),
        recentlyAdded: List.filled(6, item),
      ),
      textScale: 1.25,
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('继续聆听-0')),
        matching: find.byWidgetPredicate(
          (widget) => widget is Image && widget.image is AssetImage,
        ),
      ),
      findsNWidgets(2),
    );
    await capture(tester, 'home-long-title');
  });

  for (final scenario in [
    (size: const Size(1536, 1024), preset: DesktopThemePreset.dusk),
    (size: const Size(1024, 720), preset: DesktopThemePreset.dusk),
    for (final preset in [
      DesktopThemePreset.sunroom,
      DesktopThemePreset.daylight,
      DesktopThemePreset.lavender,
      DesktopThemePreset.celadon,
    ])
      (size: const Size(1536, 1024), preset: preset),
  ]) {
    final size = scenario.size;
    testWidgets(
      'library songs and collections ${scenario.preset.name} at ${size.width}',
      (tester) async {
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        await tester.runAsync(LibraryVisualPreview.prepareCovers);
        await tester.pumpWidget(const MaterialApp(home: SizedBox()));
        final preloadContext = tester.element(find.byType(SizedBox).first);
        await tester.runAsync(() async {
          for (final track in LibraryVisualPreview.tracks) {
            final file = FileImage(
              File('${LibraryVisualPreview.coverDir}/${track.id}'),
            );
            await precacheImage(
              ResizeImage(file, width: 80, height: 80, allowUpscaling: false),
              preloadContext,
            );
            await precacheImage(ResizeImage(file, width: 512), preloadContext);
          }
        });
        await mount(tester, size, library: true, desktopTheme: scenario.preset);
        await capture(
          tester,
          'library-songs-${scenario.preset.name}-${size.width.toInt()}',
        );
        await tester.tap(find.byKey(const ValueKey('library-tab-albums')));
        await tester.pumpAndSettle();
        await capture(
          tester,
          'library-albums-${scenario.preset.name}-${size.width.toInt()}',
        );
        await tester.tap(find.byKey(const ValueKey('library-collection-0')));
        await tester.pumpAndSettle();
        final list = tester.widget<TrackList>(find.byType(TrackList));
        expect(list.items, hasLength(2));
        expect(list.items.every((track) => track.album == '我要的幸福'), isTrue);
        await tester.tap(find.text('开始懂了').first);
        await tester.pumpAndSettle();
        expect(find.text('播放：开始懂了'), findsOneWidget);
        await tester.tap(find.byIcon(Icons.favorite).first);
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<TrackList>(find.byType(TrackList))
              .likedTrackIds
              .contains(1),
          isFalse,
        );
        await tester.tap(find.byTooltip('返回'));
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const ValueKey('library-tab-artists')));
        await tester.pumpAndSettle();
        await capture(
          tester,
          'library-artists-${scenario.preset.name}-${size.width.toInt()}',
        );
        await tester.tap(find.byTooltip('列表视图'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      },
    );
  }
  for (final size in [
    const Size(1536, 1024),
    const Size(1024, 720),
    const Size(900, 700),
  ]) {
    testWidgets('settings layout and search at ${size.width}', (tester) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await mount(tester, size, settings: true);
      if (size.width == 1536) {
        for (final pair in [('appearance', 'playback'), ('audio', 'library')]) {
          final left = tester.getRect(find.byKey(ValueKey(pair.$1)));
          final right = tester.getRect(find.byKey(ValueKey(pair.$2)));
          expect(left.top, right.top);
          expect(left.bottom, right.bottom);
        }
      }
      await capture(tester, 'settings-${size.width.toInt()}');
      await tester.enterText(
        find.byKey(const ValueKey('settings-search')),
        '播放',
      );
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('appearance')), findsNothing);
      expect(find.byKey(const ValueKey('playback')), findsOneWidget);
      final toggle = find.byType(Switch);
      expect(tester.widget<Switch>(toggle).value, isTrue);
      await tester.tap(toggle);
      await tester.pumpAndSettle();
      expect(tester.widget<Switch>(toggle).value, isFalse);
      await capture(tester, 'settings-search-${size.width.toInt()}');
      await tester.enterText(
        find.byKey(const ValueKey('settings-search')),
        'audio',
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byType(DropdownButtonFormField<String>).first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('WASAPI 独占').last);
      await tester.pumpAndSettle();
      expect(find.text('WASAPI 独占'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }
}
