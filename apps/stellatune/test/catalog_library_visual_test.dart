import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_controller.dart';
import 'package:stellatune/library/catalog_tree_controller.dart';
import 'package:stellatune/ui/pages/library/catalog_library_view.dart';
import 'package:stellatune/ui/pages/library/catalog_widgets.dart';
import 'package:stellatune/ui/preview/home_preview.dart';
import 'package:stellatune/ui/preview/library_preview.dart';
import 'package:stellatune/ui/theme/desktop_theme.dart';

class VisualLibrary implements LibraryBridge {
  @override
  Stream<LibraryEvent> events() => const Stream.empty();
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class VisualCatalog extends CatalogBridge {
  final calls = <CatalogQuery>[];
  CatalogItem folder(String id, String title) => CatalogItem(
    reference: MediaRef(sourceInstanceId: '1', kind: MediaKind.folder, id: id),
    title: title,
    artistRefs: const [],
  );
  @override
  Future<List<LibrarySource>> sources() async => [
    const LibrarySource(
      id: '1',
      name: '本地音乐库',
      local: true,
      available: true,
      browseKinds: [
        MediaKind.track,
        MediaKind.album,
        MediaKind.artist,
        MediaKind.folder,
      ],
      searchKinds: [MediaKind.track, MediaKind.album, MediaKind.artist],
      sorts: CatalogSort.values,
    ),
  ];
  @override
  Future<CatalogPage> browse(CatalogQuery query) async {
    calls.add(query);
    if (query.kind == MediaKind.folder) {
      final parent = query.parent?.id;
      return CatalogPage(
        items: parent == null
            ? [folder('Music', 'Music'), folder('Archive', 'Archive')]
            : parent == 'Music'
            ? [
                for (final name in [
                  '华语',
                  '原声音乐',
                  'Jazz',
                  'Classical',
                  '现场录音',
                  '无损收藏',
                  '日韩',
                  '欧美',
                  '电子',
                  '民谣',
                ])
                  folder('Music/$name', name),
              ]
            : parent == 'Music/华语'
            ? [
                for (final name in ['孙燕姿', '周杰伦', '陈奕迅', '王菲', '林俊杰', '张悬'])
                  folder('Music/华语/$name', name),
              ]
            : [],
      );
    }
    final tracks = LibraryVisualPreview.tracks;
    final count = tracks.length;
    return CatalogPage(
      items: [
        for (var i = 0; i < count; i++)
          CatalogItem(
            reference: MediaRef(
              sourceInstanceId: '1',
              kind: query.kind,
              id: '$i',
            ),
            title: (query.kind == MediaKind.track
                ? tracks[i].title
                : query.kind == MediaKind.album
                ? tracks[i].album
                : tracks[i].artist)!,
            artist: tracks[i].artist,
            album: tracks[i].album,
            durationMs: tracks[i].durationMs,
            localTrackId: tracks[i].id,
            trackCount: query.kind == MediaKind.track ? null : 12,
            artistRefs: const [],
          ),
      ],
      total: count,
    );
  }

  @override
  Future<CatalogItem> detail(MediaRef reference) async {
    final page = await browse(
      CatalogQuery(
        sourceInstanceId: '1',
        kind: reference.kind,
        search: '',
        sort: CatalogSort.default_,
        limit: 100,
      ),
    );
    return page.items.firstWhere((item) => item.reference == reference);
  }
}

void main() {
  setUpAll(() async {
    for (final font in {
      'NotoSansSC': 'assets/fonts/NotoSansSC-Regular.ttf',
      'Caveat': 'assets/fonts/caveat/Caveat.ttf',
      'MaterialIcons': 'fonts/MaterialIcons-Regular.otf',
    }.entries) {
      await (FontLoader(font.key)..addFont(rootBundle.load(font.value))).load();
    }
    await LibraryVisualPreview.prepareCovers();
  });

  testWidgets(
    'approved desktop layouts and animated tree retain independent navigation',
    (tester) async {
      tester.view.physicalSize = const Size(984, 712);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final bridge = VisualCatalog();
      final container = ProviderContainer(
        overrides: [
          catalogBridgeProvider.overrideWithValue(bridge),
          libraryBridgeProvider.overrideWithValue(VisualLibrary()),
          coverDirProvider.overrideWithValue(LibraryVisualPreview.coverDir),
        ],
      );
      addTearDown(container.dispose);
      await tester.pumpWidget(const MaterialApp(home: SizedBox()));
      final imageContext = tester.element(find.byType(MaterialApp));
      await tester.runAsync(() async {
        final background = DesktopThemePreset.celadon.palette.backgroundAsset;
        if (background != null) {
          await precacheImage(
            ResizeImage(AssetImage(background), width: 1536),
            imageContext,
          );
        }
        for (final track in LibraryVisualPreview.tracks) {
          await precacheImage(
            FileImage(
              File(
                '${LibraryVisualPreview.coverDir}${Platform.pathSeparator}${track.id}',
              ),
            ),
            imageContext,
          );
        }
      });
      await tester.pumpAndSettle();
      await tester.runAsync(() async {
        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: RepaintBoundary(
              key: const ValueKey('catalog-visual-root'),
              child: HomeVisualPreview(
                desktopTheme: DesktopThemePreset.celadon,
                initialDestination: 1,
                contentBuilder: (_) => CatalogLibraryView(
                  onAddFolder: () {},
                  onScan: (_) {},
                  folderManager: const SizedBox(),
                ),
              ),
            ),
          ),
        );
      });
      await tester.pumpAndSettle();
      Future<void> capture(String name) async {
        for (var attempt = 0; attempt < 30; attempt++) {
          await tester.runAsync(
            () => Future<void>.delayed(const Duration(milliseconds: 50)),
          );
          await tester.pumpAndSettle();
          final pending = tester
              .widgetList<Image>(find.byType(Image))
              .any(
                (image) => PaintingBinding.instance.imageCache
                    .statusForKey(image.image)
                    .pending,
              );
          if (!pending) break;
        }
        expect(tester.takeException(), isNull);
        if (!const bool.fromEnvironment('CATALOG_CAPTURE')) return;
        final boundary = tester.renderObject<RenderRepaintBoundary>(
          find.byKey(const ValueKey('catalog-visual-root')),
        );
        await tester.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1.5);
          final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
          image.dispose();
          await File('build/visual-review/catalog-$name.png')
              .writeAsBytes(bytes!.buffer.asUint8List());
        });
      }

      await capture('songs');
      for (final kind in [MediaKind.album, MediaKind.artist]) {
        await tester.tap(find.byKey(ValueKey('catalog-tab-${kind.name}')));
        await tester.pumpAndSettle();
        expect(find.byType(SliverGrid), findsOneWidget);
        await capture(kind.name);
        final mouse = await tester.createGesture(
          kind: ui.PointerDeviceKind.mouse,
        );
        await mouse.addPointer(location: Offset.zero);
        await mouse.moveTo(
          tester.getCenter(find.byType(CatalogCollectionCard).at(1)),
        );
        await tester.pumpAndSettle();
        await capture('${kind.name}-hover');
        await mouse.removePointer();
        await tester.pumpAndSettle();
        await tester.tap(find.byTooltip('列表视图'));
        await tester.pumpAndSettle();
        expect(find.byType(SliverGrid), findsNothing);
        await tester.tap(find.byTooltip('网格视图'));
        await tester.pumpAndSettle();
      }
      await tester.tap(find.byKey(const ValueKey('catalog-tab-folder')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('tree-expand-Music')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('tree-expand-Music/华语')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('tree-folder-Music/华语/孙燕姿')));
      await tester.pumpAndSettle();
      expect(container.read(catalogControllerProvider).kind, MediaKind.track);
      expect(
        container.read(catalogControllerProvider).parent?.reference.id,
        'Music/华语/孙燕姿',
      );
      await capture('folders');
      final tree = container.read(catalogTreeProvider('1'));
      final pane = find.byKey(const ValueKey('catalog-tree-pane'));
      final initialWidth = tester.getSize(pane).width;
      final callsBeforeCollapse = bridge.calls.length;
      final songs = find
          .descendant(
            of: find.byType(CatalogItemsView),
            matching: find.byType(Scrollable),
          )
          .first;
      await tester.drag(songs, const Offset(0, -120));
      await tester.pumpAndSettle();
      final songOffset = tester.state<ScrollableState>(songs).position.pixels;
      await tester.drag(
        find.byKey(const ValueKey('catalog-tree-scroll')),
        const Offset(0, -70),
      );
      await tester.pumpAndSettle();
      final treeOffset = tree.scrollOffset;
      expect(treeOffset, greaterThan(0));
      expect(tester.state<ScrollableState>(songs).position.pixels, songOffset);
      await tester.tap(find.byKey(const ValueKey('catalog-toggle-tree')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 120));
      expect(tester.getSize(pane).width, greaterThan(0));
      expect(tester.getSize(pane).width, lessThan(initialWidth));
      await tester.pumpAndSettle();
      expect(tester.getSize(pane).width, 0);
      expect(bridge.calls.length, callsBeforeCollapse);
      await capture('folders-collapsed');
      await tester.tap(find.byKey(const ValueKey('catalog-toggle-tree')));
      await tester.pumpAndSettle();
      expect(tester.getSize(pane).width, initialWidth);
      expect(tree.scrollOffset, treeOffset);
      expect(tester.state<ScrollableState>(songs).position.pixels, songOffset);
      expect(tree.expanded, hasLength(2));
      expect(tester.takeException(), isNull);
    },
  );
}
