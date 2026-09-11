import 'package:animations/animations.dart';
import 'package:stellatune/ui/diagnostics/log_window_controls.dart';

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/bridge/api/error.dart';
import 'package:stellatune/ui/diagnostics/diagnostics_overlay.dart';
import 'package:stellatune/ui/diagnostics/log_record_view.dart';

AppError failure(
  String id, {
  ErrorCategory category = ErrorCategory.unavailable,
  String operation = 'set_output',
  String fingerprint = 'driver:realtek',
}) => AppError(
  category: category,
  operation: operation,
  diagnosticId: id,
  fingerprint: fingerprint,
  context: 'device=Realtek',
);

class MissingDetailService extends DiagnosticsService {
  int lookups = 0;

  @override
  Future<LogRecord?> detail(String id) async {
    lookups++;
    record('WARN', 'diagnostics', 'Record not found');
    return null;
  }
}

class CurrentLogsService extends DiagnosticsService {
  int historyRequests = 0;

  @override
  bool get connected => true;

  @override
  Future<List<String>> sessions() async {
    historyRequests++;
    return ['previous-session'];
  }

  @override
  Future<LogPage> query(
    String selected,
    int offset,
    String level,
    String source,
    String search,
  ) async {
    historyRequests++;
    return const LogPage(records: []);
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  testWidgets(
    'log page searches live records without requesting historical sessions',
    (tester) async {
      final service = CurrentLogsService()..session = 'current-session';
      service.record('INFO', 'test', 'Current matching log');
      service.record('INFO', 'test', 'Unrelated record');
      await tester.pumpWidget(
        MaterialApp(
          builder: (_, child) =>
              DiagnosticsOverlay(service: service, child: child!),
          home: const Scaffold(),
        ),
      );
      service.open();
      await tester.pumpAndSettle();
      expect(find.text('Current session · Live'), findsNothing);
      expect(find.text('Open log folder'), findsOneWidget);
      expect(find.text('Load more'), findsNothing);
      await tester.enterText(find.byType(TextField), 'matching');
      await tester.pumpAndSettle(const Duration(milliseconds: 250));
      expect(find.text('Current matching log'), findsOneWidget);
      expect(find.text('Unrelated record'), findsNothing);
      service.record('INFO', 'test', 'New matching log');
      await tester.pumpAndSettle(const Duration(milliseconds: 250));
      expect(find.text('New matching log'), findsOneWidget);
      expect(service.historyRequests, 0);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      await service.shutdown();
    },
  );
  test('structured failures deduplicate; cancellation and background errors stay silent', () async {
    final service = DiagnosticsService();
    service.report(failure('first'));
    await Future<void>.delayed(Duration.zero);
    final first = service.notice.value;
    expect(first!.message, '无法使用此输出设备');
    service.report(failure('event', operation: 'playback_event'));
    expect(identical(service.notice.value, first), isTrue);
    service.notice.value = null;
    service.report(
      failure(
        'cancel',
        category: ErrorCategory.cancelled,
        fingerprint: 'cancel',
      ),
    );
    service.report(StateError('retry'), notify: false);
    service.record('ERROR', 'plugin', 'background error');
    expect(service.notice.value, isNull);
    service.beginOutputOperation();
    service.report(
      failure('other-event', operation: 'playback_event', fingerprint: 'other'),
    );
    expect(service.notice.value, isNull);
    service.endOutputOperation();
    await service.shutdown();
  });

  test(
    'loader failure logs preserve multiline details and survive restart',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'stellatune-diagnostics-test-',
      );
      try {
        final first = DiagnosticsService();
        await first.prepareDirectory(directory.path);
        final id = first.record(
          'ERROR',
          'startup',
          'Rust loader failed',
          error: 'Cookie: secret\nunderlying DLL error',
          stack: StackTrace.fromString('frame A\nframe B'),
        );
        await first.shutdown();
        final next = DiagnosticsService();
        await next.prepareDirectory(directory.path);
        final sessions = await next.sessions();
        expect(sessions, hasLength(1));
        final detail = await next.detail(id);
        expect(detail!.details, contains('frame A\nframe B'));
        expect(detail.details, isNot(contains('secret')));
        final page = await next.query(
          sessions.single,
          0,
          'ERROR',
          'flutter',
          'underlying',
        );
        expect(page.records.single.id, id);
        await next.shutdown();
      } finally {
        await directory.delete(recursive: true);
      }
    },
  );

  test(
    'high frequency collection bounds memory without turning logs into notices',
    () async {
      final service = DiagnosticsService();
      for (var i = 0; i < 5200; i++) {
        service.record('INFO', 'stress', 'entry $i');
      }
      expect(service.records.length, lessThanOrEqualTo(5000));
      expect(service.notice.value, isNull);
      await service.shutdown();
    },
  );

  testWidgets(
    'closed panel never rebuilds the page, global notice opens its detail',
    (tester) async {
      final service = DiagnosticsService();
      var builds = 0;
      await tester.pumpWidget(
        MaterialApp(
          builder: (context, child) =>
              DiagnosticsOverlay(service: service, child: child!),
          home: Builder(
            builder: (_) {
              builds++;
              return const Scaffold(body: Text('Home'));
            },
          ),
        ),
      );
      for (var i = 0; i < 200; i++) {
        service.record('INFO', 'stress', 'tick $i');
      }
      await tester.pump();
      expect(builds, 1);
      service.report(
        StateError('driver full details'),
        operation: 'set_output',
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('driver full details'), findsNothing);
      await tester.tap(find.text('View logs'));
      await tester.pumpAndSettle();
      expect(find.textContaining('driver full details'), findsOneWidget);
      expect(builds, 1);
      await tester.pumpWidget(const SizedBox.shrink());
      await service.shutdown();
    },
  );

  setUpAll(() async {
    await (FontLoader(
      'NotoSansSC',
    )..addFont(rootBundle.load('assets/fonts/NotoSansSC-Regular.ttf'))).load();
    await (FontLoader(
      'MaterialIcons',
    )..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'))).load();
  });

  testWidgets('fullscreen controls have overlays and return preserves scroll', (
    tester,
  ) async {
    final service = DiagnosticsService();
    final scroll = ScrollController();
    await tester.pumpWidget(
      MaterialApp(
        builder: (context, child) =>
            DiagnosticsOverlay(service: service, child: child!),
        home: Scaffold(
          body: ListView.builder(
            controller: scroll,
            itemExtent: 50,
            itemCount: 100,
            itemBuilder: (_, index) => Text('Track $index'),
          ),
        ),
      ),
    );
    scroll.jumpTo(500);
    await tester.pump();
    service.open();
    await tester.pumpAndSettle();
    expect(
      tester.getSize(find.byKey(const ValueKey('diagnostics-page'))),
      tester.view.physicalSize / tester.view.devicePixelRatio,
    );
    final mouse = await tester.createGesture(kind: ui.PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(find.byTooltip('Follow latest')));
    await tester.pump(const Duration(seconds: 1));
    await tester.pumpAndSettle();
    expect(find.text('Follow latest'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await mouse.removePointer();
    await tester.tap(find.text('All levels'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('ERROR').last);
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.tap(find.byTooltip('Back to player'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('diagnostics-page')), findsNothing);
    expect(scroll.offset, 500);
    service.open();
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(service.visible.value, isFalse);
    expect(scroll.offset, 500);
    await tester.pumpWidget(const SizedBox.shrink());
    scroll.dispose();
    await service.shutdown();
  });

  testWidgets(
    'log page animates both ways and reverses without a second navigator',
    (tester) async {
      final service = DiagnosticsService();
      await tester.pumpWidget(
        MaterialApp(
          builder: (_, child) =>
              DiagnosticsOverlay(service: service, child: child!),
          home: const Scaffold(body: Text('Home')),
        ),
      );
      final page = find.byKey(const ValueKey('diagnostics-page'));
      final fade = find.byKey(const ValueKey('diagnostics-page-transition'));
      service.open();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 80));
      final opacity = tester.widget<SharedAxisTransition>(fade).animation.value;
      expect(opacity, greaterThan(0));
      expect(opacity, lessThan(1));
      await tester.pumpAndSettle();
      final pageState = tester.state(page);
      final controls = find.byType(LogWindowControls);
      final controlsPosition = tester.getTopLeft(controls);
      final outgoing = tester.widget<SharedAxisTransition>(
        find.byKey(const ValueKey('diagnostics-underlying-transition')),
      );
      expect(outgoing.transitionType, SharedAxisTransitionType.scaled);
      expect(outgoing.secondaryAnimation.value, 1);
      service.close();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 60));
      expect(page, findsOneWidget);
      expect(tester.getTopLeft(controls), controlsPosition);
      expect(
        tester.widget<SharedAxisTransition>(fade).animation.value,
        lessThan(1),
      );
      service.open();
      await tester.pumpAndSettle();
      expect(identical(tester.state(page), pageState), isTrue);
      expect(find.byType(Navigator), findsNWidgets(2));
      service.close();
      await tester.pumpAndSettle();
      expect(page, findsNothing);
      expect(find.byType(Navigator), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
      await service.shutdown();
    },
  );

  testWidgets('log page respects reduced motion', (tester) async {
    final service = DiagnosticsService();
    await tester.pumpWidget(
      MaterialApp(
        builder: (_, child) => MediaQuery(
          data: const MediaQueryData(disableAnimations: true),
          child: DiagnosticsOverlay(service: service, child: child!),
        ),
        home: const Scaffold(),
      ),
    );
    service.open();
    await tester.pump();
    expect(
      tester
          .widget<SharedAxisTransition>(
            find.byKey(const ValueKey('diagnostics-page-transition')),
          )
          .animation
          .value,
      1,
    );
    service.close();
    await tester.pump();
    expect(find.byKey(const ValueKey('diagnostics-page')), findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    await service.shutdown();
  });

  testWidgets('missing detail retries once without a log feedback loop', (
    tester,
  ) async {
    final service = MissingDetailService();
    await tester.pumpWidget(
      MaterialApp(
        builder: (context, child) =>
            DiagnosticsOverlay(service: service, child: child!),
        home: const Scaffold(),
      ),
    );
    service.open('missing');
    await tester.pumpAndSettle();
    for (var i = 0; i < 20; i++) {
      service.record('INFO', 'player', 'tick $i');
      await tester.pump(const Duration(milliseconds: 100));
    }
    expect(service.lookups, 2);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    await service.shutdown();
  });
  for (final brightness in Brightness.values) {
    for (final width in [540.0, 1200.0]) {
      testWidgets(
        'log panel ${brightness.name} width $width filters, clears and renders long details',
        (tester) async {
          tester.view.devicePixelRatio = 1;
          tester.view.physicalSize = Size(width, 760);
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          final service = DiagnosticsService();
          service.record('INFO', 'player', 'playback ready');
          final id = service.record(
            'ERROR',
            'output',
            'Device failed: ${'long message ' * 80}',
            error:
                'driver unavailable\ncontext: output Realtek\n${'stack frame\n' * 100}',
          );
          await tester.pumpWidget(
            RepaintBoundary(
              key: const ValueKey('capture'),
              child: MaterialApp(
                debugShowCheckedModeBanner: false,
                theme: ThemeData(
                  brightness: brightness,
                  fontFamily: 'NotoSansSC',
                ),
                builder: (context, child) =>
                    DiagnosticsOverlay(service: service, child: child!),
                home: const Scaffold(
                  body: Center(child: Text('Music library')),
                ),
              ),
            ),
          );
          service.open();
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          await tester.enterText(find.byType(TextField), 'Device failed');
          await tester.pump(const Duration(milliseconds: 250));
          expect(find.textContaining('playback ready'), findsNothing);
          service.open(id);
          await tester.pumpAndSettle();
          expect(find.textContaining('driver unavailable'), findsOneWidget);
          expect(tester.takeException(), isNull);
          final boundary = tester.renderObject<RenderRepaintBoundary>(
            find.byKey(const ValueKey('capture')),
          );
          await tester.runAsync(() async {
            final image = await boundary.toImage();
            final bytes = await image.toByteData(
              format: ui.ImageByteFormat.png,
            );
            image.dispose();
            final directory = await Directory('build/visual-review')
                .create(recursive: true);
            await File(
              '${directory.path}/diagnostics-${brightness.name}-$width.png',
            ).writeAsBytes(bytes!.buffer.asUint8List());
          });
          await tester.tap(find.byTooltip('Clear view'));
          await tester.pumpAndSettle();
          expect(service.records, hasLength(2));
          expect(find.byType(LogRecordTile), findsNothing);
          await tester.pumpWidget(const SizedBox.shrink());
          await service.shutdown();
        },
      );
    }
  }
}
