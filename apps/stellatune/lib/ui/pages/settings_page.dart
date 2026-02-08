import 'dart:io';
import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/app/settings_store.dart';
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

class SettingsPageState extends ConsumerState<SettingsPage> {
  Future<List<PluginDescriptor>>? _pluginsFuture;
  Future<List<OutputSinkTypeDescriptor>>? _outputSinkTypesFuture;
  Future<List<SourceCatalogTypeDescriptor>>? _sourceTypesFuture;
  Future<List<_InstalledPlugin>>? _installedPluginsFuture;
  String? _pluginDir;
  late final OutputSettingsUiSession _outputUiSession;

  String? _selectedOutputBackendKey;
  String? _selectedOutputSinkTypeKey;
  final TextEditingController _outputSinkConfigController =
      TextEditingController(text: '{}');
  final TextEditingController _outputSinkTargetController =
      TextEditingController(text: '{}');
  final TextEditingController _pluginRuntimeTargetIdController =
      TextEditingController();
  final TextEditingController _pluginRuntimeJsonController =
      TextEditingController(text: '{"scope":"player","command":"play"}');
  StreamSubscription<PluginRuntimeEvent>? _pluginRuntimeSub;
  Timer? _outputSinkConfigApplyDebounce;
  final List<PluginRuntimeEvent> _pluginRuntimeEvents = <PluginRuntimeEvent>[];
  List<Object?> _outputSinkTargets = const [];
  bool _loadingOutputSinkTargets = false;
  final Map<String, String> _sourceConfigDrafts = <String, String>{};
  final Map<String, String> _outputSinkConfigDrafts = <String, String>{};
  Set<String> _cachedLoadedPluginIds = <String>{};
  bool _cachedLoadedPluginIdsReady = false;
  List<OutputSinkTypeDescriptor> _cachedOutputSinkTypes = const [];
  bool _cachedOutputSinkTypesReady = false;
  List<SourceCatalogTypeDescriptor> _cachedSourceTypes = const [];
  bool _cachedSourceTypesReady = false;

  Future<void> installPluginFromTopBar() => _installPluginArtifact();

  void refreshFromTopBar() => setState(_refresh);

  @override
  void initState() {
    super.initState();
    _outputUiSession = ref.read(settingsStoreProvider).outputSettingsUiSession;
    _restoreOutputUiSessionOrSettings();
    _refresh();
    if (_parsePluginTypeKey(_selectedOutputBackendKey) != null) {
      unawaited(_loadOutputSinkTargets());
    }
    _startPluginRuntimeListener();
  }

  @override
  void dispose() {
    _persistOutputUiSession();
    _outputSinkConfigApplyDebounce?.cancel();
    _outputSinkConfigController.dispose();
    _outputSinkTargetController.dispose();
    _pluginRuntimeTargetIdController.dispose();
    _pluginRuntimeJsonController.dispose();
    unawaited(_pluginRuntimeSub?.cancel());
    super.dispose();
  }

  void _restoreOutputUiSessionOrSettings() {
    final session = _outputUiSession;
    if (!session.initialized) {
      _loadFromSettings();
      return;
    }
    _selectedOutputBackendKey = session.selectedOutputBackendKey;
    _selectedOutputSinkTypeKey = session.selectedOutputSinkTypeKey;
    _outputSinkConfigController.text = session.outputSinkConfigJson;
    _outputSinkTargetController.text = session.outputSinkTargetJson;
    _outputSinkTargets = List<Object?>.from(session.outputSinkTargets);
    _loadingOutputSinkTargets = false;
    _outputSinkConfigDrafts
      ..clear()
      ..addAll(session.outputSinkConfigDrafts);
    _cachedOutputSinkTypes = session.cachedOutputSinkTypes;
    _cachedOutputSinkTypesReady = session.cachedOutputSinkTypesReady;
  }

  void _persistOutputUiSession() {
    final session = _outputUiSession;
    session.initialized = true;
    session.selectedOutputBackendKey = _selectedOutputBackendKey;
    session.selectedOutputSinkTypeKey = _selectedOutputSinkTypeKey;
    session.outputSinkConfigJson = _outputSinkConfigController.text;
    session.outputSinkTargetJson = _outputSinkTargetController.text;
    session.outputSinkTargets = List<Object?>.from(_outputSinkTargets);
    session.loadingOutputSinkTargets = false;
    session.outputSinkConfigDrafts
      ..clear()
      ..addAll(_outputSinkConfigDrafts);
    session.cachedOutputSinkTypes = _cachedOutputSinkTypes;
    session.cachedOutputSinkTypesReady = _cachedOutputSinkTypesReady;
  }

  void _startPluginRuntimeListener() {
    unawaited(_pluginRuntimeSub?.cancel());
    _pluginRuntimeSub = ref
        .read(playerBridgeProvider)
        .pluginRuntimeEvents()
        .listen((event) {
          if (!mounted) return;
          setState(() {
            _pluginRuntimeEvents.insert(0, event);
            if (_pluginRuntimeEvents.length > 120) {
              _pluginRuntimeEvents.removeRange(
                120,
                _pluginRuntimeEvents.length,
              );
            }
          });
        });
  }

  Future<void> _sendPluginRuntimeEventJson() async {
    final payload = _pluginRuntimeJsonController.text.trim();
    if (payload.isEmpty) return;
    final pluginId = _pluginRuntimeTargetIdController.text.trim();
    try {
      await ref
          .read(playerBridgeProvider)
          .pluginPublishEventJson(
            pluginId: pluginId.isEmpty ? null : pluginId,
            eventJson: payload,
          );
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Plugin event sent')));
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Send failed: $e')));
    }
  }

  void _loadFromSettings() {
    final settings = ref.read(settingsStoreProvider);
    final route = settings.outputSinkRoute;
    _selectedOutputBackendKey = route == null
        ? _localBackendKey(settings.selectedBackend)
        : _pluginBackendKey(route.pluginId, route.typeId);
    _selectedOutputSinkTypeKey = route == null
        ? null
        : '${route.pluginId}::${route.typeId}';
    _outputSinkConfigController.text = route?.configJson ?? '{}';
    _outputSinkTargetController.text = route?.targetJson ?? '{}';
    if (route != null) {
      _outputSinkConfigDrafts['${route.pluginId}::${route.typeId}'] =
          route.configJson;
    }
    _persistOutputUiSession();
  }

  void _refresh() {
    final bridge = ref.read(playerBridgeProvider);
    _pluginsFuture = bridge.pluginsList();
    _outputSinkTypesFuture = null;
    _cachedOutputSinkTypes = const [];
    _cachedOutputSinkTypesReady = false;
    _sourceTypesFuture = bridge.sourceListTypes();
    _installedPluginsFuture = _listInstalledPlugins();
  }

  void _refreshPluginRuntimeState() {
    final bridge = ref.read(playerBridgeProvider);
    _pluginsFuture = bridge.pluginsList();
    _outputSinkTypesFuture = null;
    _cachedOutputSinkTypes = const [];
    _cachedOutputSinkTypesReady = false;
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

  String _outputSinkTypeKey(OutputSinkTypeDescriptor t) =>
      '${t.pluginId}::${t.typeId}';

  String _localBackendKey(AudioBackend backend) => 'local:${backend.name}';

  String _pluginBackendKey(String pluginId, String typeId) =>
      'plugin:$pluginId::$typeId';

  AudioBackend? _parseLocalBackendKey(String? key) {
    if (key == null || !key.startsWith('local:')) return null;
    final raw = key.substring('local:'.length);
    for (final backend in AudioBackend.values) {
      if (backend.name == raw) return backend;
    }
    return null;
  }

  String? _parsePluginTypeKey(String? key) {
    if (key == null || !key.startsWith('plugin:')) return null;
    final raw = key.substring('plugin:'.length);
    final parts = raw.split('::');
    if (parts.length != 2) return null;
    if (parts[0].trim().isEmpty || parts[1].trim().isEmpty) return null;
    return '${parts[0]}::${parts[1]}';
  }

  String _outputSinkConfigForType(OutputSinkTypeDescriptor t) {
    final key = _outputSinkTypeKey(t);
    final draft = _outputSinkConfigDrafts[key];
    if (draft != null) return draft;
    if (_selectedOutputSinkTypeKey == key) {
      final live = _outputSinkConfigController.text.trim();
      if (live.isNotEmpty) return live;
    }
    final route = ref.read(settingsStoreProvider).outputSinkRoute;
    if (route != null && key == '${route.pluginId}::${route.typeId}') {
      return route.configJson;
    }
    return t.defaultConfigJson;
  }

  String _targetValueOf(Object? target) =>
      target is String ? target : jsonEncode(target);

  String _targetLabelOf(Object? target) {
    if (target is Map) {
      final map = target.cast<Object?, Object?>();
      final name = (map['name'] ?? '').toString().trim();
      if (name.isNotEmpty) return name;
      final id = (map['id'] ?? '').toString().trim();
      if (id.isNotEmpty) return id;
    }
    final text = _targetValueOf(target);
    return text.length <= 96 ? text : '${text.substring(0, 93)}...';
  }

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
    final l10n = AppLocalizations.of(context)!;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(l10n.settingsSourceConfigSaved)));
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
    final settings = ref.read(settingsStoreProvider);
    await settings.setPluginEnabled(pluginId: id, enabled: enabled);
    await _reloadPluginsWithCurrentDisabled();
    if (!enabled) {
      final route = settings.outputSinkRoute;
      if (route != null && route.pluginId == id) {
        final bridge = ref.read(playerBridgeProvider);
        await bridge.clearOutputSinkRoute();
        await settings.clearOutputSinkRoute();
        await bridge.setOutputDevice(
          backend: settings.selectedBackend,
          deviceId: settings.selectedDeviceId,
        );
        unawaited(bridge.refreshDevices());
      }
    }
    if (mounted) {
      _loadFromSettings();
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
      final l10n = AppLocalizations.of(context)!;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsPluginInstallFailed(e.toString()))),
      );
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
    if (_loadingOutputSinkTargets) return;
    final selectedKey =
        _parsePluginTypeKey(_selectedOutputBackendKey) ??
        _selectedOutputSinkTypeKey;
    if (selectedKey == null || selectedKey.isEmpty) return;
    _selectedOutputSinkTypeKey = selectedKey;
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
        final targetValues = targets.map(_targetValueOf).toSet();
        final current = _outputSinkTargetController.text.trim();
        if (!targetValues.contains(current)) {
          _outputSinkTargetController.text = _targetValueOf(targets.first);
        }
      }
    } catch (_) {
      if (!mounted) return;
      final l10n = AppLocalizations.of(context)!;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsSinkLoadTargetsFailed)),
      );
    } finally {
      if (mounted) {
        setState(() => _loadingOutputSinkTargets = false);
      }
    }
  }

  Future<void> _applyOutputSinkRoute({bool showFeedback = false}) async {
    final bridge = ref.read(playerBridgeProvider);
    final settings = ref.read(settingsStoreProvider);
    final selectedBackendKey =
        _selectedOutputBackendKey ?? _localBackendKey(settings.selectedBackend);

    final localBackend = _parseLocalBackendKey(selectedBackendKey);
    if (localBackend != null) {
      await settings.setSelectedBackend(localBackend);
      await bridge.clearOutputSinkRoute();
      await settings.clearOutputSinkRoute();
      await bridge.setOutputDevice(
        backend: localBackend,
        deviceId: settings.selectedDeviceId,
      );
      if (showFeedback && mounted) {
        final l10n = AppLocalizations.of(context)!;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('${l10n.settingsBackend}: 已切换为系统后端')),
        );
      }
      return;
    }

    final selectedKey = _parsePluginTypeKey(selectedBackendKey);
    if (selectedKey == null || selectedKey.isEmpty) return;
    _selectedOutputSinkTypeKey = selectedKey;
    final parts = selectedKey.split('::');
    if (parts.length != 2) return;

    var targetJson = _outputSinkTargetController.text.trim();
    if (targetJson.isEmpty && _outputSinkTargets.isNotEmpty) {
      targetJson = _targetValueOf(_outputSinkTargets.first);
      _outputSinkTargetController.text = targetJson;
    }
    if (targetJson.isEmpty) {
      targetJson = '{}';
      _outputSinkTargetController.text = targetJson;
    }

    var configJson = _outputSinkConfigController.text.trim();
    if (configJson.isEmpty) {
      configJson = '{}';
      _outputSinkConfigController.text = configJson;
    }
    _outputSinkConfigDrafts[selectedKey] = configJson;

    final route = OutputSinkRoute(
      pluginId: parts[0],
      typeId: parts[1],
      configJson: configJson,
      targetJson: targetJson,
    );
    await bridge.setOutputSinkRoute(route);
    await settings.setOutputSinkRoute(route);
    if (showFeedback && mounted) {
      final l10n = AppLocalizations.of(context)!;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.settingsSinkRouteApplied)));
    }
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
    _outputSinkTypesFuture ??= bridge.outputSinkListTypes();
    _installedPluginsFuture ??= _listInstalledPlugins();

    final devices = ref.watch(audioDevicesProvider).value ?? const [];
    _persistOutputUiSession();

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
                  l10n.settingsAppearanceTitle,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<Locale?>(
                  decoration: InputDecoration(
                    labelText: l10n.settingsLanguage,
                    border: const OutlineInputBorder(),
                    isDense: true,
                  ),
                  initialValue: ref.watch(settingsStoreProvider).locale,
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
                  onChanged: (v) async {
                    await ref.read(settingsStoreProvider).setLocale(v);
                    setState(() {});
                  },
                ),
                const SizedBox(height: 12),
                DropdownButtonFormField<ThemeMode>(
                  decoration: InputDecoration(
                    labelText: l10n.settingsThemeMode,
                    border: const OutlineInputBorder(),
                    isDense: true,
                  ),
                  initialValue: ref.watch(settingsStoreProvider).themeMode,
                  items: [
                    DropdownMenuItem(
                      value: ThemeMode.system,
                      child: Text(l10n.settingsThemeSystem),
                    ),
                    DropdownMenuItem(
                      value: ThemeMode.light,
                      child: Text(l10n.settingsThemeLight),
                    ),
                    DropdownMenuItem(
                      value: ThemeMode.dark,
                      child: Text(l10n.settingsThemeDark),
                    ),
                  ],
                  onChanged: (v) async {
                    if (v == null) return;
                    await ref.read(settingsStoreProvider).setThemeMode(v);
                    setState(() {});
                  },
                ),
                if (Platform.isWindows || Platform.isLinux || Platform.isMacOS)
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: Text(l10n.settingsCloseToTray),
                    subtitle: Text(l10n.settingsCloseToTraySubtitle),
                    value: ref.watch(settingsStoreProvider).closeToTray,
                    onChanged: (v) async {
                      await ref.read(settingsStoreProvider).setCloseToTray(v);
                      setState(() {});
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
                  l10n.settingsOutputTitle,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                FutureBuilder<List<OutputSinkTypeDescriptor>>(
                  future: _outputSinkTypesFuture,
                  builder: (context, snap) {
                    if (snap.data != null) {
                      _cachedOutputSinkTypes = snap.data!;
                      _cachedOutputSinkTypesReady = true;
                    } else if (snap.connectionState == ConnectionState.done) {
                      _cachedOutputSinkTypes = const [];
                      _cachedOutputSinkTypesReady = true;
                    }
                    final sinkTypes =
                        snap.data ??
                        (_cachedOutputSinkTypesReady
                            ? _cachedOutputSinkTypes
                            : const <OutputSinkTypeDescriptor>[]);
                    final settings = ref.watch(settingsStoreProvider);
                    final route = settings.outputSinkRoute;
                    final selected =
                        _selectedOutputBackendKey ??
                        (route == null
                            ? _localBackendKey(settings.selectedBackend)
                            : _pluginBackendKey(route.pluginId, route.typeId));
                    final values = <String>{
                      _localBackendKey(AudioBackend.shared),
                      _localBackendKey(AudioBackend.wasapiExclusive),
                      ...sinkTypes.map(
                        (t) => _pluginBackendKey(t.pluginId, t.typeId),
                      ),
                    };
                    final value = values.contains(selected) ? selected : null;
                    if (value == null &&
                        selected.startsWith('plugin:') &&
                        snap.connectionState == ConnectionState.done) {
                      WidgetsBinding.instance.addPostFrameCallback((_) {
                        if (!mounted) return;
                        setState(() {
                          _selectedOutputBackendKey = _localBackendKey(
                            settings.selectedBackend,
                          );
                          _selectedOutputSinkTypeKey = null;
                          _outputSinkTargets = const [];
                        });
                      });
                    }
                    return DropdownButtonFormField<String>(
                      decoration: InputDecoration(
                        labelText: l10n.settingsBackend,
                        border: const OutlineInputBorder(),
                        isDense: true,
                      ),
                      initialValue: value,
                      items: [
                        DropdownMenuItem(
                          value: _localBackendKey(AudioBackend.shared),
                          child: Text(l10n.settingsBackendShared),
                        ),
                        DropdownMenuItem(
                          value: _localBackendKey(AudioBackend.wasapiExclusive),
                          child: Text(l10n.settingsBackendWasapiExclusive),
                        ),
                        for (final t in sinkTypes)
                          DropdownMenuItem(
                            value: _pluginBackendKey(t.pluginId, t.typeId),
                            child: Text(
                              'Plugin: ${t.displayName} (${t.pluginName})',
                            ),
                          ),
                      ],
                      onChanged: (v) async {
                        if (v == null) return;
                        final messenger = ScaffoldMessenger.of(context);
                        final local = _parseLocalBackendKey(v);
                        setState(() {
                          _selectedOutputBackendKey = v;
                          if (local != null) {
                            _selectedOutputSinkTypeKey = null;
                            _outputSinkTargets = const [];
                          } else {
                            final sinkKey = _parsePluginTypeKey(v);
                            _selectedOutputSinkTypeKey = sinkKey;
                            _outputSinkTargets = const [];
                            OutputSinkTypeDescriptor? sink;
                            for (final t in sinkTypes) {
                              if (_outputSinkTypeKey(t) == sinkKey) {
                                sink = t;
                                break;
                              }
                            }
                            if (sink != null) {
                              _outputSinkConfigController.text =
                                  _outputSinkConfigForType(sink);
                            } else {
                              _outputSinkConfigController.text = '{}';
                            }
                            final active = settings.outputSinkRoute;
                            if (active != null &&
                                sinkKey ==
                                    '${active.pluginId}::${active.typeId}') {
                              _outputSinkTargetController.text =
                                  active.targetJson;
                            } else {
                              _outputSinkTargetController.text = '{}';
                            }
                          }
                        });
                        try {
                          if (local == null) {
                            await _loadOutputSinkTargets();
                          }
                          await _applyOutputSinkRoute();
                        } catch (e) {
                          messenger.showSnackBar(
                            SnackBar(content: Text('Apply backend failed: $e')),
                          );
                        } finally {
                          try {
                            await ref
                                .read(playerBridgeProvider)
                                .refreshDevices();
                          } catch (_) {}
                        }
                      },
                    );
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
                    final selectedBackendKey =
                        _selectedOutputBackendKey ??
                        _localBackendKey(
                          ref.read(settingsStoreProvider).selectedBackend,
                        );
                    final localBackend = _parseLocalBackendKey(
                      selectedBackendKey,
                    );
                    if (localBackend != null) {
                      final selected = ref
                          .watch(settingsStoreProvider)
                          .selectedDeviceId;
                      final available = devices
                          .where((d) => d.backend == localBackend)
                          .toList();
                      final availableIds = available.map((d) => d.id).toSet();
                      if (selected != null &&
                          !availableIds.contains(selected)) {
                        return null;
                      }
                      return selected;
                    }
                    final targetValue = _outputSinkTargetController.text.trim();
                    final targetValues = _outputSinkTargets
                        .map(_targetValueOf)
                        .toSet();
                    return targetValues.contains(targetValue)
                        ? targetValue
                        : null;
                  }(),
                  items: () {
                    final selectedBackendKey =
                        _selectedOutputBackendKey ??
                        _localBackendKey(
                          ref.read(settingsStoreProvider).selectedBackend,
                        );
                    final localBackend = _parseLocalBackendKey(
                      selectedBackendKey,
                    );
                    if (localBackend != null) {
                      return <DropdownMenuItem<String?>>[
                        DropdownMenuItem(
                          value: null,
                          child: Text(l10n.settingsDeviceDefault),
                        ),
                        ...devices
                            .where((d) => d.backend == localBackend)
                            .map(
                              (d) => DropdownMenuItem(
                                value: d.id,
                                child: Text(d.name),
                              ),
                            ),
                      ];
                    }
                    return <DropdownMenuItem<String?>>[
                      for (final item in _outputSinkTargets)
                        DropdownMenuItem(
                          value: _targetValueOf(item),
                          child: Text(_targetLabelOf(item)),
                        ),
                    ];
                  }(),
                  onChanged: (v) async {
                    final messenger = ScaffoldMessenger.of(context);
                    final localBackend = _parseLocalBackendKey(
                      _selectedOutputBackendKey ??
                          _localBackendKey(
                            ref.read(settingsStoreProvider).selectedBackend,
                          ),
                    );
                    if (localBackend == null) {
                      setState(() {
                        _outputSinkTargetController.text = v ?? '{}';
                      });
                      try {
                        await _applyOutputSinkRoute();
                      } catch (e) {
                        messenger.showSnackBar(
                          SnackBar(content: Text('Apply backend failed: $e')),
                        );
                      }
                      return;
                    }
                    final settings = ref.read(settingsStoreProvider);
                    await settings.setSelectedDeviceId(v);
                    final bridge = ref.read(playerBridgeProvider);
                    await bridge.setOutputDevice(
                      backend: localBackend,
                      deviceId: v,
                    );
                    setState(() {});
                  },
                ),
                const SizedBox(height: 12),
                Builder(
                  builder: (context) {
                    final settings = ref.watch(settingsStoreProvider);
                    final backend = _parseLocalBackendKey(
                      _selectedOutputBackendKey ??
                          _localBackendKey(settings.selectedBackend),
                    );
                    final enabled = backend == AudioBackend.wasapiExclusive;
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
                            final outputTypes = _cachedOutputSinkTypesReady
                                ? _cachedOutputSinkTypes
                                : const <OutputSinkTypeDescriptor>[];
                            final outputByPlugin =
                                <String, List<OutputSinkTypeDescriptor>>{};
                            for (final t in outputTypes) {
                              outputByPlugin
                                  .putIfAbsent(
                                    t.pluginId,
                                    () => <OutputSinkTypeDescriptor>[],
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
                                    final pluginOutputSinkTypes =
                                        pluginId == null
                                        ? const <OutputSinkTypeDescriptor>[]
                                        : (outputByPlugin[pluginId] ??
                                              const <
                                                OutputSinkTypeDescriptor
                                              >[]);
                                    final hasCustomUi =
                                        pluginSourceTypes.isNotEmpty ||
                                        pluginOutputSinkTypes.isNotEmpty;
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
                                          for (final t in pluginOutputSinkTypes)
                                            Padding(
                                              padding: const EdgeInsets.only(
                                                top: 8,
                                              ),
                                              child: Column(
                                                crossAxisAlignment:
                                                    CrossAxisAlignment.start,
                                                children: [
                                                  Text(
                                                    'Output: ${t.displayName}',
                                                    style: Theme.of(
                                                      context,
                                                    ).textTheme.titleSmall,
                                                  ),
                                                  const SizedBox(height: 6),
                                                  SchemaForm(
                                                    key: ValueKey(
                                                      'settings-output-config:${t.pluginId}:${t.typeId}',
                                                    ),
                                                    schemaJson:
                                                        t.configSchemaJson,
                                                    initialValueJson:
                                                        _outputSinkConfigForType(
                                                          t,
                                                        ),
                                                    onChangedJson: (json) {
                                                      final key =
                                                          _outputSinkTypeKey(t);
                                                      _outputSinkConfigDrafts[key] =
                                                          json;
                                                      if (_selectedOutputSinkTypeKey ==
                                                          key) {
                                                        _outputSinkConfigController
                                                                .text =
                                                            json;
                                                        _outputSinkConfigApplyDebounce
                                                            ?.cancel();
                                                        _outputSinkConfigApplyDebounce = Timer(
                                                          const Duration(
                                                            milliseconds: 350,
                                                          ),
                                                          () async {
                                                            if (!mounted) {
                                                              return;
                                                            }
                                                            try {
                                                              await _loadOutputSinkTargets();
                                                              await _applyOutputSinkRoute();
                                                            } catch (_) {}
                                                          },
                                                        );
                                                      }
                                                    },
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
        const SizedBox(height: 12),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Plugin Runtime Debug',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _pluginRuntimeTargetIdController,
                  decoration: const InputDecoration(
                    labelText: 'Target plugin id (optional)',
                    hintText: 'empty = broadcast',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _pluginRuntimeJsonController,
                  minLines: 3,
                  maxLines: 6,
                  decoration: const InputDecoration(
                    labelText: 'Event JSON',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 8),
                Row(
                  children: [
                    FilledButton.icon(
                      onPressed: _sendPluginRuntimeEventJson,
                      icon: const Icon(Icons.send),
                      label: const Text('Send'),
                    ),
                    const SizedBox(width: 8),
                    OutlinedButton(
                      onPressed: () {
                        setState(() => _pluginRuntimeEvents.clear());
                      },
                      child: const Text('Clear Events'),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                Container(
                  width: double.infinity,
                  height: 220,
                  decoration: BoxDecoration(
                    border: Border.all(
                      color: Theme.of(
                        context,
                      ).dividerColor.withValues(alpha: 0.5),
                    ),
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: _pluginRuntimeEvents.isEmpty
                      ? const Center(child: Text('No runtime events yet'))
                      : ListView.separated(
                          padding: const EdgeInsets.all(8),
                          itemBuilder: (context, index) {
                            final e = _pluginRuntimeEvents[index];
                            return SelectableText(
                              '[${e.kind.name}] ${e.pluginId}: ${e.payloadJson}',
                              style: Theme.of(context).textTheme.bodySmall,
                            );
                          },
                          separatorBuilder: (_, _) => const SizedBox(height: 6),
                          itemCount: _pluginRuntimeEvents.length,
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
