import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/ui/diagnostics/log_page.dart';
import 'package:stellatune/ui/diagnostics/log_record_view.dart';

void main() {
  testWidgets(
    'burst and follow only build viewport rows in a full log buffer',
    (tester) async {
      final service = DiagnosticsService()..chinese = false;
      var builds = 0;
      final old = debugOnRebuildDirtyWidget;
      debugOnRebuildDirtyWidget = (element, builtOnce) {
        old?.call(element, builtOnce);
        if (element.widget is LogRecordTile) builds++;
      };
      addTearDown(() => debugOnRebuildDirtyWidget = old);
      for (var i = 0; i < 5000; i++) {
        service.record(
          'INFO',
          'stress',
          i.isEven ? 'Record $i' : 'Record $i\nSecond line',
        );
      }
      await tester.pumpWidget(
        MaterialApp(home: DiagnosticsPage(service: service)),
      );
      await tester.pumpAndSettle();
      debugPrint('Log rows built on opening: $builds');
      expect(
        builds,
        lessThan(80),
        reason: 'Opening a full buffer should not lay out thousands of rows',
      );
      builds = 0;
      for (var i = 0; i < 2000; i++) {
        service.record('INFO', 'stress', 'Burst $i');
      }
      await tester.pumpAndSettle();
      debugPrint('Log rows built for a 2000-record burst: $builds');
      expect(
        builds,
        lessThan(80),
        reason: 'Follow must jump directly to the last viewport',
      );
      expect(find.text('Burst 1999'), findsOneWidget);
      final list = tester.widget<ListView>(find.byType(ListView));
      await tester.tap(find.byTooltip('Follow latest'));
      await tester.pumpAndSettle();
      list.controller!.jumpTo(1200);
      await tester.pumpAndSettle();
      final offset = list.controller!.offset;
      for (var i = 0; i < 200; i++) {
        service.record('INFO', 'stress', 'Paused $i');
      }
      await tester.pumpAndSettle();
      expect(list.controller!.offset, offset);
      await tester.tap(find.byTooltip('Follow latest'));
      await tester.pumpAndSettle();
      expect(find.text('Paused 199'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
      await service.shutdown();
      expect(tester.takeException(), isNull);
    },
  );

  test('native batches deduplicate and release evicted IDs', () async {
    final service = DiagnosticsService();
    for (var i = 0; i < 5000; i++) {
      service.record('INFO', 'stress', 'Record $i');
    }
    final first = service.records.first;
    final last = service.records.last;
    final revision = service.recordsRevision;
    service.acceptBatch(LogBatch(records: [last, last], resync: false));
    for (var i = 0; i < 100; i++) {
      service.acceptBatch(const LogBatch(records: [], resync: false));
    }
    expect(service.recordsRevision, revision);
    service.record('INFO', 'stress', 'Evict oldest');
    service.acceptBatch(LogBatch(records: [first, first], resync: true));
    expect(service.records.length, 5000);
    expect(service.records.last.id, first.id);
    expect(service.recordsRevision, revision + 2);
    await service.shutdown();
  });

  testWidgets(
    'large text scaling and long messages keep bounded rows and complete details',
    (tester) async {
      final service = DiagnosticsService()..chinese = false;
      final message = '${'x' * 2047}😀${'y' * 8000}';
      final id = service.record(
        'ERROR',
        'stress',
        message,
        error: 'Full error details',
      );
      await tester.pumpWidget(
        MaterialApp(
          home: MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(2)),
            child: DiagnosticsPage(service: service),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('${'x' * 2047}…'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.tap(find.byType(LogRecordTile));
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<LogRecordDetails>(find.byType(LogRecordDetails))
            .record
            .message,
        message,
      );
      expect(
        (await service.detail(id))!.details,
        contains('Full error details'),
      );
      await tester.pumpWidget(const SizedBox());
      await service.shutdown();
    },
  );
}
