import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/library/library_paths.dart';
import 'package:stellatune/platform/rust_runtime.dart';
import 'package:stellatune/ui/app.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  await initRustRuntime();
  final bridge = await PlayerBridge.create();
  final dbPath = await defaultLibraryDbPath();
  final library = await LibraryBridge.create(dbPath: dbPath);

  runApp(
    ProviderScope(
      overrides: [
        playerBridgeProvider.overrideWithValue(bridge),
        libraryBridgeProvider.overrideWithValue(library),
      ],
      child: const StellatuneApp(),
    ),
  );
}
