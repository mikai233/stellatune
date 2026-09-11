import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/ui/pages/library/catalog_widgets.dart';

void main() {
  testWidgets(
    'collection grid fills five columns and ignores viewport height',
    (tester) async {
      tester.view.physicalSize = const Size(750, 480);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(
        ProviderScope(
          child: MaterialApp(
            home: Scaffold(
              body: CatalogItemsView(
                locationKey: 'density',
                grid: true,
                items: [
                  for (var i = 0; i < 100; i++)
                    CatalogItem(
                      reference: MediaRef(
                        sourceInstanceId: '1',
                        kind: MediaKind.album,
                        id: '$i',
                      ),
                      title: 'Album $i',
                      artistRefs: const [],
                    ),
                ],
                itemBuilder: (item, _) =>
                    CatalogCollectionCard(item: item, onOpen: () {}),
                footer: const SizedBox(),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final cards = find.byType(CatalogCollectionCard);
      final first = tester.getRect(cards.at(0));
      expect(tester.getRect(cards.at(4)).top, first.top);
      expect(tester.getRect(cards.at(5)).top, greaterThan(first.bottom));
      expect(tester.getRect(cards.at(1)).left - first.right, closeTo(18, .1));
      expect(
        tester.getSize(find.byType(CatalogArtwork).first).width,
        first.width,
      );
      expect(first.width, inInclusiveRange(125, 145));
      tester.view.physicalSize = const Size(750, 800);
      await tester.pumpAndSettle();
      expect(tester.getRect(cards.first).size, first.size);
      expect(tester.takeException(), isNull);
    },
  );

  for (final kind in [MediaKind.album, MediaKind.artist]) {
    testWidgets(
      '${kind.name} hover stays within artwork and keyboard can open it',
      (tester) async {
        var opened = 0;
        await tester.pumpWidget(
          ProviderScope(
            child: MaterialApp(
              home: Scaffold(
                body: Align(
                  alignment: Alignment.topLeft,
                  child: SizedBox(
                    width: 165,
                    height: 215,
                    child: CatalogCollectionCard(
                      item: CatalogItem(
                        reference: MediaRef(
                          sourceInstanceId: '1',
                          kind: kind,
                          id: '1',
                        ),
                        title: 'Collection',
                        artistRefs: const [],
                      ),
                      onOpen: () => opened++,
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
        final feedback = find.byKey(const ValueKey('catalog-cover-feedback'));
        final before = tester.getRect(find.byType(CatalogCollectionCard));
        final pointer = await tester.createGesture(
          kind: PointerDeviceKind.mouse,
        );
        await pointer.addPointer(location: const Offset(400, 400));
        await pointer.moveTo(tester.getCenter(find.text('Collection')));
        await tester.pumpAndSettle();
        expect(tester.getRect(find.byType(CatalogCollectionCard)), before);
        expect(
          tester.getRect(feedback),
          tester.getRect(find.byType(CatalogArtwork)),
        );
        final ink = tester.widget<InkWell>(find.byType(InkWell));
        expect(
          ink.overlayColor!.resolve({WidgetState.hovered}),
          Colors.transparent,
        );
        final decoration =
            tester.widget<AnimatedContainer>(feedback).decoration!
                as BoxDecoration;
        expect(decoration.color!.a, greaterThan(0));
        if (kind == MediaKind.artist) {
          expect(decoration.borderRadius, BorderRadius.circular(82.5));
        }
        expect(opened, 0);
        await pointer.removePointer();
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
        await tester.pumpAndSettle();
        expect(opened, 1);
        expect(tester.takeException(), isNull);
      },
    );
  }
}
