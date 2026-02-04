import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';

class SettingsPage extends ConsumerStatefulWidget {
  const SettingsPage({super.key});

  @override
  ConsumerState<SettingsPage> createState() => _SettingsPageState();
}

class _InstalledPlugin {
  const _InstalledPlugin({
    required this.dirPath,
    required this.id,
    required this.name,
  });

  final String dirPath;
  final String? id;
  final String? name;

  String get nameOrDir => name ?? p.basename(dirPath);
}

class _PluginTile extends StatelessWidget {
  const _PluginTile({
    required this.plugin,
    required this.enabled,
    required this.onToggle,
    required this.onUninstall,
  });

  final _InstalledPlugin plugin;
  final bool enabled;
  final ValueChanged<bool>? onToggle;
  final VoidCallback onUninstall;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final title = plugin.nameOrDir;
    final subtitle = plugin.id ?? p.basename(plugin.dirPath);
    return ListTile(
      dense: true,
      leading: Icon(
        Icons.extension,
        color: enabled ? null : theme.colorScheme.onSurfaceVariant,
      ),
      title: Text(title),
      subtitle: Text(subtitle),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Switch(value: enabled, onChanged: onToggle),
          IconButton(
            tooltip: AppLocalizations.of(context)!.uninstall,
            onPressed: onUninstall,
            icon: const Icon(Icons.delete_outline),
          ),
        ],
      ),
    );
  }
}

class _SettingsPageState extends ConsumerState<SettingsPage> {
  Future<List<PluginDescriptor>>? _pluginsFuture;
  Future<List<DspTypeDescriptor>>? _dspTypesFuture;
  Future<List<_InstalledPlugin>>? _installedPluginsFuture;
  String? _pluginDir;

  bool _gainEnabled = false;
  double _gain = 1.0;

  @override
  void initState() {
    super.initState();
    _loadFromSettings();
    _refresh();
  }

  void _loadFromSettings() {
    final chain = ref.read(settingsStoreProvider).dspChain;
    var enabled = false;
    var gain = 1.0;
    for (final item in chain) {
      if (item.typeId == 'gain') {
        enabled = true;
        gain = _parseGain(item.configJson) ?? 1.0;
        break;
      }
    }
    _gainEnabled = enabled;
    _gain = gain;
  }

  void _refresh() {
    final bridge = ref.read(playerBridgeProvider);
    _pluginsFuture = bridge.pluginsList();
    _dspTypesFuture = bridge.dspListTypes();
    _installedPluginsFuture = _listInstalledPlugins();
  }

  Future<void> _ensurePluginDir() async {
    _pluginDir ??= await defaultPluginDir();
  }

  Future<List<_InstalledPlugin>> _listInstalledPlugins() async {
    await _ensurePluginDir();
    final root = Directory(_pluginDir!);
    if (!await root.exists()) return const [];

    final out = <_InstalledPlugin>[];
    await for (final entity in root.list(
      recursive: false,
      followLinks: false,
    )) {
      if (entity is! Directory) continue;
      final manifest = File(p.join(entity.path, 'plugin.toml'));
      if (!await manifest.exists()) continue;
      final text = await manifest.readAsString();
      final id = _parseTomlString(text, 'id');
      final name = _parseTomlString(text, 'name');
      out.add(_InstalledPlugin(dirPath: entity.path, id: id, name: name));
    }
    out.sort((a, b) => (a.nameOrDir).compareTo(b.nameOrDir));
    return out;
  }

  String? _parseTomlString(String toml, String key) {
    final re = RegExp(
      '^\\s*${RegExp.escape(key)}\\s*=\\s*"([^"]*)"\\s*\$',
      multiLine: true,
    );
    final m = re.firstMatch(toml);
    if (m == null) return null;
    final v = (m.group(1) ?? '').trim();
    return v.isEmpty ? null : v;
  }

  Future<void> _installPluginFolder() async {
    final l10n = AppLocalizations.of(context)!;
    await _ensurePluginDir();
    final pluginDir = _pluginDir!;

    final src = await FilePicker.platform.getDirectoryPath(
      dialogTitle: l10n.settingsInstallPluginPickFolder,
    );
    if (src == null || src.trim().isEmpty) return;

    final srcDir = Directory(src);
    if (!await srcDir.exists()) return;
    final manifest = File(p.join(srcDir.path, 'plugin.toml'));
    if (!await manifest.exists()) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsInstallPluginMissingManifest)),
      );
      return;
    }

    final destRoot = Directory(pluginDir);
    await destRoot.create(recursive: true);

    final dest = Directory(p.join(pluginDir, p.basename(src)));
    if (await dest.exists()) {
      await dest.delete(recursive: true);
    }
    await _copyDir(srcDir, dest);

    final bridge = ref.read(playerBridgeProvider);
    final disabledIds = ref.read(settingsStoreProvider).disabledPluginIds;
    await writeDisabledPluginsFile(
      pluginDir: pluginDir,
      disabledIds: disabledIds,
    );
    await bridge.pluginsReloadWithDisabled(
      dir: pluginDir,
      disabledIds: disabledIds.toList(),
    );
    setState(_refresh);

    if (!mounted) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(l10n.settingsPluginInstalled)));
  }

  Future<void> _copyDir(Directory src, Directory dest) async {
    await dest.create(recursive: true);
    await for (final entity in src.list(recursive: false, followLinks: false)) {
      final name = p.basename(entity.path);
      final newPath = p.join(dest.path, name);
      if (entity is File) {
        await entity.copy(newPath);
      } else if (entity is Directory) {
        await _copyDir(entity, Directory(newPath));
      }
    }
  }

  double? _parseGain(String json) {
    final m = RegExp(r'"gain"\\s*:\\s*([0-9]+(?:\\.[0-9]+)?)').firstMatch(json);
    if (m == null) return null;
    return double.tryParse(m.group(1) ?? '');
  }

  List<DspChainItem> _buildChain(List<DspTypeDescriptor> types) {
    if (!_gainEnabled) return const [];
    final gainType = types.firstWhere(
      (t) => t.typeId == 'gain',
      orElse: () => const DspTypeDescriptor(
        pluginId: '',
        pluginName: '',
        typeId: '',
        displayName: '',
        configSchemaJson: '',
        defaultConfigJson: '',
      ),
    );
    if (gainType.pluginId.isEmpty) return const [];
    final config = '{ "gain": ${_gain.toStringAsFixed(3)} }';
    return [
      DspChainItem(
        pluginId: gainType.pluginId,
        typeId: gainType.typeId,
        configJson: config,
      ),
    ];
  }

  Future<void> _apply() async {
    final bridge = ref.read(playerBridgeProvider);
    final settings = ref.read(settingsStoreProvider);
    final types = await (_dspTypesFuture ?? bridge.dspListTypes());
    final chain = _buildChain(types);
    await settings.setDspChain(chain);
    await bridge.dspSetChain(chain);
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(AppLocalizations.of(context)!.settingsApplied)),
    );
  }

  Future<void> _reset() async {
    setState(() {
      _gainEnabled = false;
      _gain = 1.0;
    });
    await _apply();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final bridge = ref.read(playerBridgeProvider);
    _pluginsFuture ??= bridge.pluginsList();
    _dspTypesFuture ??= bridge.dspListTypes();
    _installedPluginsFuture ??= _listInstalledPlugins();

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.settingsTitle),
        actions: [
          IconButton(
            tooltip: l10n.settingsInstallPlugin,
            onPressed: _installPluginFolder,
            icon: const Icon(Icons.add),
          ),
          IconButton(
            tooltip: l10n.refresh,
            onPressed: () => setState(_refresh),
            icon: const Icon(Icons.refresh),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
        children: [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    l10n.settingsPluginsTitle,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 6),
                  FutureBuilder<String>(
                    future: defaultPluginDir(),
                    builder: (context, snap) {
                      final dir = snap.data;
                      if (dir == null) return const SizedBox.shrink();
                      return Text(
                        '${l10n.settingsPluginDir}: $dir',
                        style: Theme.of(context).textTheme.bodySmall,
                      );
                    },
                  ),
                  const SizedBox(height: 8),
                  FutureBuilder<List<_InstalledPlugin>>(
                    future: _installedPluginsFuture,
                    builder: (context, snap) {
                      if (snap.connectionState != ConnectionState.done) {
                        return const LinearProgressIndicator();
                      }
                      final items = snap.data ?? const [];
                      if (items.isEmpty) {
                        return Text(l10n.settingsNoPlugins);
                      }

                      final disabled = ref
                          .read(settingsStoreProvider)
                          .disabledPluginIds;

                      return Column(
                        children: [
                          for (final p in items)
                            _PluginTile(
                              plugin: p,
                              enabled: p.id == null
                                  ? true
                                  : !disabled.contains(p.id),
                              onToggle: p.id == null
                                  ? null
                                  : (v) async {
                                      await ref
                                          .read(settingsStoreProvider)
                                          .setPluginEnabled(
                                            pluginId: p.id!,
                                            enabled: v,
                                          );
                                      await _ensurePluginDir();
                                      final disabledIds = ref
                                          .read(settingsStoreProvider)
                                          .disabledPluginIds;
                                      await writeDisabledPluginsFile(
                                        pluginDir: _pluginDir!,
                                        disabledIds: disabledIds,
                                      );
                                      await bridge.pluginsReloadWithDisabled(
                                        dir: _pluginDir!,
                                        disabledIds: disabledIds.toList(),
                                      );
                                      if (mounted) setState(_refresh);
                                    },
                              onUninstall: () async {
                                final ok = await showDialog<bool>(
                                  context: context,
                                  builder: (context) => AlertDialog(
                                    title: Text(l10n.settingsUninstallPlugin),
                                    content: Text(
                                      l10n.settingsUninstallPluginConfirm(
                                        p.nameOrDir,
                                      ),
                                    ),
                                    actions: [
                                      TextButton(
                                        onPressed: () =>
                                            Navigator.of(context).pop(false),
                                        child: Text(l10n.cancel),
                                      ),
                                      FilledButton(
                                        onPressed: () =>
                                            Navigator.of(context).pop(true),
                                        child: Text(l10n.uninstall),
                                      ),
                                    ],
                                  ),
                                );
                                if (ok != true) return;

                                try {
                                  await Directory(
                                    p.dirPath,
                                  ).delete(recursive: true);
                                  if (p.id != null) {
                                    await ref
                                        .read(settingsStoreProvider)
                                        .setPluginEnabled(
                                          pluginId: p.id!,
                                          enabled: true,
                                        );
                                  }
                                  await _ensurePluginDir();
                                  final disabledIds = ref
                                      .read(settingsStoreProvider)
                                      .disabledPluginIds;
                                  await writeDisabledPluginsFile(
                                    pluginDir: _pluginDir!,
                                    disabledIds: disabledIds,
                                  );
                                  await bridge.pluginsReloadWithDisabled(
                                    dir: _pluginDir!,
                                    disabledIds: disabledIds.toList(),
                                  );
                                  if (!context.mounted) return;
                                  setState(_refresh);
                                  ScaffoldMessenger.of(context).showSnackBar(
                                    SnackBar(
                                      content: Text(
                                        AppLocalizations.of(
                                          context,
                                        )!.settingsPluginUninstalled,
                                      ),
                                    ),
                                  );
                                } catch (_) {
                                  if (!context.mounted) return;
                                  ScaffoldMessenger.of(context).showSnackBar(
                                    SnackBar(
                                      content: Text(
                                        AppLocalizations.of(
                                          context,
                                        )!.settingsUninstallPluginFailed,
                                      ),
                                    ),
                                  );
                                }
                              },
                            ),
                        ],
                      );
                    },
                  ),
                  const Divider(height: 24),
                  FutureBuilder<List<PluginDescriptor>>(
                    future: _pluginsFuture,
                    builder: (context, snap) {
                      if (snap.connectionState != ConnectionState.done) {
                        return const LinearProgressIndicator();
                      }
                      final plugins = snap.data ?? const [];
                      if (plugins.isEmpty) {
                        return Text(l10n.settingsNoLoadedPlugins);
                      }
                      return Column(
                        children: [
                          for (final p in plugins)
                            ListTile(
                              dense: true,
                              leading: const Icon(Icons.extension),
                              title: Text(p.name),
                              subtitle: Text(p.id),
                            ),
                        ],
                      );
                    },
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 12),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: FutureBuilder<List<DspTypeDescriptor>>(
                future: _dspTypesFuture,
                builder: (context, snap) {
                  if (snap.connectionState != ConnectionState.done) {
                    return const LinearProgressIndicator();
                  }
                  final types = snap.data ?? const [];
                  final hasGain = types.any((t) => t.typeId == 'gain');
                  return Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        l10n.settingsDspTitle,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 8),
                      SwitchListTile(
                        title: Text(l10n.settingsEnableGain),
                        subtitle: hasGain
                            ? Text(l10n.settingsExamplePluginNote)
                            : Text(l10n.settingsNoGainFound),
                        value: _gainEnabled && hasGain,
                        onChanged: hasGain
                            ? (v) => setState(() => _gainEnabled = v)
                            : null,
                      ),
                      ListTile(
                        title: Text(
                          '${l10n.settingsGain}: ${_gain.toStringAsFixed(2)}x',
                        ),
                        subtitle: Slider(
                          value: _gain.clamp(0.0, 4.0),
                          min: 0.0,
                          max: 4.0,
                          divisions: 80,
                          onChanged: (_gainEnabled && hasGain)
                              ? (v) => setState(() => _gain = v)
                              : null,
                        ),
                      ),
                      const SizedBox(height: 8),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          TextButton(
                            onPressed: _reset,
                            child: Text(l10n.reset),
                          ),
                          const SizedBox(width: 8),
                          FilledButton(
                            onPressed: _apply,
                            child: Text(l10n.apply),
                          ),
                        ],
                      ),
                    ],
                  );
                },
              ),
            ),
          ),
        ],
      ),
    );
  }
}
