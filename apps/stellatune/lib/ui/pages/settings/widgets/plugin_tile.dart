import 'package:flutter/material.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/forms/schema_form.dart';
import 'package:stellatune/ui/pages/settings/models/installed_plugin.dart';

class SettingsPluginTile extends StatefulWidget {
  final InstalledPlugin plugin;
  final bool isDisabled;
  final bool isLoaded;
  final bool loadedKnown;
  final List<SourceCatalogTypeDescriptor> pluginSourceTypes;
  final List<OutputSinkTypeDescriptor> pluginOutputSinkTypes;
  final Future<void> Function()? onOpenWebUi;
  final Future<void> Function(bool) onToggleEnabled;
  final Future<void> Function() onUninstall;
  final String? Function(SourceCatalogTypeDescriptor) sourceConfigForType;
  final String? Function(OutputSinkTypeDescriptor) outputSinkConfigForType;
  final void Function(SourceCatalogTypeDescriptor, String)
  onSourceConfigChanged;
  final void Function(OutputSinkTypeDescriptor, String)
  onOutputSinkConfigChanged;
  final Future<void> Function(SourceCatalogTypeDescriptor) onSaveSourceConfig;

  const SettingsPluginTile({
    super.key,
    required this.plugin,
    required this.isDisabled,
    required this.isLoaded,
    required this.loadedKnown,
    required this.pluginSourceTypes,
    required this.pluginOutputSinkTypes,
    required this.onOpenWebUi,
    required this.onToggleEnabled,
    required this.onUninstall,
    required this.sourceConfigForType,
    required this.outputSinkConfigForType,
    required this.onSourceConfigChanged,
    required this.onOutputSinkConfigChanged,
    required this.onSaveSourceConfig,
  });

  @override
  State<SettingsPluginTile> createState() => _SettingsPluginTileState();
}

class _SettingsPluginTileState extends State<SettingsPluginTile> {
  String _uninstallErrorMessage(
    AppLocalizations l10n,
    InstalledPlugin plugin,
    Object error,
  ) {
    final raw = error.toString();
    final lower = raw.toLowerCase();
    final pluginName = plugin.nameOrDir;
    final isZh = Localizations.localeOf(context).languageCode == 'zh';
    final looksBusy =
        lower.contains('still in use') ||
        lower.contains('draining generation') ||
        lower.contains('retired lease') ||
        lower.contains('busy');
    if (looksBusy) {
      if (isZh) {
        return '插件“$pluginName”当前仍在使用中（可能正在播放当前歌曲）。请先停止播放或切换歌曲后重试卸载。';
      }
      return 'Plugin "$pluginName" is still in use (possibly by the current playback). Stop playback or switch tracks, then retry uninstall.';
    }
    final accessDenied =
        lower.contains('拒绝访问') ||
        lower.contains('access is denied') ||
        lower.contains('os error 5');
    if (accessDenied) {
      if (isZh) {
        return '无法卸载插件“$pluginName”：文件仍被占用。请先停止播放后重试。';
      }
      return 'Cannot uninstall plugin "$pluginName": files are still in use. Stop playback and retry.';
    }
    return l10n.settingsUninstallPluginFailed;
  }

  @override
  Widget build(BuildContext context) {
    final p = widget.plugin;
    final l10n = AppLocalizations.of(context)!;
    final hasCustomUi =
        widget.pluginSourceTypes.isNotEmpty ||
        widget.pluginOutputSinkTypes.isNotEmpty;
    final canToggleEnabled = p.id != null && p.isInstalled;
    final isEnabled = p.isInstalled && !widget.isDisabled;
    final canUninstall = !isEnabled || p.isPendingUninstall || p.isDeleteFailed;

    final (statusText, statusIsError) = switch ((
      p.id,
      p.installState,
      widget.isDisabled,
      widget.loadedKnown,
      widget.isLoaded,
    )) {
      (null, _, _, _, _) => ('插件 ID 缺失', true),
      (_, 'pending_uninstall', _, _, _) => (
        '卸载中（后台重试中，${p.uninstallRetryCount} 次）',
        false,
      ),
      (_, 'delete_failed', _, _, _) => (
        '卸载失败（后台重试中，${p.uninstallRetryCount} 次）',
        true,
      ),
      (_, _, true, _, _) => ('已禁用', false),
      (_, _, false, false, _) => ('正在检查加载状态...', false),
      (_, _, false, true, true) => ('已加载', false),
      (_, _, false, true, false) => ('未加载（可能加载失败，请检查日志）', true),
    };

    final Color? pluginIconColor;
    if (p.isPendingUninstall) {
      pluginIconColor = Colors.orange.shade700;
    } else if (p.isDeleteFailed) {
      pluginIconColor = Theme.of(context).colorScheme.error;
    } else if (widget.isDisabled) {
      pluginIconColor = null;
    } else if (statusIsError) {
      pluginIconColor = Theme.of(context).colorScheme.error;
    } else {
      pluginIconColor = Colors.green.shade600;
    }

    Widget buildActions() => Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (widget.onOpenWebUi != null)
          IconButton(
            tooltip: 'Open Web UI',
            onPressed: widget.onOpenWebUi,
            icon: const Icon(Icons.web),
          ),
        Switch(
          value: isEnabled,
          onChanged: !canToggleEnabled
              ? null
              : (v) async {
                  try {
                    await widget.onToggleEnabled(v);
                  } catch (e, s) {
                    logger.e(
                      'failed to toggle plugin state',
                      error: e,
                      stackTrace: s,
                    );
                    if (!context.mounted) return;
                    ScaffoldMessenger.of(context).showSnackBar(
                      SnackBar(content: Text('Failed to reload: $e')),
                    );
                  }
                },
        ),
        IconButton(
          tooltip: l10n.settingsUninstallPlugin,
          onPressed: canUninstall
              ? () async {
                  final ok = await showDialog<bool>(
                    context: context,
                    builder: (context) => AlertDialog(
                      title: Text(l10n.settingsUninstallPlugin),
                      content: Text(
                        l10n.settingsUninstallPluginConfirm(p.nameOrDir),
                      ),
                      actions: [
                        TextButton(
                          onPressed: () => Navigator.of(context).pop(false),
                          child: Text(l10n.cancel),
                        ),
                        FilledButton(
                          onPressed: () => Navigator.of(context).pop(true),
                          child: Text(l10n.uninstall),
                        ),
                      ],
                    ),
                  );
                  if (ok == true) {
                    try {
                      await widget.onUninstall();
                    } catch (e, s) {
                      logger.e(
                        'failed to uninstall plugin',
                        error: e,
                        stackTrace: s,
                      );
                      if (!context.mounted) return;
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                          content: Text(_uninstallErrorMessage(l10n, p, e)),
                        ),
                      );
                    }
                  }
                }
              : null,
          icon: Icon(
            Icons.delete_outline,
            color: canUninstall
                ? Theme.of(context).colorScheme.error
                : Theme.of(context).disabledColor,
          ),
        ),
      ],
    );

    final subtitle = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          p.id ?? p.dirPath,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
            color: Theme.of(
              context,
            ).colorScheme.onSurfaceVariant.withValues(alpha: 0.7),
          ),
        ),
        if (p.infoJson != null && p.infoJson!.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Text(
              p.infoJson!,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        if (p.uninstallLastError != null && p.uninstallLastError!.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Text(
              p.uninstallLastError!,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.error,
              ),
            ),
          ),
        Text(
          statusText,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
            color: statusIsError
                ? Theme.of(context).colorScheme.error
                : Theme.of(context).colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );

    if (!hasCustomUi) {
      return Card(
        margin: const EdgeInsets.only(bottom: 8),
        child: ListTile(
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
          contentPadding: const EdgeInsets.fromLTRB(16, 4, 8, 4),
          leading: Icon(Icons.extension, color: pluginIconColor),
          title: Text(p.nameOrDir),
          subtitle: subtitle,
          trailing: buildActions(),
        ),
      );
    }

    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: Theme(
        data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
        child: ExpansionTile(
          onExpansionChanged: (v) {},
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
          collapsedShape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
          tilePadding: const EdgeInsets.fromLTRB(16, 4, 8, 4),
          leading: Icon(Icons.extension, color: pluginIconColor),
          title: Text(p.nameOrDir),
          subtitle: subtitle,
          trailing: buildActions(),
          childrenPadding: const EdgeInsets.fromLTRB(16, 0, 8, 12),
          children: [
            for (final t in widget.pluginSourceTypes)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Source: ${t.displayName}',
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    const SizedBox(height: 6),
                    SchemaForm(
                      key: ValueKey(
                        'settings-source-config:${t.pluginId}:${t.typeId}',
                      ),
                      schemaJson: t.configSchemaJson,
                      initialValueJson: widget.sourceConfigForType(t) ?? '',
                      onChangedJson: (json) =>
                          widget.onSourceConfigChanged(t, json),
                    ),
                    const SizedBox(height: 6),
                    Align(
                      alignment: Alignment.centerRight,
                      child: Padding(
                        padding: const EdgeInsets.only(right: 8),
                        child: FilledButton.tonal(
                          onPressed: () => widget.onSaveSourceConfig(t),
                          child: Text(l10n.apply),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            for (final t in widget.pluginOutputSinkTypes)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Output: ${t.displayName}',
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    const SizedBox(height: 6),
                    SchemaForm(
                      key: ValueKey(
                        'settings-output-config:${t.pluginId}:${t.typeId}',
                      ),
                      schemaJson: t.configSchemaJson,
                      initialValueJson: widget.outputSinkConfigForType(t) ?? '',
                      onChangedJson: (json) =>
                          widget.onOutputSinkConfigChanged(t, json),
                    ),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }
}
