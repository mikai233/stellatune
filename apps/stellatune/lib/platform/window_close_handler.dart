import 'dart:async';

import 'package:stellatune/app/logging.dart';
import 'package:window_manager/window_manager.dart';

/// A window may only hide when a working tray can bring it back.
class WindowCloseHandler extends WindowListener {
  WindowCloseHandler({
    required this.closeToTray,
    required this.trayAvailable,
    required this.hideWindow,
    required this.exitApp,
  });

  bool Function() closeToTray;
  final Future<bool> Function() trayAvailable;
  final Future<void> Function() hideWindow;
  final Future<void> Function() exitApp;
  bool _handlingClose = false;

  @override
  void onWindowClose() => unawaited(handleClose());

  Future<void> handleClose() async {
    if (_handlingClose) return;
    _handlingClose = true;
    try {
      try {
        if (closeToTray() && await trayAvailable()) {
          await hideWindow();
          return;
        }
      } catch (error, stack) {
        logger.w(
          'cannot close to tray; exiting',
          error: error,
          stackTrace: stack,
        );
      }
      await exitApp();
    } finally {
      _handlingClose = false;
    }
  }
}
