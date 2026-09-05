import 'dart:io';
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/bridge/api/player.dart' as player_api;
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/lyrics/lyrics_controller.dart';
import 'package:stellatune/player/decoder_extension_support.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/ui/pages/settings/logic/settings_plugin_runtime_service.dart';
import 'package:stellatune/ui/pages/settings/models/installed_plugin.dart';
import 'package:stellatune/ui/pages/settings/settings_value_utils.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_appearance_section.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_lyrics_cache_section.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_plugins_list.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_section_card.dart';

class _PluginRuntimeSnapshot {
  const _PluginRuntimeSnapshot({
    required this.installedPlugins,
    required this.disabledPluginIds,
    required this.loadedPluginIds,
    required this.sourceTypes,
  });

  final List<InstalledPlugin> installedPlugins;
  final Set<String> disabledPluginIds;
  final Set<String> loadedPluginIds;
  final List<SourceCatalogTypeDescriptor> sourceTypes;
}

class SettingsPage extends ConsumerStatefulWidget {
  const SettingsPage({super.key, this.useGlobalTopBar = false});

  final bool useGlobalTopBar;

  @override
  ConsumerState<SettingsPage> createState() => SettingsPageState();
}

class SettingsPageState extends ConsumerState<SettingsPage> {
  Future<List<PluginDescriptor>>? _pluginsFuture;
  Future<List<OutputSinkTypeDescriptor>>? _outputSinkTypesFuture;
  Future<List<SourceCatalogTypeDescriptor>>? _sourceTypesFuture;
  Future<List<InstalledPlugin>>? _installedPluginsFuture;
  Future<Set<String>>? _disabledPluginIdsFuture;
  Future<_PluginRuntimeSnapshot>? _pluginRuntimeSnapshotFuture;
  Future<String>? _pluginDirFuture;
  String? _pluginDir;
  late final OutputSettingsUiSession _outputUiSession;
  final SettingsPluginRuntimeService _pluginRuntimeService =
      const SettingsPluginRuntimeService();

  String? _selectedOutputBackendKey;
  String? _selectedOutputSinkTypeKey;
  final TextEditingController _outputSinkConfigController =
      TextEditingController(text: '{}');
  final TextEditingController _outputSinkTargetController =
      TextEditingController(text: '{}');
  Timer? _outputSinkConfigApplyDebounce;
  List<Object?> _outputSinkTargets = const [];
  bool _loadingOutputSinkTargets = false;
  final Map<String, String> _outputSinkConfigDrafts = <String, String>{};
  Set<String> _cachedLoadedPluginIds = <String>{};
  bool _cachedLoadedPluginIdsReady = false;
  Set<String> _cachedDisabledPluginIds = <String>{};
  bool _cachedDisabledPluginIdsReady = false;
  List<InstalledPlugin> _cachedInstalledPlugins = const [];
  bool _cachedInstalledPluginsReady = false;
  List<OutputSinkTypeDescriptor> _cachedOutputSinkTypes = const [];
  bool _cachedOutputSinkTypesReady = false;
  List<SourceCatalogTypeDescriptor> _cachedSourceTypes = const [];
  bool _cachedSourceTypesReady = false;
  ResampleQuality _resampleQuality = ResampleQuality.high;
  bool _applyingPlaybackLatency = false;
  int _playbackLatencyRevision = 0;

  @override
  void initState() {
    super.initState();
    _outputUiSession = ref.read(settingsUiSessionProvider);
    _restoreOutputUiSessionOrSettings();
    _refresh();
  }

  @override
  void dispose() {
    _persistOutputUiSession();
    _outputSinkConfigApplyDebounce?.cancel();
    _outputSinkConfigController.dispose();
    _outputSinkTargetController.dispose();
    super.dispose();
  }

  void _updateUi(VoidCallback updater) => setState(updater);

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
    _resampleQuality = session.resampleQuality;
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
    session.resampleQuality = _resampleQuality;
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
    _resampleQuality = settings.resampleQuality;
    _persistOutputUiSession();
  }

  void _refresh() {
    final bridge = ref.read(playerBridgeProvider);
    final library = ref.read(libraryBridgeProvider);
    _pluginsFuture = _pluginRuntimeService.listLoadedPlugins(bridge);
    _outputSinkTypesFuture = null;
    _cachedOutputSinkTypes = const [];
    _cachedOutputSinkTypesReady = false;
    _sourceTypesFuture = _pluginRuntimeService.listSourceTypes(bridge);
    _installedPluginsFuture = _listInstalledPlugins();
    _disabledPluginIdsFuture = _pluginRuntimeService.listDisabledPluginIds(
      library,
    );
    _pluginRuntimeSnapshotFuture = null;
  }

  void _refreshPluginRuntimeState() {
    final bridge = ref.read(playerBridgeProvider);
    final library = ref.read(libraryBridgeProvider);
    _pluginsFuture = _pluginRuntimeService.listLoadedPlugins(bridge);
    _outputSinkTypesFuture = null;
    _cachedOutputSinkTypes = const [];
    _cachedOutputSinkTypesReady = false;
    _sourceTypesFuture = _pluginRuntimeService.listSourceTypes(bridge);
    _disabledPluginIdsFuture = _pluginRuntimeService.listDisabledPluginIds(
      library,
    );
    _pluginRuntimeSnapshotFuture = null;
  }

  Future<void> _ensurePluginDir() async {
    _pluginDir ??= await (_pluginDirFuture ??= defaultPluginDir());
  }

  Future<List<InstalledPlugin>> _listInstalledPlugins() async {
    await _ensurePluginDir();
    return _pluginRuntimeService.listInstalledPlugins(
      bridge: ref.read(playerBridgeProvider),
      pluginDir: _pluginDir!,
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final bridge = ref.read(playerBridgeProvider);
    _pluginsFuture ??= _pluginRuntimeService.listLoadedPlugins(bridge);
    _sourceTypesFuture ??= _pluginRuntimeService.listSourceTypes(bridge);
    _outputSinkTypesFuture ??= bridge.outputSinkListTypes();
    _installedPluginsFuture ??= _listInstalledPlugins();
    _pluginDirFuture ??= defaultPluginDir();
    _pluginRuntimeSnapshotFuture ??= _createPluginRuntimeSnapshotFuture();

    final devices = ref.watch(audioDevicesProvider).value ?? const [];
    _persistOutputUiSession();

    final appBar = AppBar(title: Text(l10n.settingsTitle));

    final pageBody = ListView(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
      children: [
        _buildAppearanceCard(l10n),
        const SizedBox(height: 12),
        _buildOutputCard(l10n, devices),
        const SizedBox(height: 12),
        _buildPluginsCard(l10n),
        const SizedBox(height: 12),
        _buildLyricsCacheCard(l10n),
      ],
    );

    if (widget.useGlobalTopBar) {
      return pageBody;
    }

    return Scaffold(appBar: appBar, body: pageBody);
  }
}

extension _SettingsHelpers on SettingsPageState {
  String _outputSinkTypeKey(OutputSinkTypeDescriptor t) =>
      SettingsValueUtils.outputSinkTypeKey(t);

  String _localBackendKey(AudioBackend backend) =>
      SettingsValueUtils.localBackendKey(backend);

  String _pluginBackendKey(String pluginId, String typeId) =>
      SettingsValueUtils.pluginBackendKey(pluginId, typeId);

  AudioBackend? _parseLocalBackendKey(String? key) =>
      SettingsValueUtils.parseLocalBackendKey(key);

  List<AudioBackend> _availableLocalBackends() =>
      SettingsValueUtils.availableLocalBackends();

  String _localBackendLabel(AppLocalizations l10n, AudioBackend backend) {
    return switch (backend) {
      AudioBackend.shared =>
        Platform.isWindows
            ? l10n.settingsBackendShared
            : l10n.settingsBackendSharedGeneric,
      AudioBackend.wasapiExclusive => l10n.settingsBackendWasapiExclusive,
    };
  }

  String? _parsePluginTypeKey(String? key) =>
      SettingsValueUtils.parsePluginTypeKey(key);

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
      SettingsValueUtils.targetValueOf(target);

  String _targetLabelOf(Object? target) =>
      SettingsValueUtils.targetLabelOf(target);

  String _targetDebugSummary(Object? target) =>
      SettingsValueUtils.targetDebugSummary(target);

  bool _jsonTextsEquivalent(String left, String right) =>
      SettingsValueUtils.jsonTextsEquivalent(left, right);

  void _logOutputSinkTargets(
    String stage, {
    required String pluginId,
    required String typeId,
    required List<Object?> targets,
    String? selectedTargetJson,
  }) {
    final preview = targets.take(6).map(_targetDebugSummary).join(' || ');
    final selectedSummary = selectedTargetJson == null
        ? '<null>'
        : _targetDebugSummary(selectedTargetJson);
    logger.i(
      'output sink targets stage=$stage plugin=$pluginId type=$typeId '
      'count=${targets.length} selected=$selectedSummary preview=[$preview]',
    );
  }

  Future<void> _openPluginWebUi({
    required String pluginId,
    required String pluginName,
  }) async {
    await _ensurePluginDir();
    final pluginsDir = (_pluginDir ?? '').trim();
    if (!mounted || pluginsDir.isEmpty) {
      return;
    }
    try {
      final url = await player_api.pluginOpenUi(pluginId: pluginId);
      final raw = url.trim();
      if (raw.isEmpty) {
        throw StateError('plugin does not expose a web ui entry');
      }
      final uri = Uri.tryParse(raw);
      if (uri == null) {
        throw StateError('invalid plugin ui url: $raw');
      }

      var opened = await launchUrl(uri, mode: LaunchMode.externalApplication);
      if (!opened) {
        opened = await launchUrl(uri);
      }
      if (!opened) {
        throw StateError('failed to launch browser for plugin web ui');
      }
    } catch (e, s) {
      logger.w(
        'failed to open plugin web ui in browser plugin=$pluginId',
        error: e,
        stackTrace: s,
      );
      if (!mounted) {
        return;
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Failed to open "$pluginName" Web UI: $e')),
      );
    }
  }
}

extension _SettingsRuntimeOps on SettingsPageState {
  Future<void> _applyOutputOptions({
    bool? matchTrackSampleRate,
    bool? gaplessPlayback,
    bool? seekTrackFade,
    ResampleQuality? resampleQuality,
  }) async {
    final settings = ref.read(settingsStoreProvider);
    await ref
        .read(playerBridgeProvider)
        .setOutputOptions(
          matchTrackSampleRate:
              matchTrackSampleRate ?? settings.matchTrackSampleRate,
          gaplessPlayback: gaplessPlayback ?? settings.gaplessPlayback,
          seekTrackFade: seekTrackFade ?? settings.seekTrackFade,
          resampleQuality: resampleQuality ?? settings.resampleQuality,
        );
  }

  Future<void> _setPluginEnabled({
    required InstalledPlugin plugin,
    required bool enabled,
  }) async {
    final id = plugin.id?.trim();
    if (id == null || id.isEmpty) return;
    final library = ref.read(libraryBridgeProvider);
    if (enabled) {
      await library.pluginEnable(pluginId: id);
    } else {
      await library.pluginDisable(pluginId: id);
    }
    await library.pluginApplyState();
    await _refreshDecoderExtensionSupportCache();
    if (!enabled) {
      await ref
          .read(playbackControllerProvider.notifier)
          .removeUnplayableQueuedItemsDueToDisabledPlugins(pluginId: id);
    }
    if (mounted) {
      _loadFromSettings();
      _updateUi(_refreshPluginRuntimeState);
    }
  }

  Future<void> _uninstallPlugin(InstalledPlugin plugin) async {
    await _ensurePluginDir();
    final pluginId = plugin.id?.trim();
    if (pluginId != null && pluginId.isNotEmpty) {
      await ref
          .read(playerBridgeProvider)
          .pluginsUninstallById(dir: _pluginDir!, pluginId: pluginId);
    } else {
      await Directory(plugin.dirPath).delete(recursive: true);
      await ref.read(libraryBridgeProvider).pluginApplyState();
    }
    await _refreshDecoderExtensionSupportCache();
    if (!mounted) return;
    _updateUi(_refresh);
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

    PlatformFile? picked;
    try {
      picked = await FilePicker.pickFile(
        dialogTitle: l10n.settingsInstallPluginPickFolder,
        type: FileType.custom,
        allowedExtensions: ['zip'],
        windowsOptions: const WindowsOptions(lockParentWindow: true),
        linuxOptions: const LinuxOptions(lockParentWindow: true),
      );
    } catch (e, s) {
      logger.e(
        'failed to open plugin artifact picker',
        error: e,
        stackTrace: s,
      );
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsPluginInstallFailed(e.toString()))),
      );
      return;
    }
    if (picked == null) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('No plugin file selected.')));
      return;
    }
    final srcPath = picked.path?.trim();
    if (srcPath == null || srcPath.isEmpty) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Selected file path is empty.')),
      );
      return;
    }

    try {
      final bridge = ref.read(playerBridgeProvider);
      await bridge.pluginsInstallFromFile(
        dir: pluginDir,
        artifactPath: srcPath,
      );
      await _refreshDecoderExtensionSupportCache();
      if (!mounted) return;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.settingsPluginInstalled)));
    } catch (e, s) {
      logger.e('failed to install plugin', error: e, stackTrace: s);
      try {
        final status = await ref
            .read(libraryBridgeProvider)
            .pluginApplyStateStatusJson();
        logger.w('plugin apply-state status after install failure: $status');
      } catch (statusError, statusStack) {
        logger.w(
          'failed to query plugin apply-state status after install failure',
          error: statusError,
          stackTrace: statusStack,
        );
      }
      if (!mounted) return;
      final l10n = AppLocalizations.of(context)!;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsPluginInstallFailed(e.toString()))),
      );
    } finally {
      if (mounted) {
        _updateUi(_refresh);
      }
    }
  }

  Future<void> _refreshDecoderExtensionSupportCache() async {
    DecoderExtensionSupportCache.instance.invalidate();
    try {
      await DecoderExtensionSupportCache.instance.refresh(
        ref.read(playerBridgeProvider),
      );
    } catch (e, s) {
      logger.w(
        'failed to refresh decoder extension cache after plugin apply state',
        error: e,
        stackTrace: s,
      );
    }
  }

  Future<void> _loadOutputSinkTargets({
    bool selectFirst = false,
    bool showErrorFeedback = true,
  }) async {
    if (_loadingOutputSinkTargets) return;
    final selectedKey =
        _parsePluginTypeKey(_selectedOutputBackendKey) ??
        _selectedOutputSinkTypeKey;
    if (selectedKey == null || selectedKey.isEmpty) return;
    _selectedOutputSinkTypeKey = selectedKey;
    final parts = selectedKey.split('::');
    if (parts.length != 2) return;
    _updateUi(() => _loadingOutputSinkTargets = true);
    try {
      final raw = await ref
          .read(playerBridgeProvider)
          .outputSinkListTargetsJson(
            pluginId: parts[0],
            typeId: parts[1],
            configJson: _outputSinkConfigController.text.trim(),
          );
      final targets = SettingsValueUtils.parseOutputSinkTargetsJson(raw);
      _logOutputSinkTargets(
        'load',
        pluginId: parts[0],
        typeId: parts[1],
        targets: targets,
        selectedTargetJson: _outputSinkTargetController.text.trim(),
      );
      if (!mounted) return;
      _updateUi(() => _outputSinkTargets = targets);
      if (targets.isNotEmpty) {
        final targetValues = targets.map(_targetValueOf).toSet();
        final current = _outputSinkTargetController.text.trim();
        if (selectFirst || !targetValues.contains(current)) {
          _outputSinkTargetController.text = _targetValueOf(targets.first);
        }
      } else {
        _outputSinkTargetController.text = '';
      }
    } catch (e, s) {
      logger.e('failed to load output sink targets', error: e, stackTrace: s);
      if (!showErrorFeedback || !mounted) return;
      final l10n = AppLocalizations.of(context)!;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsSinkLoadTargetsFailed)),
      );
    } finally {
      if (mounted) {
        _updateUi(() => _loadingOutputSinkTargets = false);
      }
    }
  }

  Future<void> _refreshSelectedOutputDevices({bool selectFirst = false}) async {
    final selectedBackendKey = _currentSelectedBackendKey();
    final localBackend = _parseLocalBackendKey(selectedBackendKey);
    if (localBackend != null) {
      final devices = await ref.refresh(audioDevicesProvider.future);
      final backendDevices = devices
          .where((d) => d.backend == localBackend)
          .toList();
      final currentDeviceId = ref.read(settingsStoreProvider).selectedDeviceId;
      String? nextDeviceId;
      if (backendDevices.isEmpty) {
        nextDeviceId = null;
      } else if (selectFirst) {
        nextDeviceId = null;
      } else if (currentDeviceId == null) {
        nextDeviceId = null;
      } else if (!backendDevices.any((d) => d.id == currentDeviceId)) {
        nextDeviceId = null;
      } else {
        nextDeviceId = currentDeviceId;
      }
      await ref
          .read(settingsStoreProvider.notifier)
          .setSelectedDeviceId(nextDeviceId);
      return;
    }
    await _loadOutputSinkTargets(selectFirst: selectFirst);
  }

  Future<void> _applyOutputSinkRoute({bool showFeedback = false}) async {
    final bridge = ref.read(playerBridgeProvider);
    final settings = ref.read(settingsStoreProvider);
    final selectedBackendKey =
        _selectedOutputBackendKey ?? _localBackendKey(settings.selectedBackend);

    final localBackend = _parseLocalBackendKey(selectedBackendKey);
    if (localBackend != null) {
      await _applyLocalBackendRoute(
        bridge: bridge,
        settings: settings,
        localBackend: localBackend,
        showFeedback: showFeedback,
      );
      return;
    }

    final pluginRouteSelection = _buildPluginRouteSelection(selectedBackendKey);
    if (pluginRouteSelection == null) return;
    final selectedKey = pluginRouteSelection.selectedKey;
    final route = pluginRouteSelection.route;
    if (_isUnchangedOutputSinkRoute(settings.outputSinkRoute, route)) {
      logger.d(
        'skip output sink route apply: no config/target changes '
        'plugin=${route.pluginId} type=${route.typeId}',
      );
      return;
    }

    _outputSinkConfigDrafts[selectedKey] = route.configJson;
    logger.i(
      'apply output sink route start plugin=${route.pluginId} type=${route.typeId} '
      'target=${_targetDebugSummary(route.targetJson)} config_len=${route.configJson.length}',
    );
    await bridge.setOutputSinkRoute(route);
    await ref.read(settingsStoreProvider.notifier).setOutputSinkRoute(route);
    logger.i(
      'apply output sink route success plugin=${route.pluginId} type=${route.typeId} '
      'target=${_targetDebugSummary(route.targetJson)}',
    );
    if (!showFeedback || !mounted) return;
    final l10n = AppLocalizations.of(context)!;
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(l10n.settingsSinkRouteApplied)));
  }

  Future<void> _applyLocalBackendRoute({
    required PlayerBridge bridge,
    required SettingsState settings,
    required AudioBackend localBackend,
    required bool showFeedback,
  }) async {
    await ref
        .read(settingsStoreProvider.notifier)
        .setSelectedBackend(localBackend);
    await bridge.clearOutputSinkRoute();
    await ref.read(settingsStoreProvider.notifier).clearOutputSinkRoute();
    await bridge.setOutputDevice(
      backend: localBackend,
      deviceId: settings.selectedDeviceId,
    );
    if (!showFeedback || !mounted) return;
    final l10n = AppLocalizations.of(context)!;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text('${l10n.settingsBackend}: 已切换为系统后端')),
    );
  }

  ({String selectedKey, OutputSinkRoute route})? _buildPluginRouteSelection(
    String selectedBackendKey,
  ) {
    final selectedKey = _parsePluginTypeKey(selectedBackendKey);
    if (selectedKey == null || selectedKey.isEmpty) return null;
    _selectedOutputSinkTypeKey = selectedKey;
    final parts = selectedKey.split('::');
    if (parts.length != 2) return null;
    final targetJson = _normalizedOutputSinkTargetJson();
    if (targetJson == null) return null;

    final route = OutputSinkRoute(
      pluginId: parts[0],
      typeId: parts[1],
      configJson: _normalizedOutputSinkConfigJson(),
      targetJson: targetJson,
    );
    return (selectedKey: selectedKey, route: route);
  }

  String? _normalizedOutputSinkTargetJson() {
    var targetJson = _outputSinkTargetController.text.trim();
    if (targetJson.isEmpty && _outputSinkTargets.isNotEmpty) {
      targetJson = _targetValueOf(_outputSinkTargets.first);
      _outputSinkTargetController.text = targetJson;
    }
    if (targetJson.isEmpty || targetJson == '{}') {
      _outputSinkTargetController.text = '';
      return null;
    }
    return targetJson;
  }

  String _normalizedOutputSinkConfigJson() {
    var configJson = _outputSinkConfigController.text.trim();
    if (configJson.isEmpty) {
      configJson = '{}';
      _outputSinkConfigController.text = configJson;
    }
    return configJson;
  }

  bool _isUnchangedOutputSinkRoute(
    OutputSinkRoute? currentRoute,
    OutputSinkRoute nextRoute,
  ) {
    return currentRoute != null &&
        currentRoute.pluginId == nextRoute.pluginId &&
        currentRoute.typeId == nextRoute.typeId &&
        _jsonTextsEquivalent(currentRoute.configJson, nextRoute.configJson) &&
        _jsonTextsEquivalent(currentRoute.targetJson, nextRoute.targetJson);
  }

  Future<void> _clearLyricsCache() async {
    final l10n = AppLocalizations.of(context)!;
    try {
      await ref.read(lyricsControllerProvider.notifier).clearCache();
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsClearLyricsCacheDone)),
      );
    } catch (e, s) {
      logger.e('failed to clear lyrics cache', error: e, stackTrace: s);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsClearLyricsCacheFailed)),
      );
    }
  }
}

extension _SettingsBuildSections on SettingsPageState {
  Widget _buildAppearanceCard(AppLocalizations l10n) {
    final settings = ref.watch(settingsStoreProvider);
    return SettingsAppearanceSection(
      l10n: l10n,
      locale: settings.locale,
      themeMode: settings.themeMode,
      closeToTray: settings.closeToTray,
      onLocaleChanged: (locale) async {
        await ref.read(settingsStoreProvider.notifier).setLocale(locale);
        _updateUi(() {});
      },
      onThemeModeChanged: (mode) async {
        await ref.read(settingsStoreProvider.notifier).setThemeMode(mode);
        _updateUi(() {});
      },
      onCloseToTrayChanged: (enabled) async {
        await ref.read(settingsStoreProvider.notifier).setCloseToTray(enabled);
        _updateUi(() {});
      },
    );
  }

  Widget _buildOutputCard(AppLocalizations l10n, List<AudioDevice> devices) {
    return SettingsSectionCard(
      title: l10n.settingsOutputTitle,
      headerBottomSpacing: 8,
      children: [
        _buildOutputBackendField(l10n),
        const SizedBox(height: 12),
        _buildOutputDeviceField(l10n: l10n, devices: devices),
        const SizedBox(height: 12),
        _buildWasapiExclusiveOptions(l10n),
        _buildSeekTrackFadeOption(l10n),
        _buildPlaybackLatencyField(l10n),
        _buildResampleQualityField(l10n),
      ],
    );
  }

  Widget _buildOutputBackendField(AppLocalizations l10n) {
    return FutureBuilder<List<OutputSinkTypeDescriptor>>(
      future: _outputSinkTypesFuture,
      builder: (context, snap) {
        _updateOutputSinkTypeCache(snap);
        final sinkTypes = _resolvedOutputSinkTypes(snap);
        final settings = ref.watch(settingsStoreProvider);
        final selected = _selectedBackendKeyFromSettings(settings);
        final value = _validatedSelectedBackend(
          selected: selected,
          sinkTypes: sinkTypes,
        );
        if (value == null) {
          _resetInvalidPluginBackendSelectionIfNeeded(
            selected: selected,
            settings: settings,
            connectionState: snap.connectionState,
          );
        }

        return DropdownButtonFormField<String>(
          decoration: InputDecoration(
            labelText: l10n.settingsBackend,
            border: const OutlineInputBorder(),
            isDense: true,
          ),
          initialValue: value,
          items: [
            for (final backend in _availableLocalBackends())
              DropdownMenuItem(
                value: _localBackendKey(backend),
                child: Text(_localBackendLabel(l10n, backend)),
              ),
            for (final t in sinkTypes)
              DropdownMenuItem(
                value: _pluginBackendKey(t.pluginId, t.typeId),
                child: Text('Plugin: ${t.displayName} (${t.pluginName})'),
              ),
          ],
          onChanged: (v) => _handleOutputBackendChanged(
            v: v,
            sinkTypes: sinkTypes,
            settings: settings,
            context: context,
          ),
        );
      },
    );
  }

  void _updateOutputSinkTypeCache(
    AsyncSnapshot<List<OutputSinkTypeDescriptor>> snap,
  ) {
    if (snap.data != null) {
      _cachedOutputSinkTypes = snap.data!;
      _cachedOutputSinkTypesReady = true;
      return;
    }
    if (snap.connectionState == ConnectionState.done) {
      _cachedOutputSinkTypes = const [];
      _cachedOutputSinkTypesReady = true;
    }
  }

  List<OutputSinkTypeDescriptor> _resolvedOutputSinkTypes(
    AsyncSnapshot<List<OutputSinkTypeDescriptor>> snap,
  ) {
    return snap.data ??
        (_cachedOutputSinkTypesReady
            ? _cachedOutputSinkTypes
            : const <OutputSinkTypeDescriptor>[]);
  }

  String _selectedBackendKeyFromSettings(SettingsState settings) {
    final route = settings.outputSinkRoute;
    return _selectedOutputBackendKey ??
        (route == null
            ? _localBackendKey(settings.selectedBackend)
            : _pluginBackendKey(route.pluginId, route.typeId));
  }

  String? _validatedSelectedBackend({
    required String selected,
    required List<OutputSinkTypeDescriptor> sinkTypes,
  }) {
    final values = <String>{
      ..._availableLocalBackends().map(_localBackendKey),
      ...sinkTypes.map((t) => _pluginBackendKey(t.pluginId, t.typeId)),
    };
    return values.contains(selected) ? selected : null;
  }

  void _resetInvalidPluginBackendSelectionIfNeeded({
    required String selected,
    required SettingsState settings,
    required ConnectionState connectionState,
  }) {
    if (!selected.startsWith('plugin:') ||
        connectionState != ConnectionState.done) {
      return;
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _updateUi(() {
        _selectedOutputBackendKey = _localBackendKey(settings.selectedBackend);
        _selectedOutputSinkTypeKey = null;
        _outputSinkTargets = const [];
      });
    });
  }

  Future<void> _handleOutputBackendChanged({
    required String? v,
    required List<OutputSinkTypeDescriptor> sinkTypes,
    required SettingsState settings,
    required BuildContext context,
  }) async {
    if (v == null) return;
    final messenger = ScaffoldMessenger.of(context);
    final local = _parseLocalBackendKey(v);
    _updateUi(() {
      _selectedOutputBackendKey = v;
      if (local != null) {
        _selectedOutputSinkTypeKey = null;
        _outputSinkTargets = const [];
      } else {
        _applyPluginSinkSelection(
          backendKey: v,
          sinkTypes: sinkTypes,
          activeRoute: settings.outputSinkRoute,
        );
      }
    });
    try {
      await _refreshSelectedOutputDevices(selectFirst: true);
      if (local == null && _normalizedOutputSinkTargetJson() == null) {
        messenger.showSnackBar(const SnackBar(content: Text('当前后端没有可用的输出设备。')));
        return;
      }
      await _applyOutputSinkRoute();
    } catch (e, s) {
      logger.e('failed to apply backend', error: e, stackTrace: s);
      messenger.showSnackBar(
        SnackBar(content: Text('Apply backend failed: $e')),
      );
    }
  }

  void _applyPluginSinkSelection({
    required String backendKey,
    required List<OutputSinkTypeDescriptor> sinkTypes,
    required OutputSinkRoute? activeRoute,
  }) {
    final sinkKey = _parsePluginTypeKey(backendKey);
    _selectedOutputSinkTypeKey = sinkKey;
    _outputSinkTargets = const [];

    OutputSinkTypeDescriptor? sink;
    for (final t in sinkTypes) {
      if (_outputSinkTypeKey(t) == sinkKey) {
        sink = t;
        break;
      }
    }
    _outputSinkConfigController.text = sink == null
        ? '{}'
        : _outputSinkConfigForType(sink);

    if (activeRoute != null &&
        sinkKey == '${activeRoute.pluginId}::${activeRoute.typeId}') {
      _outputSinkTargetController.text = activeRoute.targetJson;
    } else {
      _outputSinkTargetController.text = '{}';
    }
  }

  Widget _buildOutputDeviceField({
    required AppLocalizations l10n,
    required List<AudioDevice> devices,
  }) {
    final selectedBackendKey = _currentSelectedBackendKey();
    final localBackend = _parseLocalBackendKey(selectedBackendKey);
    return DropdownButtonFormField<String?>(
      decoration: InputDecoration(
        labelText: l10n.settingsDevice,
        border: const OutlineInputBorder(),
        isDense: true,
      ),
      initialValue: _resolvedOutputDeviceValue(
        devices: devices,
        localBackend: localBackend,
      ),
      items: _buildOutputDeviceItems(
        l10n: l10n,
        devices: devices,
        localBackend: localBackend,
      ),
      onChanged: (v) => _handleOutputDeviceChanged(
        v: v,
        localBackend: localBackend,
        context: context,
      ),
    );
  }

  String _currentSelectedBackendKey() {
    return _selectedOutputBackendKey ??
        _localBackendKey(ref.read(settingsStoreProvider).selectedBackend);
  }

  String? _resolvedOutputDeviceValue({
    required List<AudioDevice> devices,
    required AudioBackend? localBackend,
  }) {
    if (localBackend != null) {
      final selectedDeviceId = ref.watch(
        settingsStoreProvider.select((s) => s.selectedDeviceId),
      );
      final availableIds = devices
          .where((d) => d.backend == localBackend)
          .map((d) => d.id)
          .toSet();
      if (selectedDeviceId != null &&
          !availableIds.contains(selectedDeviceId)) {
        return null;
      }
      return selectedDeviceId;
    }
    final targetValue = _outputSinkTargetController.text.trim();
    final targetValues = _outputSinkTargets.map(_targetValueOf).toSet();
    return targetValues.contains(targetValue) ? targetValue : null;
  }

  List<DropdownMenuItem<String?>> _buildOutputDeviceItems({
    required AppLocalizations l10n,
    required List<AudioDevice> devices,
    required AudioBackend? localBackend,
  }) {
    if (localBackend != null) {
      return <DropdownMenuItem<String?>>[
        DropdownMenuItem(value: null, child: Text(l10n.settingsDeviceDefault)),
        ...devices
            .where((d) => d.backend == localBackend)
            .map((d) => DropdownMenuItem(value: d.id, child: Text(d.name))),
      ];
    }
    return <DropdownMenuItem<String?>>[
      for (final item in _outputSinkTargets)
        DropdownMenuItem(
          value: _targetValueOf(item),
          child: Text(_targetLabelOf(item)),
        ),
    ];
  }

  Future<void> _handleOutputDeviceChanged({
    required String? v,
    required AudioBackend? localBackend,
    required BuildContext context,
  }) async {
    if (localBackend == null) {
      final messenger = ScaffoldMessenger.of(context);
      _updateUi(() {
        _outputSinkTargetController.text = v ?? '{}';
      });
      try {
        await _applyOutputSinkRoute();
      } catch (e, s) {
        logger.e('failed to apply output sink route', error: e, stackTrace: s);
        messenger.showSnackBar(
          SnackBar(content: Text('Apply output sink route failed: $e')),
        );
      }
      return;
    }
    await ref.read(settingsStoreProvider.notifier).setSelectedDeviceId(v);
    await ref
        .read(playerBridgeProvider)
        .setOutputDevice(backend: localBackend, deviceId: v);
    _updateUi(() {});
  }

  Widget _buildWasapiExclusiveOptions(AppLocalizations l10n) {
    if (!SettingsValueUtils.supportsWasapiExclusive) {
      return const SizedBox.shrink();
    }
    final settings = ref.watch(settingsStoreProvider);
    final backend = _parseLocalBackendKey(
      _selectedOutputBackendKey ?? _localBackendKey(settings.selectedBackend),
    );
    if (backend != AudioBackend.wasapiExclusive) {
      return const SizedBox.shrink();
    }
    return Column(
      children: [
        SwitchListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          title: Text(l10n.settingsMatchTrackSampleRate),
          value: settings.matchTrackSampleRate,
          onChanged: (v) async {
            await ref
                .read(settingsStoreProvider.notifier)
                .setMatchTrackSampleRate(v);
            await _applyOutputOptions(matchTrackSampleRate: v);
            _updateUi(() {});
          },
        ),
        SwitchListTile(
          dense: true,
          contentPadding: EdgeInsets.zero,
          title: Text(l10n.settingsGaplessPlayback),
          value: settings.gaplessPlayback,
          onChanged: (v) async {
            await ref
                .read(settingsStoreProvider.notifier)
                .setGaplessPlayback(v);
            await _applyOutputOptions(gaplessPlayback: v);
            _updateUi(() {});
          },
        ),
      ],
    );
  }

  Widget _buildSeekTrackFadeOption(AppLocalizations l10n) {
    return SwitchListTile(
      dense: true,
      contentPadding: EdgeInsets.zero,
      title: Text(l10n.settingsSeekTrackFade),
      value: ref.watch(settingsStoreProvider).seekTrackFade,
      onChanged: (v) async {
        await ref.read(settingsStoreProvider.notifier).setSeekTrackFade(v);
        await _applyOutputOptions(seekTrackFade: v);
        _updateUi(() {});
      },
    );
  }

  Widget _buildPlaybackLatencyField(AppLocalizations l10n) {
    final value = ref.watch(settingsStoreProvider).playbackLatency;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: DropdownButtonFormField<PlaybackLatency>(
        key: ValueKey((value, _playbackLatencyRevision)),
        initialValue: value,
        decoration: InputDecoration(
          labelText: l10n.settingsPlaybackLatency,
          helperText: l10n.settingsPlaybackLatencyHint,
          helperMaxLines: 3,
          border: const OutlineInputBorder(),
          isDense: true,
        ),
        items: [
          DropdownMenuItem(
            value: PlaybackLatency.low,
            child: Text(l10n.settingsPlaybackLatencyLow),
          ),
          DropdownMenuItem(
            value: PlaybackLatency.medium,
            child: Text(l10n.settingsPlaybackLatencyMedium),
          ),
          DropdownMenuItem(
            value: PlaybackLatency.high,
            child: Text(l10n.settingsPlaybackLatencyHigh),
          ),
        ],
        onChanged: _applyingPlaybackLatency
            ? null
            : (next) async {
                if (next == null || next == value) return;
                _updateUi(() {
                  _applyingPlaybackLatency = true;
                });
                try {
                  await ref.read(playerBridgeProvider).setPlaybackLatency(next);
                  await ref
                      .read(settingsStoreProvider.notifier)
                      .setPlaybackLatency(next);
                } catch (error, stack) {
                  logger.w(
                    'failed to set playback latency',
                    error: error,
                    stackTrace: stack,
                  );
                  if (mounted) {
                    ScaffoldMessenger.of(
                      context,
                    ).showSnackBar(SnackBar(content: Text(error.toString())));
                    _updateUi(() {
                      _playbackLatencyRevision++;
                    });
                  }
                } finally {
                  _updateUi(() {
                    _applyingPlaybackLatency = false;
                  });
                }
              },
      ),
    );
  }

  Widget _buildResampleQualityField(AppLocalizations l10n) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8.0),
      child: DropdownButtonFormField<ResampleQuality>(
        decoration: InputDecoration(
          labelText: l10n.settingsResampleQuality,
          border: const OutlineInputBorder(),
          isDense: true,
        ),
        initialValue: _resampleQuality,
        items: [
          DropdownMenuItem(
            value: ResampleQuality.fast,
            child: Text(l10n.settingsResampleQualityFast),
          ),
          DropdownMenuItem(
            value: ResampleQuality.balanced,
            child: Text(l10n.settingsResampleQualityBalanced),
          ),
          DropdownMenuItem(
            value: ResampleQuality.high,
            child: Text(l10n.settingsResampleQualityHigh),
          ),
          DropdownMenuItem(
            value: ResampleQuality.ultra,
            child: Text(l10n.settingsResampleQualityUltra),
          ),
        ],
        onChanged: (v) async {
          if (v == null) return;
          await ref.read(settingsStoreProvider.notifier).setResampleQuality(v);
          _updateUi(() {
            _resampleQuality = v;
            _persistOutputUiSession();
          });
          await _applyOutputOptions(resampleQuality: v);
        },
      ),
    );
  }

  Widget _buildPluginsCard(AppLocalizations l10n) {
    return SettingsSectionCard(
      title: l10n.settingsPluginsTitle,
      headerBottomSpacing: 6,
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            visualDensity: VisualDensity.compact,
            tooltip: l10n.settingsInstallPlugin,
            onPressed: _installPluginArtifact,
            icon: const Icon(Icons.add),
          ),
          IconButton(
            visualDensity: VisualDensity.compact,
            tooltip: l10n.refresh,
            onPressed: () => _updateUi(_refresh),
            icon: const Icon(Icons.refresh),
          ),
          FutureBuilder<String>(
            future: _pluginDirFuture,
            builder: (context, snap) {
              final dir = snap.data;
              if (dir == null) return const SizedBox.shrink();
              return IconButton(
                visualDensity: VisualDensity.compact,
                tooltip: l10n.settingsOpenPluginDir,
                onPressed: () async {
                  final uri = Uri.directory(dir);
                  if (await canLaunchUrl(uri)) {
                    await launchUrl(uri);
                  }
                },
                icon: const Icon(Icons.folder_open_outlined),
              );
            },
          ),
        ],
      ),
      children: [
        FutureBuilder<String>(
          future: _pluginDirFuture,
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
        _buildPluginsList(l10n),
      ],
    );
  }

  Widget _buildPluginsList(AppLocalizations l10n) {
    return FutureBuilder<_PluginRuntimeSnapshot>(
      future: _pluginRuntimeSnapshotFuture,
      builder: (context, snap) {
        final runtime = snap.data;
        if (runtime != null) {
          _updatePluginRuntimeCache(runtime);
        }
        final items =
            runtime?.installedPlugins ??
            (_cachedInstalledPluginsReady
                ? _cachedInstalledPlugins
                : const <InstalledPlugin>[]);
        final disabled =
            runtime?.disabledPluginIds ??
            (_cachedDisabledPluginIdsReady
                ? _cachedDisabledPluginIds
                : <String>{});
        final loadedIds =
            runtime?.loadedPluginIds ??
            (_cachedLoadedPluginIdsReady ? _cachedLoadedPluginIds : <String>{});
        final loadedKnown = runtime != null || _cachedLoadedPluginIdsReady;
        final sourceTypes =
            runtime?.sourceTypes ??
            (_cachedSourceTypesReady
                ? _cachedSourceTypes
                : const <SourceCatalogTypeDescriptor>[]);
        if (snap.connectionState != ConnectionState.done && items.isEmpty) {
          return const LinearProgressIndicator();
        }
        if (items.isEmpty) {
          return Text(l10n.settingsNoPlugins);
        }
        return _buildPluginTiles(
          items: items,
          disabled: disabled,
          loadedIds: loadedIds,
          loadedKnown: loadedKnown,
          sourceTypes: sourceTypes,
        );
      },
    );
  }

  Future<_PluginRuntimeSnapshot> _createPluginRuntimeSnapshotFuture() async {
    final installedPlugins = await _awaitOrDefault<List<InstalledPlugin>>(
      _installedPluginsFuture,
      const <InstalledPlugin>[],
      label: 'installed plugins',
    );
    final disabledPluginIds = await _awaitOrDefault<Set<String>>(
      _disabledPluginIdsFuture,
      const <String>{},
      label: 'disabled plugin ids',
    );
    final loadedPlugins = await _awaitOrDefault<List<PluginDescriptor>>(
      _pluginsFuture,
      const <PluginDescriptor>[],
      label: 'loaded plugins',
    );
    final sourceTypes =
        await _awaitOrDefault<List<SourceCatalogTypeDescriptor>>(
          _sourceTypesFuture,
          const <SourceCatalogTypeDescriptor>[],
          label: 'source types',
        );
    return _PluginRuntimeSnapshot(
      installedPlugins: installedPlugins,
      disabledPluginIds: disabledPluginIds,
      loadedPluginIds: loadedPlugins.map((p) => p.id).toSet(),
      sourceTypes: sourceTypes,
    );
  }

  Future<T> _awaitOrDefault<T>(
    Future<T>? future,
    T fallback, {
    required String label,
  }) async {
    if (future == null) return fallback;
    try {
      return await future;
    } catch (e, s) {
      logger.w('failed to load $label', error: e, stackTrace: s);
      return fallback;
    }
  }

  void _updatePluginRuntimeCache(_PluginRuntimeSnapshot runtime) {
    _cachedInstalledPlugins = runtime.installedPlugins;
    _cachedInstalledPluginsReady = true;
    _cachedDisabledPluginIds = runtime.disabledPluginIds;
    _cachedDisabledPluginIdsReady = true;
    _cachedLoadedPluginIds = runtime.loadedPluginIds;
    _cachedLoadedPluginIdsReady = true;
    _cachedSourceTypes = runtime.sourceTypes;
    _cachedSourceTypesReady = true;
  }

  Widget _buildPluginTiles({
    required List<InstalledPlugin> items,
    required Set<String> disabled,
    required Set<String> loadedIds,
    required bool loadedKnown,
    required List<SourceCatalogTypeDescriptor> sourceTypes,
  }) {
    return SettingsPluginsList(
      plugins: items,
      disabledPluginIds: disabled,
      loadedPluginIds: loadedIds,
      loadedKnown: loadedKnown,
      sourceTypes: sourceTypes,
      outputTypes: _cachedOutputSinkTypesReady
          ? _cachedOutputSinkTypes
          : const <OutputSinkTypeDescriptor>[],
      onOpenWebUi: _openPluginWebUi,
      onToggleEnabled: _setPluginEnabled,
      onUninstall: _uninstallPlugin,
      outputSinkConfigForType: _outputSinkConfigForType,
      onOutputSinkConfigChanged: _handleOutputSinkConfigChanged,
    );
  }

  void _handleOutputSinkConfigChanged(OutputSinkTypeDescriptor t, String json) {
    final key = _outputSinkTypeKey(t);
    _outputSinkConfigDrafts[key] = json;
    if (_selectedOutputSinkTypeKey != key) {
      return;
    }
    _outputSinkConfigController.text = json;
    _outputSinkConfigApplyDebounce?.cancel();
    _outputSinkConfigApplyDebounce = Timer(
      const Duration(milliseconds: 350),
      () async {
        if (!mounted) return;
        try {
          await _applyOutputSinkRoute();
        } catch (e, s) {
          logger.e(
            'failed to apply output sink route in debounce',
            error: e,
            stackTrace: s,
          );
        }
      },
    );
  }

  Widget _buildLyricsCacheCard(AppLocalizations l10n) {
    return SettingsLyricsCacheSection(
      l10n: l10n,
      onClearLyricsCache: _clearLyricsCache,
    );
  }
}
