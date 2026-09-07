import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/settings/models/installed_plugin.dart';
import 'package:stellatune/ui/pages/settings/widgets/plugin_tile.dart';
import 'package:stellatune/ui/pages/shell/desktop_frame.dart';

void main() {
  for (final fails in [false, true]) {
    testWidgets('desktop uninstall feedback (fails: $fails)', (tester) async {
      tester.view.physicalSize = const Size(1100, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      var called = false;
      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: DesktopFrame(
            selectedIndex: 3,
            onDestinationSelected: (_) {},
            playerBar: const SizedBox(height: 78),
            child: Builder(
              builder: (context) => Padding(
                padding: const EdgeInsets.only(top: 80),
                child: SettingsPluginTile(
                  plugin: const InstalledPlugin(
                    dirPath: 'test-plugin',
                    id: 'test-plugin',
                    name: 'Test plugin',
                    hasWebUi: false,
                    infoJson: null,
                    installState: 'installed',
                    uninstallRetryCount: 0,
                    uninstallLastError: null,
                  ),
                  isDisabled: true,
                  isLoaded: false,
                  loadedKnown: true,
                  pluginSourceTypes: const [],
                  pluginOutputSinkTypes: const [],
                  onOpenWebUi: null,
                  onToggleEnabled: (_) async {},
                  outputSinkConfigForType: (_) => null,
                  onOutputSinkConfigChanged: (_, _) {},
                  onUninstall: () async {
                    called = true;
                    await Future<void>.value();
                    if (fails) throw StateError('uninstall failed');
                    if (!context.mounted) return;
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Plugin uninstalled')),
                    );
                  },
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Uninstall'));
      await tester.pumpAndSettle();
      expect(called, isTrue);
      expect(tester.takeException(), isNull);
      expect(find.byType(SnackBar), findsOneWidget);
      // A subsequent dialog remains usable after reporting the result.
      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }
}
