import 'dart:io';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

class TrayService with TrayListener {
  TrayService._();
  static final TrayService instance = TrayService._();

  bool _initialized = false;

  Future<void> init() async {
    if (_initialized) return;
    if (!(Platform.isWindows || Platform.isLinux || Platform.isMacOS)) return;

    await _initTray();
    trayManager.addListener(this);
    _initialized = true;
  }

  Future<void> _initTray() async {
    // Set icon and tooltip
    String iconPath;
    if (Platform.isWindows) {
      // Use the .ico icon from windows folder as a fallback
      // In a real build, this should be moved to assets
      iconPath = 'windows/runner/resources/app_icon.ico';
    } else if (Platform.isMacOS) {
      iconPath =
          'macos/Runner/Assets.xcassets/AppIcon.appiconset/app_icon_32.png';
    } else {
      // For Linux, we just use a placeholder or the user should provide one
      iconPath = 'windows/runner/resources/app_icon.ico'; // Fallback
    }

    // Check if file exists, otherwise tray_manager might crash or show nothing
    if (await File(iconPath).exists()) {
      await trayManager.setIcon(iconPath);
    }

    await trayManager.setToolTip('Stellatune');

    // Default English menu until setLocaleStrings is called
    await setLocaleStrings(restoreLabel: 'Restore', exitLabel: 'Exit');
  }

  Future<void> setLocaleStrings({
    required String restoreLabel,
    required String exitLabel,
  }) async {
    final menu = [
      MenuItem(key: 'restore', label: restoreLabel),
      MenuItem.separator(),
      MenuItem(key: 'exit', label: exitLabel),
    ];
    await trayManager.setContextMenu(Menu(items: menu));
  }

  @override
  void onTrayIconMouseDown() {
    _restoreWindow();
  }

  @override
  void onTrayIconRightMouseDown() {
    trayManager.popUpContextMenu();
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    if (menuItem.key == 'restore') {
      _restoreWindow();
    } else if (menuItem.key == 'exit') {
      exit(0);
    }
  }

  Future<void> _restoreWindow() async {
    await windowManager.show();
    await windowManager.focus();
  }
}
