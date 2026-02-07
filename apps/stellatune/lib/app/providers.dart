import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/app/settings_store.dart';

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

final settingsStoreProvider = ChangeNotifierProvider<SettingsStore>((ref) {
  throw UnimplementedError(
    'settingsStoreProvider must be overridden in main()',
  );
});
