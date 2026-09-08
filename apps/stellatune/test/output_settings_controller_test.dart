import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:stellatune/app/output_settings_controller.dart';
import 'package:stellatune/app/output_settings_runtime.dart';
import 'package:stellatune/app/output_settings_values.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_output_section.dart';

OutputSinkTypeDescriptor _type(String id) => OutputSinkTypeDescriptor(
  pluginId: id,
  pluginName: id,
  typeId: 'output',
  displayName: id,
  configSchemaJson: '{}',
  defaultConfigJson: '{}',
);
String _key(String id) => OutputSettingsValues.pluginBackendKey(id, 'output');
const _oldRoute = OutputSinkRoute(
  pluginId: 'A',
  typeId: 'output',
  configJson: '{}',
  targetJson: '{"id":"old"}',
);

class _Bridge implements PlayerBridge {
  final targetRequests = <String, Completer<String>>{};
  final routes = <OutputSinkRoute?>[];
  final deviceCalls = <String?>[];
  final optionCalls = <ResampleQuality>[];
  final matchRateCalls = <bool>[];
  bool failDevice = false;
  bool failOptions = false;
  bool failRoute = false;
  Completer<void>? opening;
  Completer<void>? opened;
  Completer<List<AudioDevice>>? deviceEnumeration;
  OutputSinkRoute? currentRoute;

  @override
  Future<List<OutputSinkTypeDescriptor>> outputSinkListTypes() async => [
    _type('A'),
    _type('B'),
  ];
  @override
  Future<List<AudioDevice>> refreshDevices() async =>
      deviceEnumeration?.future ??
      const [
        AudioDevice(
          backend: AudioBackend.shared,
          id: 'device-1',
          name: 'Device 1',
        ),
      ];
  @override
  Future<String> outputSinkListTargetsJson({
    required String pluginId,
    required String typeId,
    required String configJson,
  }) => targetRequests[pluginId]?.future ?? Future.value('[{"id":"old"}]');
  @override
  Future<void> setOutputDevice({
    required AudioBackend backend,
    String? deviceId,
  }) async {
    deviceCalls.add(deviceId);
    opened?.complete();
    await opening?.future;
    if (failDevice) throw StateError('device open failed');
    currentRoute = null;
  }

  @override
  Future<void> setOutputSinkRoute(OutputSinkRoute route) async {
    if (failRoute) throw StateError('plugin open failed');
    routes.add(route);
    currentRoute = route;
  }

  @override
  Future<void> clearOutputSinkRoute() async {
    routes.add(null);
    currentRoute = null;
  }

  @override
  Future<void> setOutputOptions({
    required bool matchTrackSampleRate,
    required bool gaplessPlayback,
    required bool seekTrackFade,
    required ResampleQuality resampleQuality,
  }) async {
    optionCalls.add(resampleQuality);
    matchRateCalls.add(matchTrackSampleRate);
    if (failOptions) throw StateError('options rejected');
  }

  @override
  Future<void> setPlaybackLatency(PlaybackLatency profile) async {}
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _FailingSaveStore extends SettingsStore {
  @override
  Future<void> saveOutputSelection({
    required AudioBackend backend,
    required String? deviceId,
    required OutputSinkRoute? route,
  }) async => throw FileSystemException('settings write failed');
}

void main() {
  late Directory directory;
  late SettingsStore store;
  late _Bridge bridge;
  late ProviderContainer container;
  OutputSettingsController controller() =>
      container.read(outputSettingsControllerProvider.notifier);

  setUp(() async {
    directory = await Directory.systemTemp.createTemp('output-settings-test-');
    Hive.init(directory.path);
    await Hive.openBox('settings');
    store = SettingsStore();
    bridge = _Bridge();
    container = ProviderContainer(
      overrides: [
        settingsStoreServiceProvider.overrideWithValue(store),
        playerBridgeProvider.overrideWithValue(bridge),
      ],
    );
  });
  tearDown(() async {
    container.dispose();
    await Hive.close();
    await directory.delete(recursive: true);
  });

  test('provider rebuild accepts subsequent output changes', () async {
    await controller().selectDevice('before');
    container.invalidate(outputSettingsControllerProvider);
    await controller().selectDevice('after');
    expect(store.selectedDeviceId, 'after');
    expect(bridge.deviceCalls, ['before', 'after']);
    expect(container.read(outputSettingsControllerProvider).applying, isFalse);
  });

  test(
    'old device refresh failure preserves a newer pending backend',
    () async {
      await controller().refresh();
      bridge.deviceEnumeration = Completer<List<AudioDevice>>();
      final refresh = controller().refresh();
      bridge.targetRequests['B'] = Completer<String>();
      final change = controller().selectBackend(_key('B'));

      bridge.deviceEnumeration!.completeError(StateError('old enumeration'));
      await refresh;
      expect(controller().selection.backendKey, _key('B'));
      var state = container.read(outputSettingsControllerProvider);
      expect(state.loading, isFalse);
      expect(state.loadingTargets, isTrue);
      expect(state.error, isNull);

      bridge.targetRequests['B']!.complete('[{"id":"B-device"}]');
      await change;
      state = container.read(outputSettingsControllerProvider);
      expect(store.outputSinkRoute?.pluginId, 'B');
      expect(state.loadingTargets, isFalse);
      expect(state.error, isNull);
    },
  );

  for (final fails in [false, true]) {
    test('rebuild rejects an old target request (fails: $fails)', () async {
      await controller().refresh();
      bridge.targetRequests['A'] = Completer<String>();
      final old = controller().selectBackend(_key('A'));
      container.invalidate(outputSettingsControllerProvider);
      await controller().refresh();
      await controller().selectBackend(_key('B'));

      if (fails) {
        bridge.targetRequests['A']!.completeError(StateError('old lifetime'));
      } else {
        bridge.targetRequests['A']!.complete('[{"id":"stale"}]');
      }
      await old;
      expect(store.outputSinkRoute?.pluginId, 'B');
      final state = container.read(outputSettingsControllerProvider);
      expect(state.targets, [
        {'id': 'old'},
      ]);
      expect(state.error, isNull);
      expect(bridge.routes.map((route) => route?.pluginId), ['B']);
    });
  }

  test(
    'rebuild preserves ordering behind a runtime change in progress',
    () async {
      bridge.opening = Completer<void>();
      bridge.opened = Completer<void>();
      final first = controller().selectDevice('before');
      await bridge.opened!.future;
      final gate = bridge.opening!;
      bridge.opened = null;
      bridge.opening = null;

      container.invalidate(outputSettingsControllerProvider);
      final second = controller().selectDevice('after');
      await Future<void>.delayed(Duration.zero);
      expect(bridge.deviceCalls, ['before']);

      gate.complete();
      await Future.wait([first, second]);
      expect(bridge.deviceCalls, ['before', 'after']);
      expect(store.selectedDeviceId, 'after');
      expect(controller().selection.deviceId, 'after');
      expect(container.read(outputSettingsControllerProvider).error, isNull);
    },
  );

  test(
    'device open failure keeps the confirmed backend, route and saved device',
    () async {
      await store.saveOutputSelection(
        backend: AudioBackend.wasapiExclusive,
        deviceId: 'old-device',
        route: _oldRoute,
      );
      bridge.currentRoute = _oldRoute;
      bridge.failDevice = true;
      await controller().selectBackend('local:shared');
      expect(store.outputSinkRoute, _oldRoute);
      expect(store.selectedBackend, AudioBackend.wasapiExclusive);
      expect(store.selectedDeviceId, 'old-device');
      expect(container.read(outputSettingsControllerProvider).draft, isNull);
      expect(controller().selection.backendKey, _key('A'));
      expect(bridge.currentRoute, _oldRoute);
      // Clearing a native route first would lose Rust's transactional rollback.
      expect(bridge.routes, isEmpty);
      expect(
        container.read(outputSettingsControllerProvider).error,
        isA<StateError>(),
      );
    },
  );

  test(
    'options failure never writes an unconfirmed quality or fade setting',
    () async {
      bridge.failOptions = true;
      final before = store.readState();
      await controller().setOptions(
        resampleQuality: ResampleQuality.fast,
        seekTrackFade: !before.seekTrackFade,
      );
      expect(store.resampleQuality, before.resampleQuality);
      expect(store.seekTrackFade, before.seekTrackFade);
      expect(container.read(outputSettingsControllerProvider).error, isNotNull);
      bridge.failOptions = false;
      await controller().setOptions(resampleQuality: ResampleQuality.balanced);
      expect(store.resampleQuality, ResampleQuality.balanced);
      expect(container.read(outputSettingsControllerProvider).error, isNull);
    },
  );

  for (final fails in [false, true]) {
    test('late A enumeration cannot overwrite B (A fails: $fails)', () async {
      await controller().refresh();
      bridge.targetRequests['A'] = Completer<String>();
      bridge.targetRequests['B'] = Completer<String>();
      final first = controller().selectBackend(_key('A'));
      final second = controller().selectBackend(_key('B'));
      bridge.targetRequests['B']!.complete('[{"id":"B-device"}]');
      await second;
      if (fails) {
        bridge.targetRequests['A']!.completeError(StateError('old request'));
      } else {
        bridge.targetRequests['A']!.complete('[{"id":"A-device"}]');
      }
      await first;
      expect(store.outputSinkRoute?.pluginId, 'B');
      expect(controller().selection.backendKey, _key('B'));
      final state = container.read(outputSettingsControllerProvider);
      expect(state.targets, [
        {'id': 'B-device'},
      ]);
      expect(state.loadingTargets, isFalse);
      expect(state.error, isNull);
      expect(bridge.routes.map((r) => r?.pluginId), ['B']);
    });
  }

  test(
    'writes only after acknowledgement even when settings listeners leave',
    () async {
      bridge.opening = Completer<void>();
      bridge.opened = Completer<void>();
      final subscription = container.listen(
        outputSettingsControllerProvider,
        (_, _) {},
      );
      final operation = controller().selectDevice('device-1');
      await bridge.opened!.future;
      expect(store.selectedDeviceId, isNull);
      subscription.close();
      await container.pump();
      bridge.opening!.complete();
      await operation;
      expect(store.selectedDeviceId, 'device-1');
      expect(bridge.deviceCalls, ['device-1']);
    },
  );

  test(
    'persistence failure rolls the runtime back to the previous plugin route',
    () async {
      await store.setOutputSinkRoute(_oldRoute);
      bridge.currentRoute = _oldRoute;
      container.dispose();
      container = ProviderContainer(
        overrides: [
          settingsStoreServiceProvider.overrideWithValue(_FailingSaveStore()),
          playerBridgeProvider.overrideWithValue(bridge),
        ],
      );
      await controller().selectBackend('local:shared');
      expect(bridge.currentRoute, _oldRoute);
      expect(store.outputSinkRoute, _oldRoute);
      expect(
        container.read(outputSettingsControllerProvider).error,
        isA<FileSystemException>(),
      );
    },
  );

  test(
    'failed package change synchronizes the native-output fallback',
    () async {
      await store.setOutputSinkRoute(_oldRoute);
      bridge.currentRoute = _oldRoute;
      await expectLater(
        controller().changePlugins(() async {
          bridge.currentRoute =
              null; // Rust releases native output before installing.
          throw StateError('invalid package');
        }),
        throwsStateError,
      );
      expect(store.outputSinkRoute, isNull);
      expect(bridge.currentRoute, isNull);
      expect(controller().selection.backendKey, 'local:shared');
    },
  );

  test(
    'bootstrap and later settings changes share the same saved route format',
    () async {
      await store.setOutputSinkRoute(_oldRoute);
      await restorePersistedOutputSettings(bridge: bridge, settings: store);
      expect(store.outputSinkRoute, _oldRoute);
      expect(bridge.currentRoute, _oldRoute);
      await controller().selectBackend('local:shared');
      expect(store.outputSinkRoute, isNull);
      await Hive.box('settings').close();
      await Hive.openBox('settings');
      expect(store.selectedBackend, AudioBackend.shared);
      expect(store.outputSinkRoute, isNull);
    },
  );

  testWidgets('native output exposes and persists match-track sample rate', (
    tester,
  ) async {
    await tester.runAsync(() async {
      await store.setOutputSinkRoute(_oldRoute);
      await controller().refresh();
    });
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: const Scaffold(
            body: SingleChildScrollView(child: SettingsOutputSection()),
          ),
        ),
      ),
    );
    final label = AppLocalizations.of(
      tester.element(find.byType(SettingsOutputSection)),
    )!.settingsMatchTrackSampleRate;
    expect(find.text(label), findsOneWidget);
    final toggle = find.byType(Switch).first;
    await tester.ensureVisible(toggle);
    await tester.runAsync(() async {
      final confirmed = Completer<void>();
      final subscription = container.listen(settingsStoreProvider, (_, next) {
        if (next.matchTrackSampleRate && !confirmed.isCompleted) {
          confirmed.complete();
        }
      });
      try {
        await tester.tap(toggle);
        await confirmed.future.timeout(const Duration(seconds: 2));
      } finally {
        subscription.close();
      }
    });
    await tester.pump();
    expect(bridge.matchRateCalls, [true]);
    expect(store.matchTrackSampleRate, isTrue);
    expect(tester.widget<Switch>(toggle).value, isTrue);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'opening and removing the output panel does not change the route',
    (tester) async {
      await tester.runAsync(() async {
        await store.setOutputSinkRoute(_oldRoute);
        bridge.currentRoute = _oldRoute;
        await controller().refresh();
      });
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: const Scaffold(
              body: SingleChildScrollView(child: SettingsOutputSection()),
            ),
          ),
        ),
      );
      await tester.pump();
      await tester.pumpWidget(const SizedBox.shrink());
      expect(bridge.deviceCalls, isEmpty);
      expect(bridge.routes, isEmpty);
      expect(store.outputSinkRoute, _oldRoute);
      expect(tester.takeException(), isNull);
    },
  );
}
