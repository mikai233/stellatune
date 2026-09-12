import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/ui/widgets/app_select.dart';

void main() {
  testWidgets(
    'clicking outside a closed selector releases its restored focus',
    (tester) async {
      var outsideClicks = 0;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Column(
              children: [
                AppSelect<String>(
                  width: 200,
                  value: 'Local library',
                  items: const [
                    DropdownMenuItem(
                      value: 'Local library',
                      child: Text('Local library'),
                    ),
                  ],
                  onChanged: (_) {},
                ),
                const SizedBox(height: 100),
                GestureDetector(
                  onTap: () => outsideClicks++,
                  child: const SizedBox(
                    width: 200,
                    height: 50,
                    child: Text('Other action'),
                  ),
                ),
              ],
            ),
          ),
        ),
      );
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(400, 400));
      Future<void> click(Finder target) async {
        await mouse.moveTo(tester.getCenter(target));
        await mouse.down(tester.getCenter(target));
        await mouse.up();
        await tester.pumpAndSettle();
      }

      await click(find.byType(OutlinedButton));
      await click(find.text('Local library').last);
      await click(find.text('Other action'));
      expect(outsideClicks, 1);
      expect(
        tester
            .widget<OutlinedButton>(find.byType(OutlinedButton))
            .focusNode!
            .hasFocus,
        isFalse,
      );
      await click(find.byType(OutlinedButton));
      await click(find.text('Other action'));
      expect(outsideClicks, 2);
      expect(find.byType(MenuItemButton), findsNothing);
      expect(
        tester
            .widget<OutlinedButton>(find.byType(OutlinedButton))
            .focusNode!
            .hasFocus,
        isFalse,
      );
      await mouse.removePointer();
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'selector supports system default, disabled entries and keyboard selection',
    (tester) async {
      String? value;
      final changes = <String?>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Center(
              child: StatefulBuilder(
                builder: (context, setState) => AppSelect<String?>(
                  width: 200,
                  value: value,
                  items: const [
                    DropdownMenuItem(
                      value: null,
                      child: Text('System default'),
                    ),
                    DropdownMenuItem(
                      value: 'offline',
                      enabled: false,
                      child: Text('Offline device'),
                    ),
                    DropdownMenuItem(
                      value: 'speakers',
                      child: Text('Speakers'),
                    ),
                  ],
                  onChanged: (next) => setState(() {
                    value = next;
                    changes.add(next);
                  }),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.byType(OutlinedButton));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Offline device'));
      await tester.pumpAndSettle();
      expect(changes, isEmpty);
      await tester.tap(find.text('Speakers'));
      await tester.pumpAndSettle();
      expect(value, 'speakers');
      // Pointer selection releases focus. Tab returns to the trigger, and
      // keyboard selection keeps it there for subsequent Enter/Escape actions.
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.text('System default'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(changes, ['speakers', null]);
      expect(find.text('System default'), findsOneWidget);
      expect(
        tester
            .widget<OutlinedButton>(find.byType(OutlinedButton))
            .focusNode!
            .hasFocus,
        isTrue,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.byType(MenuItemButton), findsWidgets);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<OutlinedButton>(find.byType(OutlinedButton))
            .focusNode!
            .hasFocus,
        isTrue,
      );
      expect(tester.takeException(), isNull);
    },
  );
}
