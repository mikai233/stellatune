import 'dart:io';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/bridge/api/runtime.dart' as runtime_api;
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_paths.dart';
import 'package:stellatune/platform/rust_runtime.dart';
import 'package:stellatune/platform/tray_service.dart';
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

bool _isExitInProgress = false;

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

  // Tray and Close behavior
  await TrayService.instance.init();
  await windowManager.setPreventClose(true);
}

class WindowCloseHandler extends WindowListener {
  WindowCloseHandler(this.settings, this.bridge);
  final SettingsStore settings;
  final PlayerBridge bridge;

  @override
  void onWindowClose() async {
    if (settings.closeToTray) {
      await windowManager.hide();
    } else {
      await _exitApp(bridge);
    }
  }
}

Future<AppBootstrapResult> bootstrapApp() async {
  await initRustRuntime();

  final bridge = await PlayerBridge.create();
  await SettingsStore.initHive();
  final settings = SettingsStore();
  final paths = await _resolvePaths();

  final library = await LibraryBridge.create(
    dbPath: paths.dbPath,
    disabledPluginIds: settings.disabledPluginIds.toList(),
  );

  await _reloadPlugins(
    bridge: bridge,
    library: library,
    pluginDir: paths.pluginDir,
    disabledPluginIds: settings.disabledPluginIds.toList(),
  );
  await _applyPersistedOutputSettings(bridge: bridge, settings: settings);
  await _setupLyricsCacheDb(bridge: bridge, lyricsDbPath: paths.lyricsDbPath);

  if (Platform.isWindows || Platform.isLinux || Platform.isMacOS) {
    TrayService.instance.onExitRequested = () => _exitApp(bridge);
    windowManager.addListener(WindowCloseHandler(settings, bridge));
  }

  return AppBootstrapResult(
    bridge: bridge,
    library: library,
    settings: settings,
    coverDir: paths.coverDir,
  );
}

Future<void> _exitApp(PlayerBridge bridge) async {
  if (_isExitInProgress) return;
  _isExitInProgress = true;
  try {
    await bridge.dispose();
  } catch (e, s) {
    logger.w(
      'failed to dispose player bridge before exit',
      error: e,
      stackTrace: s,
    );
  }
  try {
    await runtime_api.shutdown();
  } catch (e, s) {
    logger.w(
      'failed to request runtime shutdown before exit',
      error: e,
      stackTrace: s,
    );
  } finally {
    exit(0);
  }
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
  // Best-effort: don't block startup on restore failures.
  try {
    final backend = settings.selectedBackend;
    var localDeviceId = settings.selectedDeviceId;
    try {
      await bridge.setOutputDevice(backend: backend, deviceId: localDeviceId);
    } catch (e, s) {
      logger.w(
        'failed to set persisted output device, falling back to default',
        error: e,
        stackTrace: s,
      );
      // Persisted local device may no longer exist; fallback to system default.
      localDeviceId = null;
      await settings.setSelectedDeviceId(null);
      await bridge.setOutputDevice(backend: backend, deviceId: null);
    }

    await bridge.setOutputOptions(
      matchTrackSampleRate: settings.matchTrackSampleRate,
      gaplessPlayback: settings.gaplessPlayback,
      seekTrackFade: settings.seekTrackFade,
    );

    final route = settings.outputSinkRoute;
    if (route == null) {
      await bridge.clearOutputSinkRoute();
    } else {
      final sinkTypes = await bridge.outputSinkListTypes();
      final sinkTypeExists = sinkTypes.any(
        (t) => t.pluginId == route.pluginId && t.typeId == route.typeId,
      );
      if (!sinkTypeExists) {
        await bridge.clearOutputSinkRoute();
        await settings.clearOutputSinkRoute();
      } else {
        var effectiveRoute = route;
        try {
          final rawTargets = await bridge.outputSinkListTargetsJson(
            pluginId: route.pluginId,
            typeId: route.typeId,
            configJson: route.configJson,
          );
          final targets = _parseOutputSinkTargets(rawTargets);
          if (targets.isNotEmpty) {
            final persistedTarget = route.targetJson.trim();
            final targetValues = targets.map(_targetValueOf).toSet();
            if (!targetValues.contains(persistedTarget)) {
              effectiveRoute = OutputSinkRoute(
                pluginId: route.pluginId,
                typeId: route.typeId,
                configJson: route.configJson,
                targetJson: _targetValueOf(targets.first),
              );
            }
          }
        } catch (e, s) {
          logger.w(
            'failed to probe output sink targets',
            error: e,
            stackTrace: s,
          );
          // Target probing failed. Keep route and let runtime apply decide fallback.
        }

        try {
          await bridge.setOutputSinkRoute(effectiveRoute);
          if (effectiveRoute != route) {
            await settings.setOutputSinkRoute(effectiveRoute);
          }
        } catch (e, s) {
          logger.e(
            'failed to set output sink route, falling back to local',
            error: e,
            stackTrace: s,
          );
          // Plugin route unusable (plugin disabled/unavailable/target invalid): fallback local output.
          await bridge.clearOutputSinkRoute();
          await settings.clearOutputSinkRoute();
        }
      }
    }

    try {
      await bridge.refreshDevices();
    } catch (e, s) {
      logger.w('failed to refresh output devices', error: e, stackTrace: s);
      // Non-fatal. Device refresh stream update is best-effort.
    }
  } catch (e, s) {
    logger.e(
      'failed to apply persisted output settings',
      error: e,
      stackTrace: s,
    );
  }
}

List<Object?> _parseOutputSinkTargets(String raw) {
  dynamic decoded;
  try {
    decoded = jsonDecode(raw);
  } catch (e, s) {
    logger.w(
      'failed to decode output sink targets JSON',
      error: e,
      stackTrace: s,
    );
    return const [];
  }
  if (decoded is List) {
    return decoded.cast<Object?>();
  }
  if (decoded is Map) {
    for (final key in ['targets', 'items', 'list', 'data', 'results']) {
      final v = decoded[key];
      if (v is List) {
        return v.cast<Object?>();
      }
    }
  }
  return const [];
}

String _targetValueOf(Object? target) =>
    target is String ? target : jsonEncode(target);

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
  } catch (e, s) {
    logger.e(
      'failed to reload plugins during bootstrap',
      error: e,
      stackTrace: s,
    );
  }
}
