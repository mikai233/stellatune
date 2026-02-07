import 'dart:io';
import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/lyrics/lyrics_controller.dart';
import 'package:stellatune/ui/forms/schema_form.dart';

class SettingsPage extends ConsumerStatefulWidget {
  const SettingsPage({super.key, this.useGlobalTopBar = false});

  final bool useGlobalTopBar;

  @override
  ConsumerState<SettingsPage> createState() => SettingsPageState();
}

class _InstalledPlugin {
  const _InstalledPlugin({
    required this.dirPath,
    required this.id,
    required this.name,
    required this.infoJson,
  });

  final String dirPath;
  final String? id;
  final String? name;
  final String? infoJson;

  String get nameOrDir => name ?? p.basename(dirPath);
}

enum _OutputMode { device, plugin }

class SettingsPageState extends ConsumerState<SettingsPage> {
  Future<List<PluginDescriptor>>? _pluginsFuture;
  Future<List<OutputSinkTypeDescriptor>>? _outputSinkTypesFuture;
  Future<List<SourceCatalogTypeDescriptor>>? _sourceTypesFuture;
  Future<List<_InstalledPlugin>>? _installedPluginsFuture;
  List<AudioDevice> _devices = [];
  StreamSubscription<Event>? _eventSub;
  String? _pluginDir;

  _OutputMode _outputMode = _OutputMode.device;
  String? _selectedOutputSinkTypeKey;
  final TextEditingController _outputSinkConfigController =
      TextEditingController(text: '{}');
  final TextEditingController _outputSinkTargetController =
      TextEditingController(text: '{}');
  List<Object?> _outputSinkTargets = const [];
  bool _loadingOutputSinkTargets = false;
  final Map<String, String> _sourceConfigDrafts = <String, String>{};
  Set<String> _cachedLoadedPluginIds = <String>{};
  bool _cachedLoadedPluginIdsReady = false;
  List<SourceCatalogTypeDescriptor> _cachedSourceTypes = const [];
  bool _cachedSourceTypesReady = false;

  Future<void> installPluginFromTopBar() => _installPluginArtifact();

  void refreshFromTopBar() => setState(_refresh);

  @override
  void initState() {
    super.initState();
    _loadFromSettings();
    _refresh();
    _initEvents();
  }

  @override
  void dispose() {
    _eventSub?.cancel();
    _outputSinkConfigController.dispose();
    _outputSinkTargetController.dispose();
    super.dispose();
  }

  void _initEvents() {
    final bridge = ref.read(playerBridgeProvider);
    _eventSub = bridge.events().listen((event) {
      if (!mounted) return;
      event.whenOrNull(
        outputDevicesChanged: (devices) {
          setState(() {
            _devices = devices;
          });
        },
      );
    });
    bridge.refreshDevices();
  }

  void _loadFromSettings() {
    final settings = ref.read(settingsStoreProvider);
    final route = settings.outputSinkRoute;
    _outputMode = route == null ? _OutputMode.device : _OutputMode.plugin;
    _selectedOutputSinkTypeKey = route == null
        ? null
        : '${route.pluginId}::${route.typeId}';
    _outputSinkConfigController.text = route?.configJson ?? '{}';
    _outputSinkTargetController.text = route?.targetJson ?? '{}';
  }

  void _refresh() {
    final bridge = ref.read(playerBridgeProvider);
    _pluginsFuture = bridge.pluginsList();
    _outputSinkTypesFuture = null;
    _sourceTypesFuture = bridge.sourceListTypes();
    _installedPluginsFuture = _listInstalledPlugins();
  }

  void _refreshPluginRuntimeState() {
    final bridge = ref.read(playerBridgeProvider);
    _pluginsFuture = bridge.pluginsList();
    _sourceTypesFuture = bridge.sourceListTypes();
  }

  Future<void> _ensurePluginDir() async {
    _pluginDir ??= await defaultPluginDir();
  }

  Future<List<_InstalledPlugin>> _listInstalledPlugins() async {
    await _ensurePluginDir();
    final bridge = ref.read(playerBridgeProvider);
    final raw = await bridge.pluginsListInstalledJson(dir: _pluginDir!);
    final decoded = jsonDecode(raw);
    if (decoded is! List) return const [];
    final out = <_InstalledPlugin>[];
    for (final item in decoded) {
      if (item is! Map) continue;
      final map = item.cast<Object?, Object?>();
      final id = (map['id'] ?? '').toString().trim();
      if (id.isEmpty) continue;
      final dirPath = (map['root_dir'] ?? '').toString().trim();
      final nameRaw = (map['name'] ?? '').toString().trim();
      final infoRaw = (map['info_json'] ?? '').toString().trim();
      out.add(
        _InstalledPlugin(
          dirPath: dirPath.isEmpty ? p.join(_pluginDir!, id) : dirPath,
          id: id,
          name: nameRaw.isEmpty ? null : nameRaw,
          infoJson: infoRaw.isEmpty ? null : infoRaw,
        ),
      );
    }
    out.sort((a, b) => (a.nameOrDir).compareTo(b.nameOrDir));
    return out;
  }

  String _pluginLibExtForPlatform() {
    if (Platform.isWindows) return 'dll';
    if (Platform.isLinux) return 'so';
    if (Platform.isMacOS) return 'dylib';
    return 'dll';
  }

  String _sourceTypeKey(SourceCatalogTypeDescriptor t) =>
      '${t.pluginId}::${t.typeId}';

  String _sourceConfigForType(SourceCatalogTypeDescriptor t) {
    final key = _sourceTypeKey(t);
    final draft = _sourceConfigDrafts[key];
    if (draft != null) return draft;
    final store = ref.read(settingsStoreProvider);
    final value = store.sourceConfigFor(
      pluginId: t.pluginId,
      typeId: t.typeId,
      defaultValue: t.defaultConfigJson,
    );
    _sourceConfigDrafts[key] = value;
    return value;
  }

  Future<void> _saveSourceConfig(SourceCatalogTypeDescriptor t) async {
    final key = _sourceTypeKey(t);
    final value = _sourceConfigDrafts[key] ?? t.defaultConfigJson;
    await ref
        .read(settingsStoreProvider)
        .setSourceConfigFor(
          pluginId: t.pluginId,
          typeId: t.typeId,
          configJson: value,
        );
    if (!mounted) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(const SnackBar(content: Text('Source config saved')));
  }

  Future<void> _reloadPluginsWithCurrentDisabled() async {
    await _ensurePluginDir();
    final bridge = ref.read(playerBridgeProvider);
    final disabledIds = ref.read(settingsStoreProvider).disabledPluginIds;
    await bridge.pluginsReloadWithDisabled(
      dir: _pluginDir!,
      disabledIds: disabledIds.toList(),
    );
    await ref
        .read(libraryBridgeProvider)
        .pluginsReloadWithDisabled(
          dir: _pluginDir!,
          disabledIds: disabledIds.toList(),
        );
  }

  Future<void> _setPluginEnabled({
    required _InstalledPlugin plugin,
    required bool enabled,
  }) async {
    final id = plugin.id?.trim();
    if (id == null || id.isEmpty) return;
    await ref
        .read(settingsStoreProvider)
        .setPluginEnabled(pluginId: id, enabled: enabled);
    await _reloadPluginsWithCurrentDisabled();
    if (mounted) {
      setState(_refreshPluginRuntimeState);
    }
  }

  Future<void> _uninstallPlugin(_InstalledPlugin plugin) async {
    await _ensurePluginDir();
    if (plugin.id != null && plugin.id!.trim().isNotEmpty) {
      await ref
          .read(playerBridgeProvider)
          .pluginsUninstallById(dir: _pluginDir!, pluginId: plugin.id!);
      await ref
          .read(settingsStoreProvider)
          .setPluginEnabled(pluginId: plugin.id!, enabled: true);
    } else {
      await Directory(plugin.dirPath).delete(recursive: true);
    }
    await _reloadPluginsWithCurrentDisabled();
    if (!mounted) return;
    setState(_refresh);
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(AppLocalizations.of(context)!.settingsPluginUninstalled),
      ),
    );
  }

  Future<void> _installPluginArtifact() async {
    final l10n = AppLocalizations.of(context)!;
    await _ensurePluginDir();
    final pluginDir = _pluginDir!;

    final picked = await FilePicker.platform.pickFiles(
      dialogTitle: l10n.settingsInstallPluginPickFolder,
      type: FileType.custom,
      allowMultiple: false,
      allowedExtensions: ['zip', _pluginLibExtForPlatform()],
    );
    final files = picked?.files;
    if (files == null || files.isEmpty) return;
    final srcPath = files.first.path?.trim();
    if (srcPath == null || srcPath.isEmpty) return;

    try {
      final bridge = ref.read(playerBridgeProvider);
      await bridge.pluginsInstallFromFile(
        dir: pluginDir,
        artifactPath: srcPath,
      );
      final library = ref.read(libraryBridgeProvider);
      final disabledIds = ref.read(settingsStoreProvider).disabledPluginIds;
      await bridge.pluginsReloadWithDisabled(
        dir: pluginDir,
        disabledIds: disabledIds.toList(),
      );
      await library.pluginsReloadWithDisabled(
        dir: pluginDir,
        disabledIds: disabledIds.toList(),
      );
      if (!mounted) return;
      setState(_refresh);
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.settingsPluginInstalled)));
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Failed to install plugin: $e')));
    }
  }

  List<Object?> _parseOutputSinkTargets(String raw) {
    dynamic decoded;
    try {
      decoded = jsonDecode(raw);
    } catch (_) {
      return const [];
    }
    if (decoded is List) {
      return decoded.cast<Object?>();
    }
    if (decoded is Map) {
      for (final key in ['targets', 'items', 'list', 'data', 'results']) {
        final v = decoded[key];
        if (v is List) {
          return v.cast<Object?>();
        }
      }
    }
    return const [];
  }

  Future<void> _loadOutputSinkTargets() async {
    final selectedKey = _selectedOutputSinkTypeKey;
    if (selectedKey == null || selectedKey.isEmpty) return;
    final parts = selectedKey.split('::');
    if (parts.length != 2) return;
    setState(() => _loadingOutputSinkTargets = true);
    try {
      final raw = await ref
          .read(playerBridgeProvider)
          .outputSinkListTargetsJson(
            pluginId: parts[0],
            typeId: parts[1],
            configJson: _outputSinkConfigController.text.trim(),
          );
      final targets = _parseOutputSinkTargets(raw);
      if (!mounted) return;
      setState(() => _outputSinkTargets = targets);
      if (targets.isNotEmpty) {
        _outputSinkTargetController.text = jsonEncode(targets.first);
      }
    } catch (_) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Failed to load output sink targets')),
      );
    } finally {
      if (mounted) {
        setState(() => _loadingOutputSinkTargets = false);
      }
    }
  }

  Future<void> _applyOutputSinkRoute() async {
    final bridge = ref.read(playerBridgeProvider);
    final settings = ref.read(settingsStoreProvider);
    if (_outputMode == _OutputMode.device) {
      await bridge.clearOutputSinkRoute();
      await settings.clearOutputSinkRoute();
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Output sink route cleared')),
      );
      return;
    }
    final selectedKey = _selectedOutputSinkTypeKey;
    if (selectedKey == null || selectedKey.isEmpty) return;
    final parts = selectedKey.split('::');
    if (parts.length != 2) return;

    final route = OutputSinkRoute(
      pluginId: parts[0],
      typeId: parts[1],
      configJson: _outputSinkConfigController.text.trim(),
      targetJson: _outputSinkTargetController.text.trim(),
    );
    await bridge.setOutputSinkRoute(route);
    await settings.setOutputSinkRoute(route);
    if (!mounted) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(const SnackBar(content: Text('Output sink route applied')));
  }

  Future<void> _clearLyricsCache() async {
    final l10n = AppLocalizations.of(context)!;
    try {
      await ref.read(lyricsControllerProvider.notifier).clearCache();
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsClearLyricsCacheDone)),
      );
    } catch (_) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsClearLyricsCacheFailed)),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final bridge = ref.read(playerBridgeProvider);
    _pluginsFuture ??= bridge.pluginsList();
    _sourceTypesFuture ??= bridge.sourceListTypes();
    if (_outputMode == _OutputMode.plugin) {
      _outputSinkTypesFuture ??= bridge.outputSinkListTypes();
    }
    _installedPluginsFuture ??= _listInstalledPlugins();

    final appBar = AppBar(
      title: Text(l10n.settingsTitle),
      actions: [
        IconButton(
          tooltip: l10n.settingsInstallPlugin,
          onPressed: _installPluginArtifact,
          icon: const Icon(Icons.add),
        ),
        IconButton(
          tooltip: l10n.refresh,
          onPressed: () => setState(_refresh),
          icon: const Icon(Icons.refresh),
        ),
      ],
    );

    final pageBody = ListView(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
      children: [
        Card(
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  l10n.settingsOutputTitle,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                DropdownButtonFormField<AudioBackend>(
                  decoration: InputDecoration(
                    labelText: l10n.settingsBackend,
                    border: const OutlineInputBorder(),
                    isDense: true,
                  ),
                  initialValue: ref
                      .watch(settingsStoreProvider)
                      .selectedBackend,
                  items: [
                    DropdownMenuItem(
                      value: AudioBackend.shared,
                      child: Text(l10n.settingsBackendShared),
                    ),
                    DropdownMenuItem(
                      value: AudioBackend.wasapiExclusive,
                      child: Text(l10n.settingsBackendWasapiExclusive),
                    ),
                    if (Platform.isWindows)
                      DropdownMenuItem(
                        value: AudioBackend.asio,
                        child: Text(l10n.settingsBackendAsioExternal),
                      ),
                  ],
                  onChanged: (v) async {
                    if (v == null) return;
                    final settings = ref.read(settingsStoreProvider);
                    await settings.setSelectedBackend(v);

                    // If the previously selected device isn't available on the new backend,
                    // fall back to Default (null) to avoid passing an invalid device name.
                    var deviceId = settings.selectedDeviceId;
                    final available = _devices
                        .where((d) => d.backend == v)
                        .map((d) => d.id)
                        .toSet();
                    if (deviceId != null &&
                        available.isNotEmpty &&
                        !available.contains(deviceId)) {
                      deviceId = null;
                      await settings.setSelectedDeviceId(null);
                    }

                    final bridge = ref.read(playerBridgeProvider);
                    await bridge.setOutputDevice(
                      backend: v,
                      deviceId: deviceId,
                    );
                    bridge.refreshDevices();
                    setState(() {});
                  },
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<String?>(
                  decoration: InputDecoration(
                    labelText: l10n.settingsDevice,
                    border: const OutlineInputBorder(),
                    isDense: true,
                  ),
                  initialValue: () {
                    final selected = ref
                        .watch(settingsStoreProvider)
                        .selectedDeviceId;
                    final backend = ref
                        .read(settingsStoreProvider)
                        .selectedBackend;
                    final available = _devices
                        .where((d) => d.backend == backend)
                        .toList();

                    final availableIds = available.map((d) => d.id).toSet();
                    if (selected != null && !availableIds.contains(selected)) {
                      return null; // Fallback to Default
                    }
                    return selected;
                  }(),
                  items: [
                    DropdownMenuItem(
                      value: null,
                      child: Text(() {
                        final backend = ref
                            .read(settingsStoreProvider)
                            .selectedBackend;
                        if (backend == AudioBackend.asio) {
                          final available =
                              _devices
                                  .where((d) => d.backend == backend)
                                  .toList()
                                ..sort((a, b) => a.name.compareTo(b.name));
                          if (available.isNotEmpty) {
                            return l10n.settingsDeviceAutoWithName(
                              available.first.name,
                            );
                          }
                        }
                        return l10n.settingsDeviceDefault;
                      }()),
                    ),
                    ..._devices
                        .where(
                          (d) =>
                              d.backend ==
                              ref.read(settingsStoreProvider).selectedBackend,
                        )
                        .map(
                          (d) => DropdownMenuItem(
                            value: d.id,
                            child: Text(d.name),
                          ),
                        ),
                  ],
                  onChanged: (v) async {
                    final settings = ref.read(settingsStoreProvider);
                    await settings.setSelectedDeviceId(v);
                    final bridge = ref.read(playerBridgeProvider);
                    await bridge.setOutputDevice(
                      backend: settings.selectedBackend,
                      deviceId: v,
                    );
                    setState(() {});
                  },
                ),
                const SizedBox(height: 12),
                Builder(
                  builder: (context) {
                    final settings = ref.watch(settingsStoreProvider);
                    final backend = settings.selectedBackend;
                    final enabled =
                        backend == AudioBackend.asio ||
                        backend == AudioBackend.wasapiExclusive;
                    if (!enabled) return const SizedBox.shrink();
                    return Column(
                      children: [
                        SwitchListTile(
                          dense: true,
                          contentPadding: EdgeInsets.zero,
                          title: Text(l10n.settingsMatchTrackSampleRate),
                          value: settings.matchTrackSampleRate,
                          onChanged: (v) async {
                            final store = ref.read(settingsStoreProvider);
                            await store.setMatchTrackSampleRate(v);
                            await ref
                                .read(playerBridgeProvider)
                                .setOutputOptions(
                                  matchTrackSampleRate: v,
                                  gaplessPlayback: store.gaplessPlayback,
                                  seekTrackFade: store.seekTrackFade,
                                );
                            setState(() {});
                          },
                        ),
                        SwitchListTile(
                          dense: true,
                          contentPadding: EdgeInsets.zero,
                          title: Text(l10n.settingsGaplessPlayback),
                          value: settings.gaplessPlayback,
                          onChanged: (v) async {
                            final store = ref.read(settingsStoreProvider);
                            await store.setGaplessPlayback(v);
                            await ref
                                .read(playerBridgeProvider)
                                .setOutputOptions(
                                  matchTrackSampleRate:
                                      store.matchTrackSampleRate,
                                  gaplessPlayback: v,
                                  seekTrackFade: store.seekTrackFade,
                                );
                            setState(() {});
                          },
                        ),
                      ],
                    );
                  },
                ),
                SwitchListTile(
                  dense: true,
                  contentPadding: EdgeInsets.zero,
                  title: Text(l10n.settingsSeekTrackFade),
                  value: ref.watch(settingsStoreProvider).seekTrackFade,
                  onChanged: (v) async {
                    final store = ref.read(settingsStoreProvider);
                    await store.setSeekTrackFade(v);
                    await ref
                        .read(playerBridgeProvider)
                        .setOutputOptions(
                          matchTrackSampleRate: store.matchTrackSampleRate,
                          gaplessPlayback: store.gaplessPlayback,
                          seekTrackFade: v,
                        );
                    setState(() {});
                  },
                ),
                const SizedBox(height: 12),
                const Divider(height: 1),
                const SizedBox(height: 12),
                Text(
                  'Plugin Output Sink',
                  style: Theme.of(context).textTheme.titleSmall,
                ),
                const SizedBox(height: 8),
                SegmentedButton<_OutputMode>(
                  segments: const [
                    ButtonSegment<_OutputMode>(
                      value: _OutputMode.device,
                      label: Text('Local Device'),
                      icon: Icon(Icons.speaker),
                    ),
                    ButtonSegment<_OutputMode>(
                      value: _OutputMode.plugin,
                      label: Text('Plugin Sink'),
                      icon: Icon(Icons.settings_ethernet),
                    ),
                  ],
                  selected: <_OutputMode>{_outputMode},
                  onSelectionChanged: (selection) {
                    if (selection.isEmpty) return;
                    setState(() => _outputMode = selection.first);
                  },
                ),
                if (_outputMode == _OutputMode.device)
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Text(
                      '当前使用本地设备输出，不会调用插件输出。',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  )
                else
                  FutureBuilder<List<OutputSinkTypeDescriptor>>(
                    future: _outputSinkTypesFuture,
                    builder: (context, snap) {
                      if (snap.connectionState != ConnectionState.done) {
                        return const LinearProgressIndicator();
                      }
                      final types = snap.data ?? const [];
                      final typeKeys = types
                          .map((t) => '${t.pluginId}::${t.typeId}')
                          .toSet();
                      if (_selectedOutputSinkTypeKey != null &&
                          !typeKeys.contains(_selectedOutputSinkTypeKey)) {
                        _selectedOutputSinkTypeKey = null;
                      }
                      OutputSinkTypeDescriptor? selectedSinkType;
                      for (final t in types) {
                        if ('${t.pluginId}::${t.typeId}' ==
                            _selectedOutputSinkTypeKey) {
                          selectedSinkType = t;
                          break;
                        }
                      }
                      return Column(
                        children: [
                          DropdownButtonFormField<String>(
                            decoration: const InputDecoration(
                              labelText: 'Output Sink Type',
                              border: OutlineInputBorder(),
                              isDense: true,
                            ),
                            initialValue: _selectedOutputSinkTypeKey,
                            items: [
                              for (final t in types)
                                DropdownMenuItem(
                                  value: '${t.pluginId}::${t.typeId}',
                                  child: Text(
                                    '${t.displayName} (${t.pluginName})',
                                  ),
                                ),
                            ],
                            onChanged: (v) => setState(() {
                              _selectedOutputSinkTypeKey = v;
                              _outputSinkTargets = const [];
                              OutputSinkTypeDescriptor? next;
                              for (final t in types) {
                                if ('${t.pluginId}::${t.typeId}' == v) {
                                  next = t;
                                  break;
                                }
                              }
                              if (next != null) {
                                _outputSinkConfigController.text =
                                    next.defaultConfigJson;
                              }
                            }),
                          ),
                          const SizedBox(height: 8),
                          if (selectedSinkType != null)
                            SchemaForm(
                              key: ValueKey(
                                'sink-config-form:${selectedSinkType.pluginId}:${selectedSinkType.typeId}',
                              ),
                              schemaJson: selectedSinkType.configSchemaJson,
                              initialValueJson:
                                  _outputSinkConfigController.text,
                              onChangedJson: (json) {
                                _outputSinkConfigController.text = json;
                              },
                            )
                          else
                            TextField(
                              controller: _outputSinkConfigController,
                              minLines: 2,
                              maxLines: 4,
                              decoration: const InputDecoration(
                                labelText: 'Config JSON',
                                border: OutlineInputBorder(),
                              ),
                            ),
                          const SizedBox(height: 8),
                          Row(
                            children: [
                              FilledButton.tonalIcon(
                                onPressed:
                                    !_loadingOutputSinkTargets &&
                                        _selectedOutputSinkTypeKey != null
                                    ? _loadOutputSinkTargets
                                    : null,
                                icon: const Icon(Icons.travel_explore),
                                label: const Text('Load Targets'),
                              ),
                              const SizedBox(width: 8),
                              Text(
                                '${_outputSinkTargets.length} targets',
                                style: Theme.of(context).textTheme.bodySmall,
                              ),
                            ],
                          ),
                          if (_outputSinkTargets.isNotEmpty) ...[
                            const SizedBox(height: 8),
                            SizedBox(
                              height: 120,
                              child: ListView.builder(
                                itemCount: _outputSinkTargets.length,
                                itemBuilder: (context, index) {
                                  final item = _outputSinkTargets[index];
                                  final text = item is String
                                      ? item
                                      : jsonEncode(item);
                                  return ListTile(
                                    dense: true,
                                    title: Text(
                                      text,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                    onTap: () {
                                      _outputSinkTargetController.text =
                                          item is String
                                          ? item
                                          : jsonEncode(item);
                                    },
                                  );
                                },
                              ),
                            ),
                          ],
                          const SizedBox(height: 8),
                          TextField(
                            controller: _outputSinkTargetController,
                            minLines: 2,
                            maxLines: 4,
                            decoration: const InputDecoration(
                              labelText: 'Target JSON',
                              border: OutlineInputBorder(),
                            ),
                          ),
                          const SizedBox(height: 8),
                          Row(
                            mainAxisAlignment: MainAxisAlignment.end,
                            children: [
                              TextButton(
                                onPressed: () {
                                  setState(() {
                                    _outputMode = _OutputMode.device;
                                  });
                                  _applyOutputSinkRoute();
                                },
                                child: const Text('Clear Route'),
                              ),
                              const SizedBox(width: 8),
                              FilledButton(
                                onPressed: _applyOutputSinkRoute,
                                child: const Text('Apply Route'),
                              ),
                            ],
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
                    final items = snap.data ?? const <_InstalledPlugin>[];
                    if (snap.connectionState != ConnectionState.done &&
                        items.isEmpty) {
                      return const LinearProgressIndicator();
                    }
                    if (items.isEmpty) {
                      return Text(l10n.settingsNoPlugins);
                    }

                    final disabled = ref
                        .read(settingsStoreProvider)
                        .disabledPluginIds;

                    return FutureBuilder<List<PluginDescriptor>>(
                      future: _pluginsFuture,
                      builder: (context, loadedSnap) {
                        final loadedData = loadedSnap.data;
                        if (loadedData != null) {
                          _cachedLoadedPluginIds = loadedData
                              .map((p) => p.id)
                              .toSet();
                          _cachedLoadedPluginIdsReady = true;
                        } else if (loadedSnap.connectionState ==
                            ConnectionState.done) {
                          _cachedLoadedPluginIds = <String>{};
                          _cachedLoadedPluginIdsReady = true;
                        }
                        final loadedKnown = _cachedLoadedPluginIdsReady;
                        final loadedIds = _cachedLoadedPluginIds;

                        return FutureBuilder<List<SourceCatalogTypeDescriptor>>(
                          future: _sourceTypesFuture,
                          builder: (context, sourceSnap) {
                            final sourceData = sourceSnap.data;
                            if (sourceData != null) {
                              _cachedSourceTypes = sourceData;
                              _cachedSourceTypesReady = true;
                            } else if (sourceSnap.connectionState ==
                                ConnectionState.done) {
                              _cachedSourceTypes = const [];
                              _cachedSourceTypesReady = true;
                            }
                            final sourceTypes =
                                sourceData ??
                                (_cachedSourceTypesReady
                                    ? _cachedSourceTypes
                                    : const <SourceCatalogTypeDescriptor>[]);
                            final sourceByPlugin =
                                <String, List<SourceCatalogTypeDescriptor>>{};
                            for (final t in sourceTypes) {
                              sourceByPlugin
                                  .putIfAbsent(
                                    t.pluginId,
                                    () => <SourceCatalogTypeDescriptor>[],
                                  )
                                  .add(t);
                            }

                            return Column(
                              children: [
                                for (final p in items)
                                  () {
                                    final pluginId = p.id;
                                    final isDisabled = pluginId != null
                                        ? disabled.contains(pluginId)
                                        : false;
                                    final isLoaded = pluginId != null
                                        ? loadedIds.contains(pluginId)
                                        : false;
                                    final pluginSourceTypes = pluginId == null
                                        ? const <SourceCatalogTypeDescriptor>[]
                                        : (sourceByPlugin[pluginId] ??
                                              const <
                                                SourceCatalogTypeDescriptor
                                              >[]);
                                    final hasCustomUi =
                                        pluginSourceTypes.isNotEmpty;
                                    final isEnabled = pluginId == null
                                        ? true
                                        : !disabled.contains(pluginId);
                                    final canUninstall = !isEnabled;

                                    final (
                                      statusText,
                                      statusIsError,
                                    ) = switch ((
                                      pluginId,
                                      isDisabled,
                                      loadedKnown,
                                      isLoaded,
                                    )) {
                                      (null, _, _, _) => ('插件 ID 缺失', true),
                                      (_, true, _, _) => ('已禁用', false),
                                      (_, false, false, _) => (
                                        '正在检查加载状态...',
                                        false,
                                      ),
                                      (_, false, true, true) => ('已加载', false),
                                      (_, false, true, false) => (
                                        '未加载（可能加载失败，请检查日志）',
                                        true,
                                      ),
                                    };
                                    final Color? pluginIconColor;
                                    if (isDisabled) {
                                      pluginIconColor = null;
                                    } else if (statusIsError) {
                                      pluginIconColor = Theme.of(
                                        context,
                                      ).colorScheme.error;
                                    } else {
                                      pluginIconColor = Colors.green.shade600;
                                    }

                                    Widget buildActions() => Row(
                                      mainAxisSize: MainAxisSize.min,
                                      children: [
                                        Switch(
                                          value: isEnabled,
                                          onChanged: pluginId == null
                                              ? null
                                              : (v) async {
                                                  try {
                                                    await _setPluginEnabled(
                                                      plugin: p,
                                                      enabled: v,
                                                    );
                                                  } catch (e) {
                                                    if (!context.mounted) {
                                                      return;
                                                    }
                                                    ScaffoldMessenger.of(
                                                      context,
                                                    ).showSnackBar(
                                                      SnackBar(
                                                        content: Text(
                                                          'Failed to reload plugins: $e',
                                                        ),
                                                      ),
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
                                                      title: Text(
                                                        l10n.settingsUninstallPlugin,
                                                      ),
                                                      content: Text(
                                                        l10n.settingsUninstallPluginConfirm(
                                                          p.nameOrDir,
                                                        ),
                                                      ),
                                                      actions: [
                                                        TextButton(
                                                          onPressed: () =>
                                                              Navigator.of(
                                                                context,
                                                              ).pop(false),
                                                          child: Text(
                                                            l10n.cancel,
                                                          ),
                                                        ),
                                                        FilledButton(
                                                          onPressed: () =>
                                                              Navigator.of(
                                                                context,
                                                              ).pop(true),
                                                          child: Text(
                                                            l10n.uninstall,
                                                          ),
                                                        ),
                                                      ],
                                                    ),
                                                  );
                                                  if (ok != true) return;
                                                  try {
                                                    await _uninstallPlugin(p);
                                                  } catch (_) {
                                                    if (!context.mounted) {
                                                      return;
                                                    }
                                                    ScaffoldMessenger.of(
                                                      context,
                                                    ).showSnackBar(
                                                      SnackBar(
                                                        content: Text(
                                                          AppLocalizations.of(
                                                            context,
                                                          )!.settingsUninstallPluginFailed,
                                                        ),
                                                      ),
                                                    );
                                                  }
                                                }
                                              : null,
                                          icon: const Icon(
                                            Icons.delete_outline,
                                          ),
                                        ),
                                      ],
                                    );

                                    final subtitle = Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        Text(p.id ?? p.dirPath),
                                        if (p.infoJson != null &&
                                            p.infoJson!.isNotEmpty)
                                          Text(
                                            p.infoJson!,
                                            maxLines: 2,
                                            overflow: TextOverflow.ellipsis,
                                          ),
                                        Text(
                                          statusText,
                                          style: Theme.of(context)
                                              .textTheme
                                              .bodySmall
                                              ?.copyWith(
                                                color: statusIsError
                                                    ? Theme.of(
                                                        context,
                                                      ).colorScheme.error
                                                    : Theme.of(context)
                                                          .colorScheme
                                                          .onSurfaceVariant,
                                              ),
                                        ),
                                      ],
                                    );

                                    if (!hasCustomUi) {
                                      return Card(
                                        margin: const EdgeInsets.only(
                                          bottom: 8,
                                        ),
                                        child: ListTile(
                                          leading: Icon(
                                            Icons.extension,
                                            color: pluginIconColor,
                                          ),
                                          title: Row(
                                            children: [
                                              Expanded(
                                                child: Text(p.nameOrDir),
                                              ),
                                              buildActions(),
                                            ],
                                          ),
                                          subtitle: subtitle,
                                        ),
                                      );
                                    }

                                    return Card(
                                      margin: const EdgeInsets.only(bottom: 8),
                                      child: ExpansionTile(
                                        leading: Icon(
                                          Icons.extension,
                                          color: pluginIconColor,
                                        ),
                                        title: Row(
                                          children: [
                                            Expanded(child: Text(p.nameOrDir)),
                                            buildActions(),
                                          ],
                                        ),
                                        subtitle: subtitle,
                                        childrenPadding:
                                            const EdgeInsets.fromLTRB(
                                              12,
                                              0,
                                              12,
                                              12,
                                            ),
                                        children: [
                                          for (final t in pluginSourceTypes)
                                            Padding(
                                              padding: const EdgeInsets.only(
                                                top: 8,
                                              ),
                                              child: Column(
                                                crossAxisAlignment:
                                                    CrossAxisAlignment.start,
                                                children: [
                                                  Text(
                                                    'Source: ${t.displayName}',
                                                    style: Theme.of(
                                                      context,
                                                    ).textTheme.titleSmall,
                                                  ),
                                                  const SizedBox(height: 6),
                                                  SchemaForm(
                                                    key: ValueKey(
                                                      'settings-source-config:${t.pluginId}:${t.typeId}',
                                                    ),
                                                    schemaJson:
                                                        t.configSchemaJson,
                                                    initialValueJson:
                                                        _sourceConfigForType(t),
                                                    onChangedJson: (json) {
                                                      _sourceConfigDrafts[_sourceTypeKey(
                                                            t,
                                                          )] =
                                                          json;
                                                    },
                                                  ),
                                                  const SizedBox(height: 6),
                                                  Align(
                                                    alignment:
                                                        Alignment.centerRight,
                                                    child: FilledButton.tonal(
                                                      onPressed: () =>
                                                          _saveSourceConfig(t),
                                                      child: Text(l10n.apply),
                                                    ),
                                                  ),
                                                ],
                                              ),
                                            ),
                                        ],
                                      ),
                                    );
                                  }(),
                              ],
                            );
                          },
                        );
                      },
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
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  l10n.settingsLyricsTitle,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                Text(
                  l10n.settingsLyricsCacheSubtitle,
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
                const SizedBox(height: 10),
                Align(
                  alignment: Alignment.centerRight,
                  child: OutlinedButton.icon(
                    onPressed: _clearLyricsCache,
                    icon: const Icon(Icons.delete_sweep_outlined),
                    label: Text(l10n.settingsClearLyricsCache),
                  ),
                ),
              ],
            ),
          ),
        ),
      ],
    );

    if (widget.useGlobalTopBar) {
      return pageBody;
    }

    return Scaffold(appBar: appBar, body: pageBody);
  }
}
