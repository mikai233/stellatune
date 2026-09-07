import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_section_card.dart';

class SettingsLyricsCacheSection extends StatelessWidget {
  const SettingsLyricsCacheSection({
    super.key,
    required this.l10n,
    required this.onClearLyricsCache,
  });

  final AppLocalizations l10n;
  final Future<void> Function() onClearLyricsCache;

  @override
  Widget build(BuildContext context) {
    return SettingsSectionCard(
      title: l10n.settingsLyricsTitle,
      icon: Icons.lyrics_outlined,
      subtitle: '管理本地歌词缓存',
      headerBottomSpacing: 8,
      children: [
        Text(
          l10n.settingsLyricsCacheSubtitle,
          style: Theme.of(context).textTheme.bodyMedium,
        ),
        const SizedBox(height: 10),
        Align(
          alignment: Alignment.centerRight,
          child: OutlinedButton.icon(
            onPressed: onClearLyricsCache,
            icon: const Icon(Icons.delete_sweep_outlined),
            label: Text(l10n.settingsClearLyricsCache),
          ),
        ),
      ],
    );
  }
}
