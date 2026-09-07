import 'package:stellatune/ui/theme/desktop_theme.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_form_row.dart';

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_section_card.dart';

class SettingsAppearanceSection extends StatelessWidget {
  const SettingsAppearanceSection({
    super.key,
    required this.l10n,
    required this.locale,
    required this.desktopTheme,
    this.onDesktopThemeChanged,
    required this.closeToTray,
    required this.onLocaleChanged,
    required this.onCloseToTrayChanged,
  });

  final AppLocalizations l10n;
  final Locale? locale;
  final DesktopThemePreset desktopTheme;
  final Future<void> Function(DesktopThemePreset)? onDesktopThemeChanged;
  final bool closeToTray;
  final Future<void> Function(Locale? locale) onLocaleChanged;
  final Future<void> Function(bool enabled) onCloseToTrayChanged;

  @override
  Widget build(BuildContext context) {
    return SettingsSectionCard(
      title: l10n.settingsAppearanceTitle,
      icon: Icons.palette_outlined,
      subtitle: '让界面更符合你的使用习惯',
      children: [
        SettingsSelectField<Locale?>(
          decoration: InputDecoration(
            labelText: l10n.settingsLanguage,
            helperText: '选择界面显示语言',
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
        if (onDesktopThemeChanged != null)
          SettingsSelectField<DesktopThemePreset>(
            key: const ValueKey('desktop-theme-select'),
            initialValue: desktopTheme,
            decoration: const InputDecoration(
              labelText: '主界面配色',
              helperText: '固定配色；播放详情仍跟随封面',
            ),
            items: [
              for (final preset in DesktopThemePreset.values)
                DropdownMenuItem(value: preset, child: Text(preset.label)),
            ],
            onChanged: (value) async {
              if (value != null) await onDesktopThemeChanged!(value);
            },
          ),
        if (Platform.isWindows || Platform.isLinux || Platform.isMacOS)
          SettingsToggleRow(
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
