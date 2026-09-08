import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:stellatune/app/logging.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

class TrayService with TrayListener {
  TrayService({AssetBundle? assets, TargetPlatform? platform})
    : _assets = assets ?? rootBundle,
      _platform = platform ?? defaultTargetPlatform;

  static final TrayService instance = TrayService();
  static const _statusChannel = MethodChannel('stellatune/tray_status');

  final AssetBundle _assets;
  final TargetPlatform _platform;
  bool _initialized = false;
  bool _iconCreated = false;
  bool _available = false;
  Future<void> Function()? onExitRequested;

  bool get isAvailable => _available;

  Future<bool> init() async {
    if (_initialized) return checkAvailability();
    if (!const {
      TargetPlatform.windows,
      TargetPlatform.macOS,
      TargetPlatform.linux,
    }.contains(_platform)) {
      return false;
    }

    final icon = _platform == TargetPlatform.windows
        ? 'assets/tray/app_icon.ico'
        : 'assets/tray/app_icon.png';
    try {
      // tray_manager resolves asset keys against the installed Flutter bundle.
      // Check the bundle, never a source-tree path or the working directory.
      final bytes = await _assets.load(icon);
      if (bytes.lengthInBytes == 0) throw StateError('Empty tray icon: $icon');
      _iconCreated = true;
      await trayManager.setIcon(icon);
      if (_platform != TargetPlatform.linux) {
        await trayManager.setToolTip('Stellatune');
      }
      await _setMenu(restoreLabel: 'Restore', exitLabel: 'Exit');
      trayManager.addListener(this);
      _initialized = true;
      return await checkAvailability();
    } catch (error, stack) {
      logger.w(
        'tray unavailable; closing the window will exit',
        error: error,
        stackTrace: stack,
      );
      await dispose();
      return false;
    }
  }

  /// Recheck before hiding: creating an icon does not prove the shell has it.
  Future<bool> checkAvailability() async {
    if (!_initialized) return false;
    try {
      _available = switch (_platform) {
        TargetPlatform.windows =>
          await _statusChannel.invokeMethod<bool>('isAvailable') ?? false,
        TargetPlatform.macOS =>
          (await trayManager.getBounds())?.isEmpty == false,
        // tray_manager cannot confirm a Linux tray host. Keep the menu if the
        // desktop supports it, but don't hide the only known recovery window.
        _ => false,
      };
    } catch (error, stack) {
      _available = false;
      logger.w(
        'failed to confirm tray availability',
        error: error,
        stackTrace: stack,
      );
    }
    return _available;
  }

  Future<void> setLocaleStrings({
    required String restoreLabel,
    required String exitLabel,
  }) async {
    if (!_initialized) return;
    try {
      await _setMenu(restoreLabel: restoreLabel, exitLabel: exitLabel);
    } catch (error, stack) {
      logger.w('failed to update tray menu', error: error, stackTrace: stack);
      await dispose();
    }
  }

  Future<void> _setMenu({
    required String restoreLabel,
    required String exitLabel,
  }) => trayManager.setContextMenu(
    Menu(
      items: [
        MenuItem(key: 'restore', label: restoreLabel),
        MenuItem.separator(),
        MenuItem(key: 'exit', label: exitLabel),
      ],
    ),
  );

  Future<void> dispose() async {
    _available = false;
    if (_initialized) trayManager.removeListener(this);
    _initialized = false;
    if (!_iconCreated) return;
    _iconCreated = false;
    try {
      await trayManager.destroy();
    } catch (error, stack) {
      logger.w('failed to destroy tray icon', error: error, stackTrace: stack);
    }
  }

  @override
  void onTrayIconMouseDown() => _run(_restoreWindow);

  @override
  void onTrayIconRightMouseDown() => _run(trayManager.popUpContextMenu);

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    if (menuItem.key == 'restore') {
      _run(_restoreWindow);
    } else if (menuItem.key == 'exit') {
      final callback = onExitRequested;
      if (callback != null) _run(callback);
    }
  }

  void _run(Future<void> Function() action) {
    unawaited(
      action().catchError((Object error, StackTrace stack) {
        logger.w('tray action failed', error: error, stackTrace: stack);
      }),
    );
  }

  Future<void> _restoreWindow() async {
    await windowManager.show();
    await windowManager.focus();
  }
}
