import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/ui/theme/desktop_theme.dart';

class DeferredSettingsStore extends SettingsStore {
  final saved = Completer<void>();
  @override
  Future<void> setVolume(double value) => saved.future;
}

void main() {
  late Directory hiveDir;

  setUp(() async {
    hiveDir = await Directory.systemTemp.createTemp('stellatune_test_');
    Hive.init(hiveDir.path);
    await Hive.openBox('settings');
  });

  tearDown(() async {
    if (Hive.isBoxOpen('settings')) {
      await Hive.box('settings').deleteFromDisk();
    }
    await Hive.close();
    if (await hiveDir.exists()) {
      await hiveDir.delete(recursive: true);
    }
  });

  test(
    'settingsStoreProvider notifies select listeners after mutations',
    () async {
      final store = SettingsStore();
      final container = ProviderContainer(
        overrides: [settingsStoreServiceProvider.overrideWithValue(store)],
      );
      addTearDown(container.dispose);

      final observedThemes = <DesktopThemePreset>[];
      final observedDeviceIds = <String?>[];

      final themeSub = container.listen<DesktopThemePreset>(
        settingsStoreProvider.select((s) => s.desktopTheme),
        (previous, next) => observedThemes.add(next),
        fireImmediately: true,
      );
      final deviceSub = container.listen<String?>(
        settingsStoreProvider.select((s) => s.selectedDeviceId),
        (previous, next) => observedDeviceIds.add(next),
        fireImmediately: true,
      );
      addTearDown(themeSub.close);
      addTearDown(deviceSub.close);

      final controller = container.read(settingsStoreProvider.notifier);

      await controller.setDesktopTheme(DesktopThemePreset.lavender);
      await controller.setSelectedDeviceId('device-42');

      expect(
        container.read(settingsStoreProvider).desktopTheme,
        DesktopThemePreset.lavender,
      );
      expect(
        container.read(settingsStoreProvider).selectedDeviceId,
        'device-42',
      );
      expect(observedThemes, [
        DesktopThemePreset.daylight,
        DesktopThemePreset.lavender,
      ]);
      expect(observedDeviceIds, [null, 'device-42']);
    },
  );

  test(
    'catalog column widths survive reopening and ignore invalid values',
    () async {
      final store = SettingsStore();
      expect(store.catalogColumnWidths, isEmpty);
      await store.setCatalogColumnWidths({
        'title': 4.5,
        'artist': 2.0,
        'duration': 60,
      });
      await Hive.box('settings').close();
      await Hive.openBox('settings');
      expect(SettingsStore().catalogColumnWidths, {
        'title': 4.5,
        'artist': 2.0,
        'duration': 60,
      });
      await Hive.box('settings').put('catalog_column_widths', {
        'title': 'bad',
        'artist': -3,
        'album': double.nan,
      });
      expect(store.catalogColumnWidths, isEmpty);
    },
  );

  test(
    'playback latency defaults, persists by name and survives reopening',
    () async {
      final store = SettingsStore();
      expect(store.playbackLatency, PlaybackLatency.medium);
      final container = ProviderContainer(
        overrides: [settingsStoreServiceProvider.overrideWithValue(store)],
      );
      addTearDown(container.dispose);
      await container
          .read(settingsStoreProvider.notifier)
          .setPlaybackLatency(PlaybackLatency.high);
      expect(
        container.read(settingsStoreProvider).playbackLatency,
        PlaybackLatency.high,
      );
      await Hive.box('settings').close();
      await Hive.openBox('settings');
      expect(SettingsStore().readState().playbackLatency, PlaybackLatency.high);
      await Hive.box('settings')
          .put('playback_latency', 'unknown-future-preset');
      expect(store.playbackLatency, PlaybackLatency.medium);
    },
  );

  test('desktop theme persists and ignores the retired theme mode', () async {
    final store = SettingsStore();
    final container = ProviderContainer(
      overrides: [settingsStoreServiceProvider.overrideWithValue(store)],
    );
    addTearDown(container.dispose);
    expect(store.desktopTheme, DesktopThemePreset.daylight);
    final controller = container.read(settingsStoreProvider.notifier);
    await controller.setDesktopTheme(DesktopThemePreset.celadon);
    expect(
      container.read(desktopPaletteProvider),
      same(DesktopThemePreset.celadon.palette),
    );
    await controller.setVolume(.4);
    await Hive.box('settings').put('theme_mode', 'dark');
    expect(
      container.read(desktopPaletteProvider),
      same(DesktopThemePreset.celadon.palette),
    );
    await Hive.box('settings').close();
    await Hive.openBox('settings');
    expect(store.readState().desktopTheme, DesktopThemePreset.celadon);
    await Hive.box('settings').put('desktop_theme', 'unknown-future-theme');
    expect(store.desktopTheme, DesktopThemePreset.daylight);
  });

  test(
    'pending setting writes do not publish into a disposed provider',
    () async {
      final store = DeferredSettingsStore();
      final container = ProviderContainer(
        overrides: [settingsStoreServiceProvider.overrideWithValue(store)],
      );
      final writing = container
          .read(settingsStoreProvider.notifier)
          .setVolume(.5);
      container.dispose();
      store.saved.complete();
      await writing;
    },
  );

  test('a pending write cannot replace rebuilt settings state', () async {
    final store = DeferredSettingsStore();
    final container = ProviderContainer(
      overrides: [settingsStoreServiceProvider.overrideWithValue(store)],
    );
    addTearDown(container.dispose);
    final writing = container
        .read(settingsStoreProvider.notifier)
        .setVolume(.5);
    container.invalidate(settingsStoreProvider);
    final rebuilt = container.read(settingsStoreProvider);
    store.saved.complete();
    await writing;
    expect(container.read(settingsStoreProvider), same(rebuilt));
  });
}
