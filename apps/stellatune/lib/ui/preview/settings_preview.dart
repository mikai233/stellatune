import 'package:stellatune/ui/theme/desktop_theme.dart';
import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/settings/desktop_settings_view.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_appearance_section.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_form_row.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_library_section.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_lyrics_cache_section.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_section_card.dart';

class SettingsVisualPreview extends StatefulWidget {
  const SettingsVisualPreview({
    super.key,
    this.desktopTheme = DesktopThemePreset.daylight,
    this.onDesktopThemeChanged,
  });
  final DesktopThemePreset desktopTheme;
  final ValueChanged<DesktopThemePreset>? onDesktopThemeChanged;
  @override
  State<SettingsVisualPreview> createState() => _SettingsVisualPreviewState();
}

class _SettingsVisualPreviewState extends State<SettingsVisualPreview> {
  Locale? locale;
  bool tray = true, fade = true;
  String latency = '低',
      quality = '高',
      backend = '共享 (WASAPI Shared)',
      device = '系统默认';
  final roots = ['D:/CloudMusic/Music', 'D:/Music'];
  Widget select(
    String label,
    String value,
    List<String> values,
    ValueChanged<String> onChanged, {
    String? hint,
  }) => SettingsSelectField<String>(
    decoration: InputDecoration(labelText: label, helperText: hint),
    initialValue: value,
    items: values
        .map((value) => DropdownMenuItem(value: value, child: Text(value)))
        .toList(),
    onChanged: (value) {
      if (value != null) setState(() => onChanged(value));
    },
  );
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return DesktopSettingsView(
      panels: [
        SettingsPanel(
          id: 'appearance',
          keywords: '外观 appearance 语言 language 主题 theme 托盘 tray',
          child: SettingsAppearanceSection(
            l10n: l10n,
            locale: locale,
            desktopTheme: widget.desktopTheme,
            onDesktopThemeChanged: (value) async =>
                widget.onDesktopThemeChanged?.call(value),
            closeToTray: tray,
            onLocaleChanged: (v) async => setState(() => locale = v),
            onCloseToTrayChanged: (v) async => setState(() => tray = v),
          ),
        ),
        SettingsPanel(
          id: 'playback',
          keywords: '播放 playback 淡入淡出 fade 延迟 latency 缓冲',
          child: SettingsSectionCard(
            title: '播放',
            icon: Icons.play_circle_outline,
            subtitle: '调整播放行为与体验细节',
            children: [
              SettingsToggleRow(
                title: Text(l10n.settingsSeekTrackFade),
                subtitle: const Text('让跳转与切歌时的声音过渡更自然'),
                value: fade,
                onChanged: (value) => setState(() => fade = value),
              ),
              select(
                l10n.settingsPlaybackLatency,
                latency,
                ['低', '中', '高'],
                (v) => latency = v,
                hint: l10n.settingsPlaybackLatencyHint,
              ),
            ],
          ),
        ),
        SettingsPanel(
          id: 'audio',
          keywords: '音频 audio 输出 后端 设备 重采样',
          child: SettingsSectionCard(
            title: l10n.settingsOutputTitle,
            icon: Icons.graphic_eq,
            subtitle: '配置音频输出与音质相关选项',
            children: [
              select(l10n.settingsBackend, backend, [
                '共享 (WASAPI Shared)',
                'WASAPI 独占',
              ], (v) => backend = v),
              select(l10n.settingsDevice, device, [
                '系统默认',
                'Speakers (Realtek Audio)',
              ], (v) => device = v),
              select(l10n.settingsResampleQuality, quality, [
                '快速',
                '均衡',
                '高',
                '极高',
              ], (v) => quality = v),
            ],
          ),
        ),
        SettingsPanel(
          id: 'library',
          keywords: '音乐库 library 文件夹 folder 扫描 scan',
          child: SettingsLibrarySection(
            roots: roots,
            isScanning: false,
            onAdd: () => setState(() => roots.add('D:/New Music')),
            onRemove: (path) => setState(() => roots.remove(path)),
            onScan: (_) {},
          ),
        ),
        SettingsPanel(
          id: 'plugins',
          keywords: '插件 plugins 安装',
          child: SettingsSectionCard(
            title: l10n.settingsPluginsTitle,
            icon: Icons.extension_outlined,
            subtitle: '管理音源、解码器与输出扩展',
            trailing: IconButton(
              tooltip: '安装插件',
              onPressed: () {},
              icon: const Icon(Icons.add),
            ),
            children: const [
              Padding(
                padding: EdgeInsets.all(12),
                child: Text('暂无已安装的插件', style: TextStyle(fontSize: 13)),
              ),
            ],
          ),
        ),
        SettingsPanel(
          id: 'lyrics',
          keywords: '歌词 lyrics 缓存 cache 清理',
          child: SettingsLyricsCacheSection(
            l10n: l10n,
            onClearLyricsCache: () async {},
          ),
        ),
      ],
    );
  }
}
