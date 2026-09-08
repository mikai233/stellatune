import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/platform/tray_service.dart';
import 'package:stellatune/platform/window_close_handler.dart';
import 'package:tray_manager/tray_manager.dart';

class _MissingAssets extends CachingAssetBundle {
  @override
  Future<ByteData> load(String key) async => throw StateError('missing $key');
}

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  const trayChannel = MethodChannel('tray_manager');
  const statusChannel = MethodChannel('stellatune/tray_status');
  const windowChannel = MethodChannel('window_manager');
  late List<MethodCall> calls;
  var shellHasIcon = true;
  String? failedMethod;

  setUp(() {
    calls = [];
    shellHasIcon = true;
    failedMethod = null;
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    binding.defaultBinaryMessenger.setMockMethodCallHandler(trayChannel, (
      call,
    ) async {
      calls.add(call);
      if (call.method == failedMethod) {
        throw PlatformException(code: 'unavailable');
      }
      if (call.method == 'getBounds') {
        return {'x': 0.0, 'y': 0.0, 'width': 24.0, 'height': 24.0};
      }
      return true;
    });
    binding.defaultBinaryMessenger.setMockMethodCallHandler(
      statusChannel,
      (call) async => shellHasIcon,
    );
    binding.defaultBinaryMessenger.setMockMethodCallHandler(windowChannel, (
      call,
    ) async {
      calls.add(call);
      return false;
    });
  });

  tearDown(() {
    debugDefaultTargetPlatformOverride = null;
    for (final channel in [trayChannel, statusChannel, windowChannel]) {
      binding.defaultBinaryMessenger.setMockMethodCallHandler(channel, null);
    }
  });

  test(
    'installed asset key creates a usable tray and supports restore/exit',
    () async {
      final tray = TrayService();
      addTearDown(tray.dispose);
      expect(await tray.init(), isTrue);
      expect(tray.isAvailable, isTrue);
      final iconPath =
          (calls.singleWhere((c) => c.method == 'setIcon').arguments
                  as Map)['iconPath']
              as String;
      expect(File(iconPath).isAbsolute, isTrue);
      expect(
        iconPath.replaceAll('\\', '/'),
        endsWith('data/flutter_assets/assets/tray/app_icon.ico'),
      );
      var exits = 0;
      tray.onExitRequested = () async {
        exits++;
      };
      tray.onTrayIconMouseDown();
      await Future<void>.delayed(Duration.zero);
      expect(calls.map((c) => c.method), containsAllInOrder(['show', 'focus']));
      tray.onTrayMenuItemClick(MenuItem(key: 'exit', label: 'Exit'));
      await Future<void>.delayed(Duration.zero);
      expect(exits, 1);
      shellHasIcon = false;
      expect(await tray.checkAvailability(), isFalse);
      expect(tray.isAvailable, isFalse);
    },
  );

  test('missing bundle asset never advertises a tray', () async {
    final tray = TrayService(assets: _MissingAssets());
    expect(await tray.init(), isFalse);
    expect(calls, isEmpty);
    expect(tray.isAvailable, isFalse);
  });

  test(
    'menu creation failure cleans the partial icon and allows another init',
    () async {
      final tray = TrayService();
      addTearDown(tray.dispose);
      failedMethod = 'setContextMenu';
      expect(await tray.init(), isFalse);
      expect(calls.last.method, 'destroy');
      failedMethod = null;
      expect(await tray.init(), isTrue);
      await tray.dispose();
      expect(tray.isAvailable, isFalse);
    },
  );

  test(
    'PNG is bundled for macOS/Linux; unsupported host cannot hide a window',
    () async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      final tray = TrayService();
      addTearDown(tray.dispose);
      expect(await tray.init(), isFalse);
      final iconPath =
          (calls.singleWhere((c) => c.method == 'setIcon').arguments
              as Map)['iconPath'];
      expect(iconPath, endsWith('app_icon.png'));
      expect(calls.any((c) => c.method == 'setContextMenu'), isTrue);
    },
  );

  test('macOS verifies status item bounds', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    final tray = TrayService();
    addTearDown(tray.dispose);
    expect(await tray.init(), isTrue);
    expect(calls.any((c) => c.method == 'getBounds'), isTrue);
  });

  test('close only hides with a confirmed tray, otherwise exits', () async {
    var available = true;
    var wantsTray = true;
    var hidden = 0;
    var exited = 0;
    final handler = WindowCloseHandler(
      closeToTray: () => wantsTray,
      trayAvailable: () async => available,
      hideWindow: () async {
        hidden++;
      },
      exitApp: () async {
        exited++;
      },
    );
    await handler.handleClose();
    expect(hidden, 1);
    expect(exited, 0);
    available = false;
    await handler.handleClose();
    expect(hidden, 1);
    expect(exited, 1);
    available = true;
    wantsTray = false;
    await handler.handleClose();
    expect(exited, 2);
  });

  test('hide failure exits; overlapping close events have one owner', () async {
    final gate = Completer<bool>();
    var exited = 0;
    final handler = WindowCloseHandler(
      closeToTray: () => true,
      trayAvailable: () => gate.future,
      hideWindow: () async => throw StateError('window unavailable'),
      exitApp: () async {
        exited++;
      },
    );
    final closing = handler.handleClose();
    await handler.handleClose();
    gate.complete(true);
    await closing;
    expect(exited, 1);
  });
}
