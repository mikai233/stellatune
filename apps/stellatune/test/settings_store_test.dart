import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hive/hive.dart';
import 'package:stellatune/app/providers.dart';

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

      final observedThemeModes = <ThemeMode>[];
      final observedDeviceIds = <String?>[];

      final themeSub = container.listen<ThemeMode>(
        settingsStoreProvider.select((s) => s.themeMode),
        (previous, next) => observedThemeModes.add(next),
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

      await controller.setThemeMode(ThemeMode.dark);
      await controller.setSelectedDeviceId('device-42');

      expect(container.read(settingsStoreProvider).themeMode, ThemeMode.dark);
      expect(
        container.read(settingsStoreProvider).selectedDeviceId,
        'device-42',
      );
      expect(observedThemeModes, [ThemeMode.system, ThemeMode.dark]);
      expect(observedDeviceIds, [null, 'device-42']);
    },
  );
}
