import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/ui/widgets/app_select.dart';

void main() {
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
      // Closing restores focus to the trigger, so Enter can reopen the menu.
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.text('System default'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(changes, ['speakers', null]);
      expect(find.text('System default'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );
}
