import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/transcode/transcode_flow.dart';
import 'package:stellatune/transcode/transcode_task_controller.dart';

TranscodeProgressEvent _event(String phase, {String? path}) =>
    TranscodeProgressEvent(
      phase: phase,
      outputPath: path,
      processedFrames: BigInt.from(10),
      totalFrames: BigInt.from(100),
      writtenBytes: BigInt.from(80),
    );

void main() {
  late StreamController<TranscodeProgressEvent> stream;
  late TranscodeTaskController task;
  late int canceled;
  late int subscriptionsClosed;
  setUp(() {
    canceled = 0;
    subscriptionsClosed = 0;
    stream = StreamController<TranscodeProgressEvent>(
      sync: true,
      onCancel: () => subscriptionsClosed++,
    );
    task = TranscodeTaskController(
      start: () => stream.stream,
      cancelTask: () async {
        canceled++;
      },
      outputPath: 'initial.flac',
    );
  });
  tearDown(() {
    task.dispose();
    unawaited(stream.close());
  });

  test(
    'completion keeps final output path and closes the subscription once',
    () async {
      final done = task.run();
      expect(identical(task.run(), done), isTrue);
      stream.add(_event('progress', path: 'resolved.flac'));
      stream.add(_event('completed'));
      stream.add(_event('progress', path: 'late.flac'));
      final outcome = await done;
      expect(outcome.result, TranscodeResult.completed);
      expect(outcome.outputPath, 'resolved.flac');
      expect(task.progress?.phase, 'completed');
      expect(stream.hasListener, isFalse);
      expect(subscriptionsClosed, 1);
      await task.close();
      expect(canceled, 0);
    },
  );

  test(
    'cancel requests coalesce and wait for the real terminal event',
    () async {
      final done = task.run();
      await Future.wait([task.cancel(), task.cancel()]);
      expect(canceled, 1);
      expect(task.canceling, isTrue);
      expect(task.outcome, isNull);
      stream.add(_event('canceled'));
      expect((await done).result, TranscodeResult.canceled);
      expect(subscriptionsClosed, 1);
    },
  );

  test('cancel failure remains visible and a later cancel can retry', () async {
    task.dispose();
    task = TranscodeTaskController(
      start: () => stream.stream,
      outputPath: 'out.flac',
      cancelTask: () async {
        canceled++;
        if (canceled == 1) throw StateError('cancel RPC failed');
      },
    );
    final done = task.run();
    await task.cancel();
    expect(task.cancelError, isA<StateError>());
    expect(task.canceling, isFalse);
    expect(stream.hasListener, isTrue);
    await task.cancel();
    expect(canceled, 2);
    expect(task.cancelError, isNull);
    stream.add(_event('canceled'));
    await done;
  });

  test('stream errors settle the task and close the listener', () async {
    final done = task.run();
    stream.addError(StateError('decoder crashed'));
    final outcome = await done;
    expect(outcome.result, TranscodeResult.failed);
    expect(outcome.error.toString(), contains('decoder crashed'));
    expect(stream.hasListener, isFalse);
    expect(subscriptionsClosed, 1);
  });

  test('closing without a terminal event is a failure, not success', () async {
    final done = task.run();
    await stream.close();
    expect((await done).result, TranscodeResult.failed);
    expect(task.outcome?.error.toString(), contains('terminal result'));
  });

  test(
    'a synchronous start failure still settles and can be disposed',
    () async {
      task.dispose();
      task = TranscodeTaskController(
        start: () => throw StateError('not available'),
        cancelTask: () async {
          canceled++;
        },
        outputPath: 'out.flac',
      );
      expect((await task.run()).result, TranscodeResult.failed);
      expect(canceled, 0);
    },
  );

  test(
    'disposing a running task cancels the backend and detaches once',
    () async {
      var notifications = 0;
      task.addListener(() => notifications++);
      final done = task.run();
      task.dispose();
      task.dispose();
      await task.close();
      expect((await done).result, TranscodeResult.canceled);
      expect(canceled, 1);
      expect(subscriptionsClosed, 1);
      expect(notifications, 0);
    },
  );

  Future<void> mountProgress(
    WidgetTester tester,
    GlobalKey<NavigatorState> navigatorKey,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        navigatorKey: navigatorKey,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: Builder(
            builder: (context) => TextButton(
              onPressed: () => showTranscodeProgress(
                context,
                controller: task,
                encoderName: 'FLAC',
                sourceName: 'Track',
              ),
              child: const Text('Start'),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Start'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
  }

  testWidgets('removing the actual progress dialog stops its task', (
    tester,
  ) async {
    final navigatorKey = GlobalKey<NavigatorState>();
    await mountProgress(tester, navigatorKey);
    expect(stream.hasListener, isTrue);
    navigatorKey.currentState!.pop();
    await tester.pumpAndSettle();
    expect(canceled, 1);
    expect(subscriptionsClosed, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'completion removes its own dialog without popping a newer route',
    (tester) async {
      final navigatorKey = GlobalKey<NavigatorState>();
      await mountProgress(tester, navigatorKey);
      unawaited(
        navigatorKey.currentState!.push(
          MaterialPageRoute<void>(
            builder: (_) => const Scaffold(body: Text('Newer route')),
          ),
        ),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      stream.add(_event('completed'));
      await tester.pumpAndSettle();
      expect(find.text('Newer route'), findsOneWidget);
      expect(canceled, 0);
      expect(subscriptionsClosed, 1);
      expect(tester.takeException(), isNull);
    },
  );
}
