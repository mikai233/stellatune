import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/app_bootstrap.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/ui/app.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await initializeDesktopWindowIfNeeded();
  final bootstrap = await bootstrapApp();

  runApp(
    ProviderScope(
      overrides: [
        playerBridgeProvider.overrideWithValue(bootstrap.bridge),
        libraryBridgeProvider.overrideWithValue(bootstrap.library),
        coverDirProvider.overrideWithValue(bootstrap.coverDir),
        settingsStoreProvider.overrideWithValue(bootstrap.settings),
      ],
      child: const StellatuneApp(),
    ),
  );
}
