import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';

final playerBridgeProvider = Provider<PlayerBridge>((ref) {
  throw UnimplementedError('playerBridgeProvider must be overridden in main()');
});

final libraryBridgeProvider = Provider<LibraryBridge>((ref) {
  throw UnimplementedError(
    'libraryBridgeProvider must be overridden in main()',
  );
});
