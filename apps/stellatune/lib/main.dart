import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/library/library_paths.dart';
import 'package:stellatune/platform/rust_runtime.dart';
import 'package:stellatune/ui/app.dart';
import 'package:window_manager/window_manager.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Set up window manager for desktop platforms
  if (Platform.isWindows || Platform.isLinux || Platform.isMacOS) {
    await windowManager.ensureInitialized();
    const windowOptions = WindowOptions(
      minimumSize: Size(900, 700),
      size: Size(1000, 720),
      center: true,
      title: 'Stellatune',
      titleBarStyle: TitleBarStyle.hidden,
    );
    await windowManager.waitUntilReadyToShow(windowOptions, () async {
      await windowManager.show();
      await windowManager.focus();
    });
  }

  await initRustRuntime();
  final bridge = await PlayerBridge.create();
  final settings = await SettingsStore.open();
  // Sync persisted audio backend/device into Rust early so playback uses the right output
  // even before the Settings page is opened.
  try {
    await bridge.setOutputDevice(
      backend: settings.selectedBackend,
      deviceId: settings.selectedDeviceId,
    );
    await bridge.setOutputOptions(
      matchTrackSampleRate: settings.matchTrackSampleRate,
      gaplessPlayback: settings.gaplessPlayback,
    );
  } catch (_) {}
  final dbPath = await defaultLibraryDbPath();
  final pluginDir = await defaultPluginDir();
  await Directory(pluginDir).create(recursive: true);

  final library = await LibraryBridge.create(
    dbPath: dbPath,
    disabledPluginIds: settings.disabledPluginIds.toList(),
  );

  final coverDir = p.join(p.dirname(dbPath), 'covers');

  // Desktop-only today, but safe to call (it just loads from the folder).
  try {
    await bridge.pluginsReloadWithDisabled(
      dir: pluginDir,
      disabledIds: settings.disabledPluginIds.toList(),
    );
    await library.pluginsReloadWithDisabled(
      dir: pluginDir,
      disabledIds: settings.disabledPluginIds.toList(),
    );
  } catch (_) {}

  runApp(
    ProviderScope(
      overrides: [
        playerBridgeProvider.overrideWithValue(bridge),
        libraryBridgeProvider.overrideWithValue(library),
        coverDirProvider.overrideWithValue(coverDir),
        settingsStoreProvider.overrideWithValue(settings),
      ],
      child: const StellatuneApp(),
    ),
  );
}
