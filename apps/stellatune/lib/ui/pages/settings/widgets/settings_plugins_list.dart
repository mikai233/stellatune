import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/ui/pages/settings/models/installed_plugin.dart';
import 'package:stellatune/ui/pages/settings/widgets/plugin_tile.dart';

class SettingsPluginsList extends StatelessWidget {
  const SettingsPluginsList({
    super.key,
    required this.plugins,
    required this.disabledPluginIds,
    required this.loadedPluginIds,
    required this.loadedKnown,
    required this.sourceTypes,
    required this.outputTypes,
    required this.onOpenWebUi,
    required this.onToggleEnabled,
    required this.onUninstall,
    required this.sourceConfigForType,
    required this.outputSinkConfigForType,
    required this.onSourceConfigChanged,
    required this.onOutputSinkConfigChanged,
    required this.onSaveSourceConfig,
  });

  final List<InstalledPlugin> plugins;
  final Set<String> disabledPluginIds;
  final Set<String> loadedPluginIds;
  final bool loadedKnown;
  final List<SourceCatalogTypeDescriptor> sourceTypes;
  final List<OutputSinkTypeDescriptor> outputTypes;
  final Future<void> Function({
    required String pluginId,
    required String pluginName,
  })
  onOpenWebUi;
  final Future<void> Function({
    required InstalledPlugin plugin,
    required bool enabled,
  })
  onToggleEnabled;
  final Future<void> Function(InstalledPlugin plugin) onUninstall;
  final String? Function(SourceCatalogTypeDescriptor) sourceConfigForType;
  final String? Function(OutputSinkTypeDescriptor) outputSinkConfigForType;
  final void Function(SourceCatalogTypeDescriptor, String)
  onSourceConfigChanged;
  final void Function(OutputSinkTypeDescriptor, String)
  onOutputSinkConfigChanged;
  final Future<void> Function(SourceCatalogTypeDescriptor) onSaveSourceConfig;

  @override
  Widget build(BuildContext context) {
    final sourceByPlugin = <String, List<SourceCatalogTypeDescriptor>>{};
    for (final t in sourceTypes) {
      sourceByPlugin
          .putIfAbsent(t.pluginId, () => <SourceCatalogTypeDescriptor>[])
          .add(t);
    }

    final outputByPlugin = <String, List<OutputSinkTypeDescriptor>>{};
    for (final t in outputTypes) {
      outputByPlugin
          .putIfAbsent(t.pluginId, () => <OutputSinkTypeDescriptor>[])
          .add(t);
    }

    return Column(
      children: [
        for (final plugin in plugins)
          SettingsPluginTile(
            plugin: plugin,
            isDisabled: plugin.id != null
                ? disabledPluginIds.contains(plugin.id)
                : false,
            isLoaded: plugin.id != null
                ? loadedPluginIds.contains(plugin.id)
                : false,
            loadedKnown: loadedKnown,
            pluginSourceTypes: plugin.id == null
                ? const []
                : (sourceByPlugin[plugin.id] ?? const []),
            pluginOutputSinkTypes: plugin.id == null
                ? const []
                : (outputByPlugin[plugin.id] ?? const []),
            onOpenWebUi: plugin.id == null || !plugin.hasWebUi
                ? null
                : () => onOpenWebUi(
                    pluginId: plugin.id!,
                    pluginName: plugin.nameOrDir,
                  ),
            onToggleEnabled: (enabled) =>
                onToggleEnabled(plugin: plugin, enabled: enabled),
            onUninstall: () => onUninstall(plugin),
            sourceConfigForType: sourceConfigForType,
            outputSinkConfigForType: outputSinkConfigForType,
            onSourceConfigChanged: onSourceConfigChanged,
            onOutputSinkConfigChanged: onOutputSinkConfigChanged,
            onSaveSourceConfig: onSaveSourceConfig,
          ),
      ],
    );
  }
}
