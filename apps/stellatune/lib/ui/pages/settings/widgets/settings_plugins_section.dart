import 'dart:io';

import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/output_settings_controller.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/plugins/plugin_settings_controller.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_plugins_list.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_section_card.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:window_manager/window_manager.dart';
import 'package:windows_file_picker/windows_file_picker.dart';

class SettingsPluginsSection extends ConsumerStatefulWidget {
  const SettingsPluginsSection({super.key});
  @override
  ConsumerState<SettingsPluginsSection> createState() =>
      _SettingsPluginsSectionState();
}

class _SettingsPluginsSectionState
    extends ConsumerState<SettingsPluginsSection> {
  bool _picking = false;

  void _feedback(String text) {
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
    }
  }

  Future<void> _install() async {
    if (_picking) return;
    final l10n = AppLocalizations.of(context)!;
    final controller = ref.read(pluginSettingsControllerProvider.notifier);
    setState(() => _picking = true);
    try {
      // Resolve the app's window before the native picker starts its isolate.
      final parent = Platform.isWindows ? await windowManager.getId() : null;
      if (!mounted) return;
      final picked = await FilePicker.pickFile(
        dialogTitle: l10n.settingsInstallPluginPickFolder,
        type: FileType.custom,
        allowedExtensions: ['zip'],
        windowsOptions: FilePickerWindowsOptions(
          lockParentWindow: true,
          parentWindowHandle: parent,
        ),
        linuxOptions: const LinuxOptions(lockParentWindow: true),
      );
      if (!mounted || picked == null) return;
      final path = picked.path?.trim();
      if (path == null || path.isEmpty) {
        throw StateError('Selected file path is empty.');
      }
      await controller.install(path);
      _feedback(l10n.settingsPluginInstalled);
    } catch (error, stack) {
      logger.e('failed to install plugin', error: error, stackTrace: stack);
      DiagnosticsService.instance.report(
        error,
        stack: stack,
        operation: 'plugin_install',
      );
    } finally {
      if (mounted) setState(() => _picking = false);
    }
  }

  Future<void> _openDirectory(String directory) async {
    try {
      if (Platform.isWindows) {
        await Process.start('explorer.exe', [
          directory,
        ], mode: ProcessStartMode.detached);
      } else if (!await launchUrl(Uri.directory(directory))) {
        throw StateError('failed to open plugin directory');
      }
    } catch (error) {
      DiagnosticsService.instance.report(error, operation: 'plugin_directory');
    }
  }

  Future<void> _openUi({
    required String pluginId,
    required String pluginName,
  }) async {
    try {
      final raw = await ref
          .read(pluginSettingsControllerProvider.notifier)
          .openUi(pluginId);
      final uri = Uri.tryParse(raw.trim());
      if (uri == null || !uri.hasScheme) {
        throw StateError('invalid plugin UI URL: $raw');
      }
      if (!await launchUrl(uri, mode: LaunchMode.externalApplication) &&
          !await launchUrl(uri)) {
        throw StateError('failed to launch browser');
      }
    } catch (error) {
      DiagnosticsService.instance.report(error, operation: 'plugin_ui');
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final state = ref.watch(pluginSettingsControllerProvider);
    final output = ref.watch(outputSettingsControllerProvider);
    final controller = ref.read(pluginSettingsControllerProvider.notifier);
    final outputController = ref.read(
      outputSettingsControllerProvider.notifier,
    );
    final busy = state.busy || _picking;
    return SettingsSectionCard(
      title: l10n.settingsPluginsTitle,
      icon: Icons.extension_outlined,
      subtitle: '管理音源、解码器与输出扩展',
      headerBottomSpacing: 6,
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            visualDensity: VisualDensity.compact,
            tooltip: l10n.settingsInstallPlugin,
            onPressed: busy ? null : _install,
            icon: const Icon(Icons.add),
          ),
          IconButton(
            visualDensity: VisualDensity.compact,
            tooltip: l10n.refresh,
            onPressed: busy
                ? null
                : () async {
                    await Future.wait([
                      controller.refresh(),
                      outputController.refresh(),
                    ]);
                  },
            icon: const Icon(Icons.refresh),
          ),
          if (state.directory case final directory?)
            IconButton(
              visualDensity: VisualDensity.compact,
              tooltip: l10n.settingsOpenPluginDir,
              onPressed: busy ? null : () => _openDirectory(directory),
              icon: const Icon(Icons.folder_open_outlined),
            ),
        ],
      ),
      children: [
        if (state.directory case final directory?)
          Text(
            '${l10n.settingsPluginDir}: $directory',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        const SizedBox(height: 8),
        if (state.loading || state.busy) const LinearProgressIndicator(),
        if (!state.loading && state.plugins.isEmpty)
          Text(l10n.settingsNoPlugins),
        AbsorbPointer(
          absorbing: busy,
          child: SettingsPluginsList(
            plugins: state.plugins,
            disabledPluginIds: state.disabledIds,
            loadedPluginIds: state.loadedIds,
            loadedKnown: state.loaded,
            sourceTypes: state.sourceTypes,
            outputTypes: output.types,
            onOpenWebUi: _openUi,
            onToggleEnabled: controller.setEnabled,
            onUninstall: (plugin) async {
              await controller.uninstall(plugin);
              _feedback(l10n.settingsPluginUninstalled);
            },
            outputSinkConfigForType: outputController.configForType,
            onOutputSinkConfigChanged: outputController.updateConfig,
          ),
        ),
      ],
    );
  }
}
