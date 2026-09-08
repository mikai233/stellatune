import 'package:hive_flutter/hive_flutter.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/bridge/api/player.dart' as player_api;
import 'package:stellatune/bridge/api/runtime.dart' as runtime_api;
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/platform/rust_runtime.dart';

/// Platform resources used during startup. Overridable without loading Rust.
class AppBootstrapServices {
  const AppBootstrapServices();

  Future<void> initializeRuntime() => initRustRuntime();
  Future<PlayerBridge> createPlayer() => PlayerBridge.create();
  Future<SettingsStore> openSettings() async {
    await SettingsStore.initHive();
    return SettingsStore();
  }

  Future<void> stopHostApi() => player_api.hostApiStop();
  Future<void> shutdownRuntime() => runtime_api.shutdown();
  Future<void> closeSettings() => Hive.close();
}
