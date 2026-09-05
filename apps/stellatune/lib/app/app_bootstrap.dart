import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/bridge/api/player.dart' as player_api;
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/ui/pages/settings/settings_value_utils.dart';
import 'package:stellatune/bridge/api/runtime.dart' as runtime_api;
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_paths.dart';
import 'package:stellatune/platform/directory_access_service.dart';
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
    required this.coverDir,
    required this.lyricsDbPath,
    required this.pluginDir,
  });

  final String dbPath;
  final String coverDir;
  final String lyricsDbPath;
  final String pluginDir;
}

class _ResolvedPluginRoute {
  const _ResolvedPluginRoute({required this.route, required this.targets});

  final OutputSinkRoute route;
  final List<Object?> targets;
}

class _PersistedOutputSettingsRestorer {
  _PersistedOutputSettingsRestorer({
    required this.bridge,
    required this.settings,
  }) : persisted = settings.readState(),
       session = settings.outputSettingsUiSession;

  final PlayerBridge bridge;
  final SettingsStore settings;
  final SettingsState persisted;
  final OutputSettingsUiSession session;

  String? localDeviceId;

  Future<void> restore() async {
    localDeviceId = persisted.selectedDeviceId;
    _primeLocalSession();
    await bridge.setPlaybackLatency(persisted.playbackLatency);
    await _restoreLocalOutputDevice();
    await _restoreOutputOptions();
    await _restoreOutputRoute();
    await _refreshDevicesBestEffort();
  }

  Future<void> _restoreLocalOutputDevice() async {
    final backend = persisted.selectedBackend;
    try {
      await bridge.setOutputDevice(backend: backend, deviceId: localDeviceId);
    } catch (e, s) {
      logger.w(
        'failed to set persisted output device, falling back to default',
        error: e,
        stackTrace: s,
      );
      localDeviceId = null;
      await settings.setSelectedDeviceId(null);
      _primeLocalSession();
      await bridge.setOutputDevice(backend: backend, deviceId: null);
    }
  }

  Future<void> _restoreOutputOptions() {
    return bridge.setOutputOptions(
      matchTrackSampleRate: persisted.matchTrackSampleRate,
      gaplessPlayback: persisted.gaplessPlayback,
      seekTrackFade: persisted.seekTrackFade,
      resampleQuality: persisted.resampleQuality,
    );
  }

  Future<void> _restoreOutputRoute() async {
    final route = settings.readState().outputSinkRoute;
    if (route == null) {
      await bridge.clearOutputSinkRoute();
      _primeLocalSession();
      return;
    }

    if (!await _sinkTypeExists(route)) {
      await _fallbackToLocal(clearPersistedRoute: true);
      return;
    }

    final resolved = await _resolvePluginRoute(route);
    try {
      await bridge.setOutputSinkRoute(resolved.route);
      if (resolved.route != route) {
        await settings.setOutputSinkRoute(resolved.route);
      }
      _primePluginSession(resolved);
    } catch (e, s) {
      logger.e(
        'failed to set output sink route, falling back to local',
        error: e,
        stackTrace: s,
      );
      await _fallbackToLocal(clearPersistedRoute: true);
    }
  }

  Future<bool> _sinkTypeExists(OutputSinkRoute route) async {
    final sinkTypes = await bridge.outputSinkListTypes();
    return sinkTypes.any(
      (t) => t.pluginId == route.pluginId && t.typeId == route.typeId,
    );
  }

  Future<_ResolvedPluginRoute> _resolvePluginRoute(
    OutputSinkRoute route,
  ) async {
    try {
      final rawTargets = await bridge.outputSinkListTargetsJson(
        pluginId: route.pluginId,
        typeId: route.typeId,
        configJson: route.configJson,
      );
      final targets = SettingsValueUtils.parseOutputSinkTargetsJson(rawTargets);
      if (targets.isEmpty) {
        return _ResolvedPluginRoute(route: route, targets: targets);
      }

      final persistedTarget = route.targetJson.trim();
      final targetValues = targets
          .map(SettingsValueUtils.targetValueOf)
          .toSet();
      if (targetValues.contains(persistedTarget)) {
        return _ResolvedPluginRoute(route: route, targets: targets);
      }

      return _ResolvedPluginRoute(
        route: OutputSinkRoute(
          pluginId: route.pluginId,
          typeId: route.typeId,
          configJson: route.configJson,
          targetJson: SettingsValueUtils.targetValueOf(targets.first),
        ),
        targets: targets,
      );
    } catch (e, s) {
      logger.w('failed to probe output sink targets', error: e, stackTrace: s);
      return _ResolvedPluginRoute(route: route, targets: const []);
    }
  }

  Future<void> _fallbackToLocal({required bool clearPersistedRoute}) async {
    await bridge.clearOutputSinkRoute();
    if (clearPersistedRoute) {
      await settings.clearOutputSinkRoute();
    }
    _primeLocalSession();
  }

  Future<void> _refreshDevicesBestEffort() async {
    try {
      await bridge.refreshDevices();
    } catch (e, s) {
      logger.w('failed to refresh output devices', error: e, stackTrace: s);
      // Non-fatal. Device probing is best-effort during bootstrap.
    }
  }

  void _primeLocalSession() {
    session.initialized = true;
    session.selectedOutputBackendKey = SettingsValueUtils.localBackendKey(
      persisted.selectedBackend,
    );
    session.selectedOutputSinkTypeKey = null;
    session.outputSinkConfigJson = '{}';
    session.outputSinkTargetJson = '{}';
    session.outputSinkTargets = const [];
    session.loadingOutputSinkTargets = false;
    session.resampleQuality = persisted.resampleQuality;
  }

  void _primePluginSession(_ResolvedPluginRoute resolved) {
    final route = resolved.route;
    final typeKey = '${route.pluginId}::${route.typeId}';
    session.initialized = true;
    session.selectedOutputBackendKey = SettingsValueUtils.pluginBackendKey(
      route.pluginId,
      route.typeId,
    );
    session.selectedOutputSinkTypeKey = typeKey;
    session.outputSinkConfigJson = route.configJson;
    session.outputSinkTargetJson = route.targetJson;
    session.outputSinkTargets = List<Object?>.from(resolved.targets);
    session.loadingOutputSinkTargets = false;
    session.outputSinkConfigDrafts[typeKey] = route.configJson;
    session.resampleQuality = persisted.resampleQuality;
  }
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
    if (settings.readState().closeToTray) {
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
  bridge.bindDirectoryAccessStore(settings);
  final paths = await _resolvePaths();

  final library = await LibraryBridge.create(dbPath: paths.dbPath);
  await DirectoryAccessService.instance.syncStoredDirectories(
    paths: await library.listRoots(),
    store: settings,
  );
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
    await player_api.hostApiStop();
  } catch (e, s) {
    logger.w('failed to stop host API before exit', error: e, stackTrace: s);
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
    final restorer = _PersistedOutputSettingsRestorer(
      bridge: bridge,
      settings: settings,
    );
    await restorer.restore();
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
