import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/app_bootstrap.dart';
import 'package:stellatune/app/app_bootstrap_services.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/app/startup_app.dart';
import 'package:stellatune/bridge/bridge.dart';

class _Player extends Fake implements PlayerBridge {
  _Player(this.calls, {this.failDispose = false});
  final List<String> calls;
  final bool failDispose;

  @override
  Future<void> dispose() async {
    calls.add('dispose player');
    if (failDispose) throw StateError('dispose failed');
  }
}

class _Services extends AppBootstrapServices {
  _Services({this.failRuntime = false, this.failDispose = false});
  final bool failRuntime;
  final bool failDispose;
  final calls = <String>[];

  @override
  Future<void> initializeRuntime() async {
    calls.add('load runtime');
    if (failRuntime) throw StateError('Rust library not found');
  }

  @override
  Future<PlayerBridge> createPlayer() async =>
      _Player(calls, failDispose: failDispose);

  @override
  Future<SettingsStore> openSettings() async {
    calls.add('open settings');
    throw StateError('Cannot open settings store');
  }

  @override
  Future<void> shutdownRuntime() async {
    calls.add('shutdown runtime');
  }

  @override
  Future<void> closeSettings() async {
    calls.add('close settings');
  }
}

void main() {
  testWidgets(
    'startup paints loading before work completes, then shows the app',
    (tester) async {
      final gate = Completer<Widget>();
      var loads = 0;
      await tester.pumpWidget(
        StartupApp(
          loadApp: () {
            loads++;
            return gate.future;
          },
          onExit: () async {},
        ),
      );
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
      expect(find.text('Exit'), findsOneWidget);
      await tester.pump();
      expect(loads, 1);
      gate.complete(const MaterialApp(home: Text('Application ready')));
      await tester.pumpAndSettle();
      expect(find.text('Application ready'), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsNothing);
    },
  );

  testWidgets(
    'missing Rust library renders error and exit without invoking shutdown',
    (tester) async {
      final services = _Services(failRuntime: true);
      var exits = 0;
      await tester.pumpWidget(
        StartupApp(
          loadApp: () async {
            await bootstrapApp(services: services);
            return const SizedBox();
          },
          onExit: () async {
            exits++;
          },
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Unable to start Stellatune'), findsOneWidget);
      expect(find.textContaining('Rust library not found'), findsOneWidget);
      expect(services.calls, ['load runtime']);
      await tester.tap(find.text('Exit'));
      expect(exits, 1);
    },
  );

  testWidgets(
    'settings failure closes initialized resources before displaying failure',
    (tester) async {
      final services = _Services();
      await tester.pumpWidget(
        StartupApp(
          loadApp: () async {
            await bootstrapApp(services: services);
            return const SizedBox();
          },
          onExit: () async {},
        ),
      );
      await tester.pumpAndSettle();
      expect(find.textContaining('Cannot open settings store'), findsOneWidget);
      expect(services.calls, [
        'load runtime',
        'open settings',
        'dispose player',
        'shutdown runtime',
        'close settings',
      ]);
    },
  );

  test('cleanup failure still releases the other resources and preserves original error', () async {
    final services = _Services(failDispose: true);
    await expectLater(
      bootstrapApp(services: services),
      throwsA(
        isA<StateError>().having(
          (error) => error.message,
          'message',
          'Cannot open settings store',
        ),
      ),
    );
    expect(services.calls, [
      'load runtime',
      'open settings',
      'dispose player',
      'shutdown runtime',
      'close settings',
    ]);
  });
}
