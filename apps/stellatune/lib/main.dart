import 'dart:async';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/app_bootstrap.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/ui/app.dart';

Future<void> main() async {
  final bootstrapFuture = runZonedGuarded<Future<void>>(
    () async {
      WidgetsFlutterBinding.ensureInitialized();

      FlutterError.onError = (details) {
        FlutterError.presentError(details);
        debugPrint('FlutterError: ${details.exceptionAsString()}');
        if (details.stack != null) {
          debugPrintStack(stackTrace: details.stack);
        }
      };

      PlatformDispatcher.instance.onError = (error, stack) {
        debugPrint('PlatformDispatcher error: $error');
        debugPrintStack(stackTrace: stack);
        return true;
      };

      await initializeDesktopWindowIfNeeded();
      final bootstrap = await bootstrapApp();

      runApp(
        ProviderScope(
          overrides: [
            playerBridgeProvider.overrideWithValue(bootstrap.bridge),
            libraryBridgeProvider.overrideWithValue(bootstrap.library),
            coverDirProvider.overrideWithValue(bootstrap.coverDir),
            settingsStoreProvider.overrideWith(() => bootstrap.settings),
          ],
          child: const StellatuneApp(),
        ),
      );
    },
    (error, stack) {
      debugPrint('runZonedGuarded bootstrap error: $error');
      debugPrintStack(stackTrace: stack);
    },
  );

  if (bootstrapFuture != null) {
    await bootstrapFuture;
  }
}
