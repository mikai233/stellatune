import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/shell_page.dart';

class StellatuneApp extends StatelessWidget {
  const StellatuneApp({super.key});

  @override
  Widget build(BuildContext context) {
    const seed = Color(0xFF4F629A);
    final baseScheme = ColorScheme.fromSeed(seedColor: seed);
    final scheme = baseScheme.copyWith(
      surface: const Color(0xFFF5F6FB),
      surfaceContainerLowest: const Color(0xFFFFFFFF),
      surfaceContainerLow: const Color(0xFFF1F2F8),
      surfaceContainer: const Color(0xFFECEEF5),
      surfaceContainerHigh: const Color(0xFFE6E9F2),
      surfaceContainerHighest: const Color(0xFFDDE2EE),
      outlineVariant: const Color(0xFFC8CEDD),
      primary: const Color(0xFF4F629A),
      secondary: const Color(0xFF6D7FB0),
    );

    return MaterialApp(
      debugShowCheckedModeBanner: false,
      onGenerateTitle: (context) => AppLocalizations.of(context)!.appTitle,
      theme: ThemeData(
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
      ),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: const ShellPage(),
    );
  }
}
