import 'dart:io';

import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_section_card.dart';

class SettingsAppearanceSection extends StatelessWidget {
  const SettingsAppearanceSection({
    super.key,
    required this.l10n,
    required this.locale,
    required this.themeMode,
    required this.closeToTray,
    required this.onLocaleChanged,
    required this.onThemeModeChanged,
    required this.onCloseToTrayChanged,
  });

  final AppLocalizations l10n;
  final Locale? locale;
  final ThemeMode themeMode;
  final bool closeToTray;
  final Future<void> Function(Locale? locale) onLocaleChanged;
  final Future<void> Function(ThemeMode mode) onThemeModeChanged;
  final Future<void> Function(bool enabled) onCloseToTrayChanged;

  @override
  Widget build(BuildContext context) {
    return SettingsSectionCard(
      title: l10n.settingsAppearanceTitle,
      children: [
        DropdownButtonFormField<Locale?>(
          decoration: InputDecoration(
            labelText: l10n.settingsLanguage,
            border: const OutlineInputBorder(),
            isDense: true,
          ),
          initialValue: locale,
          items: [
            DropdownMenuItem(
              value: null,
              child: Text(l10n.settingsLocaleSystem),
            ),
            DropdownMenuItem(
              value: const Locale('zh'),
              child: Text(l10n.settingsLocaleZh),
            ),
            DropdownMenuItem(
              value: const Locale('en'),
              child: Text(l10n.settingsLocaleEn),
            ),
          ],
          onChanged: (value) async {
            await onLocaleChanged(value);
          },
        ),
        const SizedBox(height: 12),
        DropdownButtonFormField<ThemeMode>(
          decoration: InputDecoration(
            labelText: l10n.settingsThemeMode,
            border: const OutlineInputBorder(),
            isDense: true,
          ),
          initialValue: themeMode,
          items: [
            DropdownMenuItem(
              value: ThemeMode.system,
              child: Text(l10n.settingsThemeSystem),
            ),
            DropdownMenuItem(
              value: ThemeMode.light,
              child: Text(l10n.settingsThemeLight),
            ),
            DropdownMenuItem(
              value: ThemeMode.dark,
              child: Text(l10n.settingsThemeDark),
            ),
          ],
          onChanged: (value) async {
            if (value == null) return;
            await onThemeModeChanged(value);
          },
        ),
        if (Platform.isWindows || Platform.isLinux || Platform.isMacOS)
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l10n.settingsCloseToTray),
            subtitle: Text(l10n.settingsCloseToTraySubtitle),
            value: closeToTray,
            onChanged: (enabled) async {
              await onCloseToTrayChanged(enabled);
            },
          ),
      ],
    );
  }
}
