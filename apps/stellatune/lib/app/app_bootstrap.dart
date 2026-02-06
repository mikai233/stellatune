import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_paths.dart';
import 'package:stellatune/platform/rust_runtime.dart';
import 'package:window_manager/window_manager.dart';

class AppBootstrapResult {
  const AppBootstrapResult({
    required this.bridge,
    required this.library,
    required this.settings,
    required this.coverDir,
  });

  final PlayerBridge bridge;
  final LibraryBridge library;
  final SettingsStore settings;
  final String coverDir;
}

class _BootstrapPaths {
  const _BootstrapPaths({
    required this.dbPath,
    required this.pluginDir,
    required this.coverDir,
    required this.lyricsDbPath,
  });

  final String dbPath;
  final String pluginDir;
  final String coverDir;
  final String lyricsDbPath;
}

Future<void> initializeDesktopWindowIfNeeded() async {
  if (!(Platform.isWindows || Platform.isLinux || Platform.isMacOS)) {
    return;
  }
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

Future<AppBootstrapResult> bootstrapApp() async {
  await initRustRuntime();

  final bridge = await PlayerBridge.create();
  final settings = await SettingsStore.open();
  final paths = await _resolvePaths();

  await _applyPersistedOutputSettings(bridge: bridge, settings: settings);

  final library = await LibraryBridge.create(
    dbPath: paths.dbPath,
    disabledPluginIds: settings.disabledPluginIds.toList(),
  );

  await _setupLyricsCacheDb(bridge: bridge, lyricsDbPath: paths.lyricsDbPath);
  await _reloadPlugins(
    bridge: bridge,
    library: library,
    pluginDir: paths.pluginDir,
    disabledPluginIds: settings.disabledPluginIds.toList(),
  );

  return AppBootstrapResult(
    bridge: bridge,
    library: library,
    settings: settings,
    coverDir: paths.coverDir,
  );
}

Future<_BootstrapPaths> _resolvePaths() async {
  final dbPath = await defaultLibraryDbPath();
  final pluginDir = await defaultPluginDir();
  await Directory(pluginDir).create(recursive: true);

  final baseDir = p.dirname(dbPath);
  return _BootstrapPaths(
    dbPath: dbPath,
    pluginDir: pluginDir,
    coverDir: p.join(baseDir, 'covers'),
    lyricsDbPath: p.join(baseDir, 'lyrics_cache.sqlite'),
  );
}

Future<void> _applyPersistedOutputSettings({
  required PlayerBridge bridge,
  required SettingsStore settings,
}) async {
  // Best-effort: don't block startup on device/backend restore failures.
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
}

Future<void> _setupLyricsCacheDb({
  required PlayerBridge bridge,
  required String lyricsDbPath,
}) async {
  // Best-effort: lyrics can still work without persistent cache.
  try {
    await bridge.lyricsSetCacheDbPath(lyricsDbPath);
  } catch (_) {}
}

Future<void> _reloadPlugins({
  required PlayerBridge bridge,
  required LibraryBridge library,
  required String pluginDir,
  required List<String> disabledPluginIds,
}) async {
  // Best-effort: app should still start even if plugin scan/reload fails.
  try {
    await bridge.pluginsReloadWithDisabled(
      dir: pluginDir,
      disabledIds: disabledPluginIds,
    );
    await library.pluginsReloadWithDisabled(
      dir: pluginDir,
      disabledIds: disabledPluginIds,
    );
  } catch (_) {}
}
