import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/ui/pages/library_page.dart';
import 'package:stellatune/ui/pages/playlists_page.dart';
import 'package:stellatune/ui/pages/settings_page.dart';
import 'package:stellatune/ui/pages/shell/desktop_shell.dart'
    show DesktopShell, DesktopTopBarAction;
import 'package:stellatune/ui/pages/shell/mobile_shell.dart';
import 'package:stellatune/ui/widgets/open_container_shader_warmup.dart';
import 'package:stellatune/l10n/app_localizations.dart';

class ShellPage extends ConsumerStatefulWidget {
  const ShellPage({super.key});

  @override
  ConsumerState<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends ConsumerState<ShellPage> {
  int _index = 0;
  final _libraryPageKey = GlobalKey<LibraryPageState>();
  final _playlistsPageKey = GlobalKey<PlaylistsPageState>();
  final _settingsPageKey = GlobalKey<SettingsPageState>();

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final isMobile = Platform.isAndroid || Platform.isIOS;
    final isScanning = ref.watch(
      libraryControllerProvider.select((s) => s.isScanning),
    );

    final body = switch (_index) {
      0 => LibraryPage(key: _libraryPageKey, useGlobalTopBar: !isMobile),
      1 => PlaylistsPage(key: _playlistsPageKey, useGlobalTopBar: !isMobile),
      2 => SettingsPage(key: _settingsPageKey, useGlobalTopBar: !isMobile),
      _ => SettingsPage(key: _settingsPageKey, useGlobalTopBar: !isMobile),
    };

    final topBarActions = switch (_index) {
      0 => <DesktopTopBarAction>[
        DesktopTopBarAction(
          icon: (_libraryPageKey.currentState?.foldersPaneCollapsed ?? false)
              ? Icons.chevron_right
              : Icons.chevron_left,
          tooltip: (_libraryPageKey.currentState?.foldersPaneCollapsed ?? false)
              ? l10n.expand
              : l10n.collapse,
          onPressed: () {
            _libraryPageKey.currentState?.toggleFoldersPane();
            setState(() {});
          },
        ),
        DesktopTopBarAction(
          icon: Icons.create_new_folder_outlined,
          tooltip: l10n.tooltipAddFolder,
          onPressed: () => _libraryPageKey.currentState?.addFolderFromTopBar(),
        ),
        DesktopTopBarAction(
          icon: Icons.refresh,
          tooltip: l10n.tooltipScan,
          onPressed: isScanning
              ? null
              : () => _libraryPageKey.currentState?.scanFromTopBar(),
        ),
        DesktopTopBarAction(
          icon: Icons.restart_alt,
          tooltip: l10n.tooltipForceScan,
          onPressed: isScanning
              ? null
              : () => _libraryPageKey.currentState?.scanFromTopBar(force: true),
        ),
      ],
      1 => <DesktopTopBarAction>[
        DesktopTopBarAction(
          icon: (_playlistsPageKey.currentState?.isPlaylistsPanelOpen ?? false)
              ? Icons.menu_open
              : Icons.menu,
          tooltip: l10n.playlistSectionTitle,
          onPressed: () {
            _playlistsPageKey.currentState?.togglePlaylistsPanel();
            setState(() {});
          },
        ),
        DesktopTopBarAction(
          icon: Icons.playlist_add_outlined,
          tooltip: l10n.playlistCreateTooltip,
          onPressed: () =>
              _playlistsPageKey.currentState?.createPlaylistFromTopBar(),
        ),
      ],
      2 => <DesktopTopBarAction>[
        DesktopTopBarAction(
          icon: Icons.add,
          tooltip: l10n.settingsInstallPlugin,
          onPressed: () =>
              _settingsPageKey.currentState?.installPluginFromTopBar(),
        ),
        DesktopTopBarAction(
          icon: Icons.refresh,
          tooltip: l10n.refresh,
          onPressed: () => _settingsPageKey.currentState?.refreshFromTopBar(),
        ),
      ],
      _ => const <DesktopTopBarAction>[],
    };

    final shell = isMobile
        ? MobileShell(
            selectedIndex: _index,
            onDestinationSelected: (v) => setState(() => _index = v),
            child: body,
          )
        : DesktopShell(
            selectedIndex: _index,
            onDestinationSelected: (v) => setState(() => _index = v),
            topBarActions: topBarActions,
            child: body,
          );

    return Stack(children: [shell, const OpenContainerShaderWarmup()]);
  }
}
