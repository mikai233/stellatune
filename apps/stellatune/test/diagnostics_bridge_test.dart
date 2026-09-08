import 'dart:async';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/bridge/api/diagnostics.dart' as api;
import 'package:stellatune/bridge/api/error.dart';
import 'package:stellatune/bridge/frb_generated.dart';
import 'package:stellatune/bridge/third_party/stellatune_backend_api/diagnostics/model.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final library = Platform.environment['STELLATUNE_TEST_DIAGNOSTICS_DLL'];
  test(
    'native FFI transports typed errors, batched logs and complete details',
    () async {
      await StellatuneApi.init(externalLibrary: ExternalLibrary.open(library!));
      final directory = await Directory(
        'build/diagnostics-native-${DateTime.now().microsecondsSinceEpoch}',
      ).create(recursive: true);
      final session = await api.diagnosticsInitialize(
        logDir: directory.absolute.path,
      );
      final received = Completer<void>();
      final subscription = api.diagnosticsEvents().listen((batch) {
        if (!received.isCompleted &&
            batch.records.any((r) => r.id == 'flutter:ffi-test:1')) {
          received.complete();
        }
      });
      try {
        await api.diagnosticsAppend(
          records: [
            LogRecord(
              id: 'flutter:ffi-test:1',
              session: session,
              timestampMs: DateTime.now().millisecondsSinceEpoch.toDouble(),
              level: 'ERROR',
              source: 'flutter',
              target: 'ffi-test',
              message: 'native bridge smoke',
              details: 'line one\nline two\nCookie: secret',
            ),
          ],
        );
        await received.future.timeout(const Duration(seconds: 5));
        final detail = await api.diagnosticsDetail(id: 'flutter:ffi-test:1');
        expect(detail.details, contains('line one\nline two'));
        expect(detail.details, isNot(contains('secret')));
        AppError? failure;
        try {
          await api.diagnosticsDetail(id: 'missing-record');
        } on AppError catch (error) {
          failure = error;
        }
        expect(failure, isNotNull);
        expect(failure!.operation, 'diagnostics_detail');
        final diagnostic = await api.diagnosticsDetail(
          id: failure.diagnosticId,
        );
        expect(diagnostic.details, contains('Rust backtrace:'));
        await api.diagnosticsExport(
          session: session,
          destination: '${directory.path}/export.txt',
        );
        expect(
          await File('${directory.path}/export.txt').readAsString(),
          contains('line one\nline two'),
        );
        await api.diagnosticsFlush();
      } finally {
        await subscription.cancel();
      }
    },
    skip: library == null
        ? 'Set STELLATUNE_TEST_DIAGNOSTICS_DLL to the built application DLL'
        : false,
  );
}
