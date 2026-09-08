import 'dart:async';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/output_settings_controller.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/lyrics/lyrics_controller.dart';
import 'package:stellatune/plugins/plugin_settings_controller.dart';
import 'package:stellatune/ui/pages/settings/desktop_settings_view.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_appearance_section.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_library_section.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_lyrics_cache_section.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_output_section.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_plugins_section.dart';

class SettingsPage extends ConsumerStatefulWidget {
  const SettingsPage({super.key, this.useGlobalTopBar = false});
  final bool useGlobalTopBar;
  @override
  ConsumerState<SettingsPage> createState() => SettingsPageState();
}

/// Composes settings panels. Runtime operations are owned by app controllers.
class SettingsPageState extends ConsumerState<SettingsPage> {
  @override
  void initState() {
    super.initState();
    final output = ref.read(outputSettingsControllerProvider.notifier);
    final plugins = ref.read(pluginSettingsControllerProvider.notifier);
    // Loading choices never changes the confirmed output route.
    Future.microtask(() {
      if (!mounted) return;
      unawaited(output.refresh());
      unawaited(plugins.refresh());
    });
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final settings = ref.watch(settingsStoreProvider);
    final settingsController = ref.read(settingsStoreProvider.notifier);
    final appearance = SettingsAppearanceSection(
      l10n: l10n,
      locale: settings.locale,
      desktopTheme: settings.desktopTheme,
      closeToTray: settings.closeToTray,
      onDesktopThemeChanged: settingsController.setDesktopTheme,
      onLocaleChanged: settingsController.setLocale,
      onCloseToTrayChanged: settingsController.setCloseToTray,
    );
    final lyrics = SettingsLyricsCacheSection(
      l10n: l10n,
      onClearLyricsCache: _clearLyricsCache,
    );
    if (!widget.useGlobalTopBar) {
      return Scaffold(
        appBar: AppBar(title: Text(l10n.settingsTitle)),
        body: ListView(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
          children: [
            appearance,
            const SizedBox(height: 12),
            const SettingsOutputSection(includePlayback: true),
            const SizedBox(height: 12),
            const SettingsPluginsSection(),
            const SizedBox(height: 12),
            lyrics,
          ],
        ),
      );
    }
    final library = ref.watch(libraryControllerProvider);
    return DesktopSettingsView(
      panels: [
        SettingsPanel(
          id: 'appearance',
          keywords:
              '外观 appearance 语言 language 主题 theme 托盘 tray ${l10n.settingsAppearanceTitle}',
          child: appearance,
        ),
        const SettingsPanel(
          id: 'playback',
          keywords: '播放 playback 淡入淡出 fade 延迟 latency 缓冲 buffer',
          child: SettingsPlaybackSection(),
        ),
        const SettingsPanel(
          id: 'audio',
          keywords: '音频 audio 输出 output 后端 backend 设备 device 重采样 resample 独占 exclusive 无缝 gapless',
          child: SettingsOutputSection(),
        ),
        SettingsPanel(
          id: 'library',
          keywords: '音乐库 library 文件夹 folder 扫描 scan',
          child: SettingsLibrarySection(
            roots: library.roots,
            isScanning: library.isScanning,
            status:
                library.lastError ??
                (library.isScanning
                    ? '已扫描 ${library.progress.scanned} 项'
                    : null),
            onAdd: () async {
              final path = await FilePicker.getDirectoryPath(
                dialogTitle: l10n.dialogSelectMusicFolder,
              );
              if (path == null || !mounted) return;
              await ref
                  .read(libraryControllerProvider.notifier)
                  .addRoot(path, scanAfter: true);
            },
            onRemove: (path) =>
                ref.read(libraryControllerProvider.notifier).removeRoot(path),
            onScan: (force) => ref
                .read(libraryControllerProvider.notifier)
                .scanAll(force: force),
          ),
        ),
        SettingsPanel(
          id: 'plugins',
          keywords: '插件 plugins 扩展 安装 install ${l10n.settingsPluginsTitle}',
          child: const SettingsPluginsSection(),
        ),
        SettingsPanel(
          id: 'lyrics',
          keywords: '歌词 lyrics 缓存 cache 清理 ${l10n.settingsLyricsTitle}',
          child: lyrics,
        ),
      ],
    );
  }

  Future<void> _clearLyricsCache() async {
    final l10n = AppLocalizations.of(context)!;
    var message = l10n.settingsClearLyricsCacheDone;
    try {
      await ref.read(lyricsControllerProvider.notifier).clearCache();
    } catch (error, stack) {
      logger.e('failed to clear lyrics cache', error: error, stackTrace: stack);
      message = l10n.settingsClearLyricsCacheFailed;
    }
    if (!mounted) return;
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(message)));
  }
}
