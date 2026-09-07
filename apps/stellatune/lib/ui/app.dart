import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/shell_page.dart';
import 'package:stellatune/ui/theme/desktop_theme.dart';

class StellatuneApp extends ConsumerWidget {
  const StellatuneApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final settings = ref.watch(settingsStoreProvider);
    final palette = settings.desktopTheme.palette;
    final scheme = palette.applyTo(ThemeData()).colorScheme;

    ThemeData buildTheme(ColorScheme scheme) {
      return ThemeData(
        colorScheme: scheme,
        scaffoldBackgroundColor: scheme.surface,
        canvasColor: scheme.surface,
        useMaterial3: true,
        visualDensity: VisualDensity.standard,
        fontFamily: 'NotoSansSC',
        dividerColor: scheme.onSurface.withValues(alpha: 0.10),
        inputDecorationTheme: InputDecorationTheme(
          filled: true,
          fillColor: scheme.surfaceContainerLowest.withValues(alpha: 0.78),
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(14),
            borderSide: BorderSide(
              color: scheme.onSurface.withValues(alpha: 0.10),
            ),
          ),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(14),
            borderSide: BorderSide(
              color: scheme.onSurface.withValues(alpha: 0.10),
            ),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(14),
            borderSide: BorderSide(color: scheme.primary),
          ),
        ),
        cardTheme: CardThemeData(
          color: scheme.surfaceContainerLowest.withValues(alpha: 0.82),
          elevation: 0,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(16),
            side: BorderSide(color: scheme.onSurface.withValues(alpha: 0.08)),
          ),
        ),
        navigationRailTheme: NavigationRailThemeData(
          backgroundColor: Colors.transparent,
          indicatorColor: scheme.secondaryContainer.withValues(alpha: 0.72),
          selectedIconTheme: IconThemeData(color: scheme.primary),
        ),
        iconButtonTheme: IconButtonThemeData(
          style: ButtonStyle(
            visualDensity: VisualDensity.compact,
            shape: WidgetStatePropertyAll(
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
            ),
          ),
        ),
      );
    }

    return MaterialApp(
      debugShowCheckedModeBanner: false,
      onGenerateTitle: (context) => AppLocalizations.of(context)!.appTitle,
      theme: palette.applyTo(buildTheme(scheme)),
      locale: settings.locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: const ShellPage(),
    );
  }
}
