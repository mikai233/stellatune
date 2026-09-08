import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:logger/logger.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/ui/diagnostics/diagnostics_overlay.dart';
import 'package:stellatune/ui/widgets/open_container_shader_warmup.dart';

class _WarmupLog extends LogOutput {
  final lines = <String>[];

  @override
  void output(OutputEvent event) => lines.addAll(event.lines);
}

void main() {
  testWidgets('startup warmup does not steal the app HeroController', (
    tester,
  ) async {
    final service = DiagnosticsService();
    final output = _WarmupLog();
    final logger = Logger(output: output, printer: SimplePrinter());
    final navigator = GlobalKey<NavigatorState>();
    await tester.pumpWidget(
      ProviderScope(
        overrides: [loggerProvider.overrideWithValue(logger)],
        child: MaterialApp(
          navigatorKey: navigator,
          builder: (_, child) =>
              DiagnosticsOverlay(service: service, child: child!),
          home: const Scaffold(
            body: OpenContainerShaderWarmup(
              useFullScreenPreview: bool.fromEnvironment('WARMUP_FULLSCREEN'),
              cycles: 3,
              paletteResolveAttempts: 0,
              transitionDuration: Duration(milliseconds: 40),
            ),
          ),
        ),
      ),
    );
    // Exercise the real OverlayEntry and OpenContainer push/pop cycle, including
    // startup while the user has the fullscreen diagnostics page open.
    service.open();
    for (var i = 0; i < 40; i++) {
      await tester.pump(const Duration(milliseconds: 50));
      expect(tester.takeException(), isNull);
    }
    expect(
      output.lines.any((line) => line.contains('warmup completed')),
      isTrue,
    );
    expect(output.lines.any((line) => line.contains('timeout')), isFalse);
    service.close();
    await tester.pumpAndSettle();
    navigator.currentState!.push(
      MaterialPageRoute<void>(
        builder: (_) => const Scaffold(body: Text('Playback details')),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Playback details'), findsOneWidget);
    navigator.currentState!.pop();
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    await service.shutdown();
    await logger.close();
  });
}
