import 'dart:async';
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/output_settings_controller.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/plugins/plugin_settings_controller.dart';

class _Bridge extends Fake implements PlayerBridge {
  final listing = <Completer<String>>[];
  final installing = <String, Completer<String>>{};
  bool delayListings = false;
  int decoderQueries = 0;
  String installedJson = '[]';

  @override
  Future<String> pluginsListInstalledJson({required String dir}) {
    if (!delayListings) return Future.value(installedJson);
    final request = Completer<String>();
    listing.add(request);
    return request.future;
  }

  @override
  Future<String> pluginsInstallFromFile({
    required String dir,
    required String artifactPath,
  }) {
    final request = Completer<String>();
    installing[artifactPath] = request;
    return request.future;
  }

  @override
  Future<List<PluginDescriptor>> pluginsList() async => [];

  @override
  Future<List<SourceCatalogTypeDescriptor>> sourceListTypes() async => [];

  @override
  Future<List<String>> decoderSupportedExtensions() async {
    ++decoderQueries;
    return ['flac'];
  }
}

class _Library extends Fake implements LibraryBridge {
  @override
  Future<List<String>> listDisabledPluginIds() async => [];
}

class _Output extends OutputSettingsController {
  int refreshes = 0;

  @override
  Future<void> changePlugins(Future<void> Function() change) => change();

  @override
  Future<void> refresh() async => ++refreshes;
}

Future<void> _until(bool Function() ready) async {
  for (var attempt = 0; attempt < 100; ++attempt) {
    if (ready()) return;
    await Future<void>.delayed(const Duration(milliseconds: 1));
  }
  fail('Expected asynchronous request did not start');
}

String _plugin(String id) => jsonEncode([
  {'id': id, 'name': id, 'root_dir': 'missing-plugin-test-directory/$id'},
]);

void main() {
  late _Bridge bridge;
  late _Output output;
  late ProviderContainer container;
  PluginSettingsController controller() =>
      container.read(pluginSettingsControllerProvider.notifier);

  setUp(() {
    bridge = _Bridge();
    output = _Output();
    container = ProviderContainer(
      overrides: [
        playerBridgeProvider.overrideWithValue(bridge),
        libraryBridgeProvider.overrideWithValue(_Library()),
        outputSettingsControllerProvider.overrideWith(() => output),
        pluginSettingsControllerProvider.overrideWith(
          () => PluginSettingsController(resolveDirectory: () async => '.'),
        ),
      ],
    );
  });

  tearDown(() => container.dispose());

  test('plugin settings can refresh again after provider rebuild', () async {
    bridge.installedJson = _plugin('before');
    await controller().refresh();
    container.invalidate(pluginSettingsControllerProvider);
    bridge.installedJson = _plugin('after');
    await controller().refresh();
    final state = container.read(pluginSettingsControllerProvider);
    expect(state.plugins.single.id, 'after');
    expect(state.loaded, isTrue);
    expect(state.loading, isFalse);
  });

  for (final fails in [false, true]) {
    test(
      'old plugin refresh is ignored after rebuild (fails: $fails)',
      () async {
        bridge.delayListings = true;
        final first = controller().refresh();
        await _until(() => bridge.listing.length == 1);
        container.invalidate(pluginSettingsControllerProvider);
        final second = controller().refresh();
        await _until(() => bridge.listing.length == 2);
        bridge.listing[1].complete(_plugin('current'));
        await second;

        if (fails) {
          bridge.listing[0].completeError(StateError('old listing failed'));
        } else {
          bridge.listing[0].complete(_plugin('stale'));
        }
        await first;
        final state = container.read(pluginSettingsControllerProvider);
        expect(state.plugins.single.id, 'current');
        expect(state.loading, isFalse);
        expect(state.error, isNull);
      },
    );
  }

  test('old plugin failure cannot clear a new lifetime busy state', () async {
    final old = controller().install('old.zip');
    final oldResult = expectLater(old, throwsStateError);
    await _until(() => bridge.installing.containsKey('old.zip'));
    container.invalidate(pluginSettingsControllerProvider);
    final current = controller().install('current.zip');
    await _until(() => bridge.installing.containsKey('current.zip'));

    bridge.installing['old.zip']!.completeError(
      StateError('old install failed'),
    );
    await oldResult;
    var state = container.read(pluginSettingsControllerProvider);
    expect(state.busy, isTrue);
    expect(state.error, isNull);
    expect(output.refreshes, 0);

    bridge.installing['current.zip']!.complete('installed');
    await current;
    state = container.read(pluginSettingsControllerProvider);
    expect(state.busy, isFalse);
    expect(state.loaded, isTrue);
    expect(output.refreshes, 1);
  });

  test(
    'disposing during installation prevents post-disposal UI work',
    () async {
      final pending = controller().install('pending.zip');
      await _until(() => bridge.installing.containsKey('pending.zip'));
      container.dispose();
      bridge.installing['pending.zip']!.complete('installed');
      await pending;
      expect(output.refreshes, 0);
      expect(bridge.decoderQueries, 0);
    },
  );

  test('failed plugin directory lookup can be retried', () async {
    container.dispose();
    var requests = 0;
    container = ProviderContainer(
      overrides: [
        playerBridgeProvider.overrideWithValue(bridge),
        libraryBridgeProvider.overrideWithValue(_Library()),
        pluginSettingsControllerProvider.overrideWith(
          () => PluginSettingsController(
            resolveDirectory: () async {
              if (++requests == 1) throw StateError('temporary path failure');
              return '.';
            },
          ),
        ),
      ],
    );
    await controller().refresh();
    expect(container.read(pluginSettingsControllerProvider).error, isNotNull);
    await controller().refresh();
    final state = container.read(pluginSettingsControllerProvider);
    expect(state.loaded, isTrue);
    expect(state.error, isNull);
    expect(requests, 2);
  });
}
