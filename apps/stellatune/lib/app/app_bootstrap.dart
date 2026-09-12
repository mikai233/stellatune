import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/app_bootstrap_services.dart';
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/bridge/api/player.dart' as player_api;
import 'package:stellatune/bridge/api/library.dart' as library_api;
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/app/output_settings_runtime.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_paths.dart';
import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/platform/tray_service.dart';
import 'package:stellatune/platform/window_close_handler.dart';
import 'package:window_manager/window_manager.dart';
import 'package:stellatune/app/logging.dart';

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

class LibraryRebuildRequired implements Exception {
  const LibraryRebuildRequired(this.dbPath);
  final String dbPath;
}

class _BootstrapPaths {
  const _BootstrapPaths({
    required this.dbPath,
    required this.coverDir,
    required this.lyricsDbPath,
    required this.pluginDir,
  });

  final String dbPath;
  final String coverDir;
  final String lyricsDbPath;
  final String pluginDir;
}

bool _isExitInProgress = false;
_AppResources? _activeResources;
WindowCloseHandler? _windowCloseHandler;

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
  await windowManager.waitUntilReadyToShow(windowOptions);
  await windowManager.show();
  await windowManager.focus();

  // Closing while startup is incomplete must remain possible, even if Rust
  // cannot load. The completed bootstrap installs the user's close preference.
  final handler = WindowCloseHandler(
    closeToTray: () => false,
    trayAvailable: TrayService.instance.checkAvailability,
    hideWindow: windowManager.hide,
    exitApp: exitApplication,
  );
  _windowCloseHandler = handler;
  windowManager.addListener(handler);
  TrayService.instance.onExitRequested = exitApplication;
  try {
    await TrayService.instance.init();
    await windowManager.setPreventClose(true);
  } catch (_) {
    // Keep the close listener for the failure screen, but release the icon.
    await TrayService.instance.dispose();
    rethrow;
  }
}

Future<AppBootstrapResult> bootstrapApp({
  AppBootstrapServices services = const AppBootstrapServices(),
}) async {
  final resources = _AppResources(services);
  _activeResources = resources;
  try {
    await services.initializeRuntime();
    resources.runtimeReady = true;
    final bridge = await services.createPlayer();
    resources.bridge = bridge;
    resources.settingsStarted = true;
    final settings = await services.openSettings();
    bridge.bindDirectoryAccessStore(settings);
    final paths = await _resolvePaths();

    if (await library_api.libraryRebuildRequired(dbPath: paths.dbPath)) {
      throw LibraryRebuildRequired(paths.dbPath);
    }

    final library = await LibraryBridge.create(dbPath: paths.dbPath);
    await DirectoryAccessService.instance.syncStoredDirectories(
      paths: await library.listRoots(),
      store: settings,
    );
    resources.hostApiStarted = true;
    await player_api.hostApiStart(
      dataRoot: p.join(p.dirname(paths.pluginDir), 'plugin-data'),
    );
    try {
      await library.pluginApplyState();
    } catch (e, s) {
      logger.w(
        'failed to apply plugin runtime state during bootstrap',
        error: e,
        stackTrace: s,
      );
    }

    await _applyPersistedOutputSettings(bridge: bridge, settings: settings);
    await _setupLyricsCacheDb(bridge: bridge, lyricsDbPath: paths.lyricsDbPath);
    try {
      await player_api.playbackRestoreState();
    } catch (e, s) {
      logger.w(
        'failed to restore playback after plugin initialization',
        error: e,
        stackTrace: s,
      );
    }

    _windowCloseHandler?.closeToTray = () => settings.readState().closeToTray;

    return AppBootstrapResult(
      bridge: bridge,
      library: library,
      settings: settings,
      coverDir: paths.coverDir,
    );
  } catch (_) {
    await resources.dispose();
    rethrow;
  }
}

Future<void> exitApplication() async {
  if (_isExitInProgress) return;
  _isExitInProgress = true;
  try {
    await _activeResources?.dispose();
    await TrayService.instance.dispose();
  } finally {
    exit(0);
  }
}

class _AppResources {
  _AppResources(this.services);
  final AppBootstrapServices services;
  PlayerBridge? bridge;
  bool runtimeReady = false;
  bool settingsStarted = false;
  bool hostApiStarted = false;
  Future<void>? _disposal;

  Future<void> dispose() => _disposal ??= _dispose();

  Future<void> _dispose() async {
    // Attempt every cleanup, retaining the original startup error if one fails.
    final player = bridge;
    if (player != null) await _cleanup('player bridge', player.dispose);
    if (hostApiStarted) await _cleanup('host API', services.stopHostApi);
    if (runtimeReady) await _cleanup('runtime', services.shutdownRuntime);
    if (settingsStarted) await _cleanup('settings', services.closeSettings);
    await TrayService.instance.dispose();
  }

  Future<void> _cleanup(
    String resource,
    Future<void> Function() release,
  ) async {
    try {
      await release();
    } catch (error, stack) {
      logger.w('failed to close $resource', error: error, stackTrace: stack);
    }
  }
}

Future<_BootstrapPaths> _resolvePaths() async {
  final dbPath = await defaultLibraryDbPath();
  final pluginDir = await defaultPluginDir();
  await Directory(pluginDir).create(recursive: true);

  final baseDir = p.dirname(dbPath);
  return _BootstrapPaths(
    dbPath: dbPath,
    coverDir: p.join(baseDir, 'covers'),
    lyricsDbPath: p.join(baseDir, 'lyrics_cache.sqlite'),
    pluginDir: pluginDir,
  );
}

Future<void> _applyPersistedOutputSettings({
  required PlayerBridge bridge,
  required SettingsStore settings,
}) async {
  // Best-effort: don't block startup on restore failures.
  try {
    await restorePersistedOutputSettings(bridge: bridge, settings: settings);
  } catch (e, s) {
    logger.e(
      'failed to apply persisted output settings',
      error: e,
      stackTrace: s,
    );
  }
}

Future<void> _setupLyricsCacheDb({
  required PlayerBridge bridge,
  required String lyricsDbPath,
}) async {
  // Best-effort: lyrics can still work without persistent cache.
  try {
    await bridge.lyricsSetCacheDbPath(lyricsDbPath);
  } catch (e, s) {
    logger.e('failed to setup lyrics cache db', error: e, stackTrace: s);
  }
}
