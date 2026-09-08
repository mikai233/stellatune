import 'dart:async';
import 'dart:ui';

import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/app_bootstrap.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/app/semantics_diagnostics.dart';
import 'package:stellatune/app/startup_app.dart';
import 'package:stellatune/ui/app.dart';

Future<void> main() async {
  final bootstrapFuture = runZonedGuarded<Future<void>>(
    () async {
      WidgetsFlutterBinding.ensureInitialized();
      assert(() {
        installSemanticsDiagnostics();
        return true;
      }());

      ErrorWidget.builder = (_) => const Center(child: Directionality(
        textDirection: TextDirection.ltr, child: Text('界面暂不可用 / View unavailable'),
      ));
      FlutterError.onError = (details) {
        FlutterError.presentError(details);
        DiagnosticsService.instance.report(
          details.exception,
          stack: details.stack,
          operation: 'flutter',
        );
        debugPrint('FlutterError: ${details.exceptionAsString()}');
        if (details.stack != null) {
          debugPrintStack(stackTrace: details.stack);
        }
      };

      PlatformDispatcher.instance.onError = (error, stack) {
        DiagnosticsService.instance.report(
          error,
          stack: stack,
          operation: 'platform',
        );
        debugPrint('PlatformDispatcher error: $error');
        debugPrintStack(stackTrace: stack);
        return true;
      };

      runApp(
        StartupApp(
          loadApp: () async {
            await initializeDesktopWindowIfNeeded();
            final bootstrap = await bootstrapApp();
            return ProviderScope(
              overrides: [
                playerBridgeProvider.overrideWithValue(bootstrap.bridge),
                libraryBridgeProvider.overrideWithValue(bootstrap.library),
                coverDirProvider.overrideWithValue(bootstrap.coverDir),
                settingsStoreServiceProvider.overrideWithValue(
                  bootstrap.settings,
                ),
              ],
              child: const StellatuneApp(),
            );
          },
          onExit: exitApplication,
        ),
      );
    },
    (error, stack) {
      DiagnosticsService.instance.report(
        error,
        stack: stack,
        operation: 'startup',
      );
      debugPrint('runZonedGuarded bootstrap error: $error');
      debugPrintStack(stackTrace: stack);
    },
  );

  if (bootstrapFuture != null) {
    await bootstrapFuture;
  }
}
