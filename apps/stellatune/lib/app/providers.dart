import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';

export 'package:stellatune/app/settings_store.dart'
    show
        OutputSettingsUiSession,
        SettingsController,
        SettingsState,
        SettingsStore,
        settingsStoreProvider,
        settingsStoreServiceProvider,
        settingsUiSessionProvider;

final playerBridgeProvider = Provider<PlayerBridge>((ref) {
  throw UnimplementedError('playerBridgeProvider must be overridden in main()');
});

final libraryBridgeProvider = Provider<LibraryBridge>((ref) {
  throw UnimplementedError(
    'libraryBridgeProvider must be overridden in main()',
  );
});

final coverDirProvider = Provider<String>((ref) {
  throw UnimplementedError('coverDirProvider must be overridden in main()');
});

final audioDevicesProvider = FutureProvider<List<AudioDevice>>((ref) async {
  final bridge = ref.watch(playerBridgeProvider);
  return bridge.refreshDevices();
});
