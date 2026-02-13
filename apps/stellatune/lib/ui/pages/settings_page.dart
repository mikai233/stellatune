import 'dart:io';
import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/lyrics/lyrics_controller.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/forms/schema_form.dart';
import 'package:stellatune/app/logging.dart';

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
    required this.installState,
    required this.uninstallRetryCount,
    required this.uninstallLastError,
  });

  final String dirPath;
  final String? id;
  final String? name;
  final String? infoJson;
  final String installState;
  final int uninstallRetryCount;
  final String? uninstallLastError;

  String get nameOrDir => name ?? p.basename(dirPath);
  bool get isInstalled => installState == 'installed';
  bool get isPendingUninstall => installState == 'pending_uninstall';
  bool get isDeleteFailed => installState == 'delete_failed';
}

class SettingsPageState extends ConsumerState<SettingsPage> {
  static const Duration _bridgeQueryTimeout = Duration(seconds: 8);

  Future<List<PluginDescriptor>>? _pluginsFuture;
  Future<List<OutputSinkTypeDescriptor>>? _outputSinkTypesFuture;
  Future<List<SourceCatalogTypeDescriptor>>? _sourceTypesFuture;
  Future<List<_InstalledPlugin>>? _installedPluginsFuture;
  Future<Set<String>>? _disabledPluginIdsFuture;
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
  final TextEditingController _neteaseKeywordsController =
      TextEditingController();
  final TextEditingController _neteasePlaylistIdController =
      TextEditingController();
  final TextEditingController _neteaseSidecarBaseUrlController =
      TextEditingController(text: 'http://127.0.0.1:46321');
  final TextEditingController _neteaseLevelController = TextEditingController(
    text: 'standard',
  );
  final TextEditingController _neteaseLimitController = TextEditingController(
    text: '30',
  );
  StreamSubscription<PluginRuntimeEvent>? _pluginRuntimeSub;
  Timer? _outputSinkConfigApplyDebounce;
  final List<PluginRuntimeEvent> _pluginRuntimeEvents = <PluginRuntimeEvent>[];
  List<QueueItem> _neteaseDebugItems = const [];
  bool _neteaseDebugLoading = false;
  String? _neteaseDebugError;
  bool _neteaseAuthBusy = false;
  bool _neteaseAuthAutoPolling = false;
  Timer? _neteaseAuthPollTimer;
  Future<void>? _ensureNeteaseSidecarInFlight;
  String? _neteaseAuthMessage;
  String? _neteaseQrKey;
  String? _neteaseQrUrl;
  String? _neteaseQrImageDataUrl;
  Map<String, Object?>? _neteaseLoginStatus;
  List<Object?> _outputSinkTargets = const [];
  bool _loadingOutputSinkTargets = false;
  final Map<String, String> _sourceConfigDrafts = <String, String>{};
  final Map<String, String> _outputSinkConfigDrafts = <String, String>{};
  Set<String> _cachedLoadedPluginIds = <String>{};
  bool _cachedLoadedPluginIdsReady = false;
  Set<String> _cachedDisabledPluginIds = <String>{};
  bool _cachedDisabledPluginIdsReady = false;
  List<OutputSinkTypeDescriptor> _cachedOutputSinkTypes = const [];
  bool _cachedOutputSinkTypesReady = false;
  List<SourceCatalogTypeDescriptor> _cachedSourceTypes = const [];
  bool _cachedSourceTypesReady = false;

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
    unawaited(_syncNeteaseSidecarBaseUrlFromConfig());
    unawaited(_ensureNeteaseSidecarResident());
  }

  @override
  void dispose() {
    _persistOutputUiSession();
    _outputSinkConfigApplyDebounce?.cancel();
    _outputSinkConfigController.dispose();
    _outputSinkTargetController.dispose();
    _pluginRuntimeTargetIdController.dispose();
    _pluginRuntimeJsonController.dispose();
    _neteaseKeywordsController.dispose();
    _neteasePlaylistIdController.dispose();
    _neteaseSidecarBaseUrlController.dispose();
    _neteaseLevelController.dispose();
    _neteaseLimitController.dispose();
    _neteaseAuthPollTimer?.cancel();
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

  String _formatRuntimeEventLine(PluginRuntimeEvent event) {
    final prefix = '[${event.kind.name}] ${event.pluginId}';
    final payload = _tryDecodeRuntimePayload(event.payloadJson);
    if (payload == null) {
      return '$prefix: ${event.payloadJson}';
    }
    final topic = payload['topic']?.toString().trim() ?? '';
    if (topic != 'host.instance.config_update') {
      return '$prefix: ${event.payloadJson}';
    }
    final capability = payload['capability']?.toString().trim() ?? 'unknown';
    final typeId = payload['type_id']?.toString().trim() ?? 'unknown';
    final statusRaw = payload['status']?.toString().trim() ?? 'unknown';
    final status = _runtimeConfigUpdateStatusLabel(statusRaw);
    final generation = payload['generation']?.toString().trim();
    final detail = payload['detail']?.toString().trim();
    final genText = (generation == null || generation.isEmpty)
        ? ''
        : ' gen=$generation';
    final detailText = (detail == null || detail.isEmpty) ? '' : ' ($detail)';
    return '$prefix: $capability/$typeId -> $status$genText$detailText';
  }

  Map<String, Object?>? _tryDecodeRuntimePayload(String payloadJson) {
    try {
      final decoded = jsonDecode(payloadJson);
      if (decoded is Map<String, dynamic>) {
        return decoded.cast<String, Object?>();
      }
      if (decoded is Map) {
        return decoded.map(
          (key, value) => MapEntry(key.toString(), value as Object?),
        );
      }
    } catch (_) {
      return null;
    }
    return null;
  }

  String _runtimeConfigUpdateStatusLabel(String rawStatus) {
    final status = rawStatus.toLowerCase();
    final isZh = Localizations.localeOf(context).languageCode == 'zh';
    if (isZh) {
      return switch (status) {
        'applied' => '已热更新',
        'requires_recreate' => '需要重建',
        'recreated' => '已重建',
        'rejected' => '已拒绝',
        'failed' => '失败',
        _ => rawStatus,
      };
    }
    return switch (status) {
      'applied' => 'applied',
      'requires_recreate' => 'requires recreate',
      'recreated' => 'recreated',
      'rejected' => 'rejected',
      'failed' => 'failed',
      _ => rawStatus,
    };
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
    final library = ref.read(libraryBridgeProvider);
    _pluginsFuture = _listLoadedPlugins(bridge);
    _outputSinkTypesFuture = null;
    _cachedOutputSinkTypes = const [];
    _cachedOutputSinkTypesReady = false;
    _sourceTypesFuture = _listSourceTypes(bridge);
    _installedPluginsFuture = _listInstalledPlugins();
    _disabledPluginIdsFuture = _listDisabledPluginIds(library);
  }

  void _refreshPluginRuntimeState() {
    final bridge = ref.read(playerBridgeProvider);
    final library = ref.read(libraryBridgeProvider);
    _pluginsFuture = _listLoadedPlugins(bridge);
    _outputSinkTypesFuture = null;
    _cachedOutputSinkTypes = const [];
    _cachedOutputSinkTypesReady = false;
    _sourceTypesFuture = _listSourceTypes(bridge);
    _disabledPluginIdsFuture = _listDisabledPluginIds(library);
  }

  Future<List<PluginDescriptor>> _listLoadedPlugins(PlayerBridge bridge) async {
    try {
      return await bridge.pluginsList().timeout(_bridgeQueryTimeout);
    } on TimeoutException catch (e, s) {
      logger.w('pluginsList timed out', error: e, stackTrace: s);
      return const <PluginDescriptor>[];
    } catch (e, s) {
      logger.w('pluginsList failed', error: e, stackTrace: s);
      return const <PluginDescriptor>[];
    }
  }

  Future<List<SourceCatalogTypeDescriptor>> _listSourceTypes(
    PlayerBridge bridge,
  ) async {
    try {
      return await bridge.sourceListTypes().timeout(_bridgeQueryTimeout);
    } on TimeoutException catch (e, s) {
      logger.w('sourceListTypes timed out', error: e, stackTrace: s);
      return const <SourceCatalogTypeDescriptor>[];
    } catch (e, s) {
      logger.w('sourceListTypes failed', error: e, stackTrace: s);
      return const <SourceCatalogTypeDescriptor>[];
    }
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
      final installStateRaw = (map['install_state'] ?? 'installed')
          .toString()
          .trim();
      final uninstallRetryCountRaw = map['uninstall_retry_count'];
      final uninstallRetryCount = switch (uninstallRetryCountRaw) {
        int v => v,
        num v => v.toInt(),
        String v => int.tryParse(v) ?? 0,
        _ => 0,
      };
      final uninstallLastErrorRaw = (map['uninstall_last_error'] ?? '')
          .toString()
          .trim();
      out.add(
        _InstalledPlugin(
          dirPath: dirPath.isEmpty ? p.join(_pluginDir!, id) : dirPath,
          id: id,
          name: nameRaw.isEmpty ? null : nameRaw,
          infoJson: infoRaw.isEmpty ? null : infoRaw,
          installState: installStateRaw.isEmpty ? 'installed' : installStateRaw,
          uninstallRetryCount: uninstallRetryCount < 0
              ? 0
              : uninstallRetryCount,
          uninstallLastError: uninstallLastErrorRaw.isEmpty
              ? null
              : uninstallLastErrorRaw,
        ),
      );
    }
    out.sort((a, b) => (a.nameOrDir).compareTo(b.nameOrDir));
    return out;
  }

  Future<Set<String>> _listDisabledPluginIds(LibraryBridge library) async {
    final ids = await library.listDisabledPluginIds();
    return ids.map((id) => id.trim()).where((id) => id.isNotEmpty).toSet();
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
    if (t.typeId.trim() == 'netease') {
      unawaited(_syncNeteaseSidecarBaseUrlFromConfig());
      unawaited(_ensureNeteaseSidecarResident());
    }
    if (!mounted) return;
    final l10n = AppLocalizations.of(context)!;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(l10n.settingsSourceConfigSaved)));
  }

  Future<void> _setPluginEnabled({
    required _InstalledPlugin plugin,
    required bool enabled,
  }) async {
    final id = plugin.id?.trim();
    if (id == null || id.isEmpty) return;
    final library = ref.read(libraryBridgeProvider);
    if (!enabled) {
      await _shutdownNeteaseSidecarResident(onlyForPluginId: id, silent: true);
    }
    if (enabled) {
      await library.pluginEnable(pluginId: id);
    } else {
      await library.pluginDisable(pluginId: id);
    }
    await library.pluginApplyState();
    if (enabled) {
      unawaited(
        _ensureNeteaseSidecarResident(onlyForPluginId: id, silent: true),
      );
    }
    if (mounted) {
      _loadFromSettings();
      setState(_refreshPluginRuntimeState);
    }
  }

  Future<void> _uninstallPlugin(_InstalledPlugin plugin) async {
    await _ensurePluginDir();
    final library = ref.read(libraryBridgeProvider);
    final pluginId = plugin.id?.trim();
    if (pluginId != null && pluginId.isNotEmpty) {
      await _shutdownNeteaseSidecarResident(
        onlyForPluginId: pluginId,
        silent: true,
      );
    }
    if (plugin.id != null && plugin.id!.trim().isNotEmpty) {
      await ref
          .read(playerBridgeProvider)
          .pluginsUninstallById(dir: _pluginDir!, pluginId: plugin.id!);
      await library.pluginEnable(pluginId: plugin.id!);
    } else {
      await Directory(plugin.dirPath).delete(recursive: true);
    }
    await library.pluginApplyState();
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

    FilePickerResult? picked;
    try {
      picked = await FilePicker.platform.pickFiles(
        dialogTitle: l10n.settingsInstallPluginPickFolder,
        type: FileType.custom,
        allowMultiple: false,
        allowedExtensions: ['zip', _pluginLibExtForPlatform()],
        lockParentWindow: true,
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
    final files = picked?.files;
    if (files == null || files.isEmpty) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('No plugin file selected.')));
      return;
    }
    final srcPath = files.first.path?.trim();
    if (srcPath == null || srcPath.isEmpty) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Selected file path is empty.')),
      );
      return;
    }

    try {
      final bridge = ref.read(playerBridgeProvider);
      final library = ref.read(libraryBridgeProvider);
      final installedPluginId = await bridge.pluginsInstallFromFile(
        dir: pluginDir,
        artifactPath: srcPath,
      );
      await library.pluginApplyState();
      unawaited(
        _ensureNeteaseSidecarResident(
          onlyForPluginId: installedPluginId,
          silent: true,
        ),
      );
      if (!mounted) return;
      setState(_refresh);
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.settingsPluginInstalled)));
    } catch (e, s) {
      logger.e('failed to install plugin', error: e, stackTrace: s);
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
    } catch (e, s) {
      logger.w(
        'failed to decode output sink targets JSON',
        error: e,
        stackTrace: s,
      );
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
    } catch (e, s) {
      logger.e('failed to load output sink targets', error: e, stackTrace: s);
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
    } catch (e, s) {
      logger.e('failed to clear lyrics cache', error: e, stackTrace: s);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.settingsClearLyricsCacheFailed)),
      );
    }
  }

  String get _neteaseSidecarBaseUrl {
    final raw = _neteaseSidecarBaseUrlController.text.trim();
    if (raw.isEmpty) return 'http://127.0.0.1:46321';
    return raw.endsWith('/') ? raw.substring(0, raw.length - 1) : raw;
  }

  Future<void> _syncNeteaseSidecarBaseUrlFromConfig() async {
    try {
      final sourceType = await _resolveNeteaseSourceType();
      if (sourceType == null) return;
      final config = _decodeJsonObjectOrEmpty(_sourceConfigForType(sourceType));
      final sidecarBaseUrl = _asText(config['sidecar_base_url']);
      if (sidecarBaseUrl == null || sidecarBaseUrl.isEmpty) return;
      if (!mounted) return;
      if (_neteaseSidecarBaseUrlController.text.trim() ==
          sidecarBaseUrl.trim()) {
        return;
      }
      setState(() {
        _neteaseSidecarBaseUrlController.text = sidecarBaseUrl.trim();
      });
    } catch (e, s) {
      logger.d(
        'failed to sync netease sidecar base url from config',
        error: e,
        stackTrace: s,
      );
    }
  }

  Future<void> _ensureNeteaseSidecarResident({
    String? onlyForPluginId,
    bool silent = true,
  }) async {
    final inFlight = _ensureNeteaseSidecarInFlight;
    if (inFlight != null) {
      await inFlight;
      return;
    }

    final task = () async {
      try {
        final sourceType = await _resolveNeteaseSourceType();
        if (sourceType == null) return;
        final expected = onlyForPluginId?.trim() ?? '';
        if (expected.isNotEmpty && sourceType.pluginId.trim() != expected) {
          return;
        }
        final bridge = ref.read(playerBridgeProvider);
        final config = _decodeJsonObjectOrEmpty(
          _sourceConfigForType(sourceType),
        );
        await bridge.sourceListItemsJson(
          pluginId: sourceType.pluginId,
          typeId: sourceType.typeId,
          configJson: jsonEncode(config),
          requestJson: jsonEncode(<String, Object?>{
            'action': 'ensure_sidecar',
          }),
        );
      } catch (e, s) {
        logger.w(
          'failed to ensure netease sidecar resident',
          error: e,
          stackTrace: s,
        );
        if (!silent && mounted) {
          setState(() => _neteaseAuthMessage = 'Sidecar 启动失败: $e');
        }
      }
    }();
    _ensureNeteaseSidecarInFlight = task;
    try {
      await task;
    } finally {
      if (identical(_ensureNeteaseSidecarInFlight, task)) {
        _ensureNeteaseSidecarInFlight = null;
      }
    }
  }

  Future<void> _shutdownNeteaseSidecarResident({
    String? onlyForPluginId,
    bool silent = true,
  }) async {
    try {
      final sourceType = await _resolveNeteaseSourceType();
      if (sourceType == null) return;
      final expected = onlyForPluginId?.trim() ?? '';
      if (expected.isNotEmpty && sourceType.pluginId.trim() != expected) {
        return;
      }
      final bridge = ref.read(playerBridgeProvider);
      final config = _decodeJsonObjectOrEmpty(_sourceConfigForType(sourceType));
      await bridge.sourceListItemsJson(
        pluginId: sourceType.pluginId,
        typeId: sourceType.typeId,
        configJson: jsonEncode(config),
        requestJson: jsonEncode(<String, Object?>{
          'action': 'shutdown_sidecar',
        }),
      );
    } catch (e, s) {
      logger.w('failed to shutdown netease sidecar', error: e, stackTrace: s);
      if (!silent && mounted) {
        setState(() => _neteaseAuthMessage = 'Sidecar 关闭失败: $e');
      }
    }
  }

  Future<Map<String, Object?>> _neteaseSidecarGetJson(
    String path, {
    Map<String, Object?> query = const <String, Object?>{},
  }) async {
    final uri = Uri.parse('$_neteaseSidecarBaseUrl$path').replace(
      queryParameters: <String, String>{
        for (final entry in query.entries)
          if (entry.value != null) entry.key: entry.value.toString(),
      },
    );
    final client = HttpClient()..connectionTimeout = const Duration(seconds: 5);
    try {
      final request = await client.getUrl(uri);
      final response = await request.close().timeout(
        const Duration(seconds: 10),
      );
      final body = await response.transform(utf8.decoder).join();
      dynamic decoded;
      try {
        decoded = body.isEmpty ? <String, Object?>{} : jsonDecode(body);
      } catch (_) {
        decoded = <String, Object?>{'raw': body};
      }
      if (response.statusCode < 200 || response.statusCode >= 300) {
        final message = decoded is Map
            ? (decoded['error'] ?? decoded['message'] ?? decoded['raw'])
            : null;
        throw Exception(
          'HTTP ${response.statusCode}${message == null ? '' : ': $message'}',
        );
      }
      if (decoded is Map<String, Object?>) return decoded;
      if (decoded is Map) return decoded.cast<String, Object?>();
      return <String, Object?>{'data': decoded};
    } finally {
      client.close(force: true);
    }
  }

  int? _extractAuthCodeFromBody(Map<String, Object?> data) {
    final body = data['body'];
    if (body is! Map) return null;
    final code = body['code'];
    if (code is int) return code;
    if (code is num) return code.toInt();
    return int.tryParse(code?.toString() ?? '');
  }

  String _describeNeteaseQrCode(int? code) {
    return switch (code) {
      800 => '二维码已过期',
      801 => '等待扫码',
      802 => '已扫码，等待确认',
      803 => '登录成功',
      200 => '请求成功',
      null => '未知状态',
      _ => '状态码: $code',
    };
  }

  Future<void> _fetchNeteaseLoginStatus() async {
    if (_neteaseAuthBusy) return;
    setState(() => _neteaseAuthBusy = true);
    try {
      await _ensureNeteaseSidecarResident();
      final data = await _neteaseSidecarGetJson('/v1/auth/login_status');
      Map<String, Object?>? bodyMap;
      final body = data['body'];
      if (body is Map<String, Object?>) bodyMap = body;
      if (body is Map && bodyMap == null) {
        bodyMap = body.cast<String, Object?>();
      }
      final profileRaw =
          (bodyMap?['data'] as Map?)?['profile'] ??
          bodyMap?['profile'] ??
          (bodyMap?['profileData']);
      String? nickname;
      if (profileRaw is Map) {
        nickname = _asText(profileRaw['nickname']);
      }
      final message = nickname == null || nickname.isEmpty
          ? '登录状态已刷新'
          : '当前账号: $nickname';
      if (!mounted) return;
      setState(() {
        _neteaseLoginStatus = bodyMap;
        _neteaseAuthMessage = message;
      });
    } catch (e, s) {
      logger.e('failed to fetch netease login status', error: e, stackTrace: s);
      if (!mounted) return;
      setState(() => _neteaseAuthMessage = '登录状态获取失败: $e');
    } finally {
      if (mounted) {
        setState(() => _neteaseAuthBusy = false);
      }
    }
  }

  Future<void> _startNeteaseQrLogin() async {
    if (_neteaseAuthBusy) return;
    setState(() {
      _neteaseAuthBusy = true;
      _neteaseAuthMessage = null;
    });
    try {
      await _ensureNeteaseSidecarResident(silent: false);
      final keyResp = await _neteaseSidecarGetJson('/v1/auth/qr/key');
      final keyBody = keyResp['body'];
      final keyMap = keyBody is Map
          ? (keyBody is Map<String, Object?>
                ? keyBody
                : keyBody.cast<String, Object?>())
          : <String, Object?>{};
      final keyData = keyMap['data'];
      final keyDataMap = keyData is Map
          ? (keyData is Map<String, Object?>
                ? keyData
                : keyData.cast<String, Object?>())
          : <String, Object?>{};
      final key = _asText(keyDataMap['unikey']) ?? _asText(keyDataMap['key']);
      if (key == null || key.isEmpty) {
        throw Exception('未获取到二维码 key');
      }

      final createResp = await _neteaseSidecarGetJson(
        '/v1/auth/qr/create',
        query: <String, Object?>{'key': key, 'qrimg': 'true'},
      );
      final createBody = createResp['body'];
      final createMap = createBody is Map
          ? (createBody is Map<String, Object?>
                ? createBody
                : createBody.cast<String, Object?>())
          : <String, Object?>{};
      final createData = createMap['data'];
      final createDataMap = createData is Map
          ? (createData is Map<String, Object?>
                ? createData
                : createData.cast<String, Object?>())
          : <String, Object?>{};

      if (!mounted) return;
      setState(() {
        _neteaseQrKey = key;
        _neteaseQrUrl = _asText(createDataMap['qrurl']);
        _neteaseQrImageDataUrl = _asText(createDataMap['qrimg']);
        _neteaseAuthMessage = '二维码已生成，请使用网易云扫码';
      });
      _setNeteaseAuthAutoPolling(true);
    } catch (e, s) {
      logger.e('failed to start netease qr login', error: e, stackTrace: s);
      if (!mounted) return;
      setState(() => _neteaseAuthMessage = '二维码生成失败: $e');
    } finally {
      if (mounted) {
        setState(() => _neteaseAuthBusy = false);
      }
    }
  }

  Future<void> _pollNeteaseQrStatus({bool silent = false}) async {
    if (_neteaseAuthBusy) return;
    final key = _neteaseQrKey?.trim() ?? '';
    if (key.isEmpty) {
      if (!silent && mounted) {
        setState(() => _neteaseAuthMessage = '请先生成二维码');
      }
      return;
    }
    setState(() => _neteaseAuthBusy = true);
    try {
      final resp = await _neteaseSidecarGetJson(
        '/v1/auth/qr/check',
        query: <String, Object?>{'key': key},
      );
      final code = _extractAuthCodeFromBody(resp);
      final statusText = _describeNeteaseQrCode(code);

      if (!mounted) return;
      setState(() {
        _neteaseAuthMessage = statusText;
      });

      if (code == 803) {
        _setNeteaseAuthAutoPolling(false);
        await _fetchNeteaseLoginStatus();
      } else if (code == 800) {
        _setNeteaseAuthAutoPolling(false);
      }
    } catch (e, s) {
      logger.e('failed to poll netease qr status', error: e, stackTrace: s);
      if (!mounted || silent) return;
      setState(() => _neteaseAuthMessage = '二维码状态检查失败: $e');
    } finally {
      if (mounted) {
        setState(() => _neteaseAuthBusy = false);
      }
    }
  }

  Future<void> _refreshNeteaseLogin() async {
    if (_neteaseAuthBusy) return;
    setState(() => _neteaseAuthBusy = true);
    try {
      await _ensureNeteaseSidecarResident();
      await _neteaseSidecarGetJson('/v1/auth/login_refresh');
      if (!mounted) return;
      setState(() => _neteaseAuthMessage = '登录状态已刷新');
      await _fetchNeteaseLoginStatus();
    } catch (e, s) {
      logger.e('failed to refresh netease login', error: e, stackTrace: s);
      if (!mounted) return;
      setState(() => _neteaseAuthMessage = '刷新登录失败: $e');
    } finally {
      if (mounted) {
        setState(() => _neteaseAuthBusy = false);
      }
    }
  }

  Future<void> _logoutNeteaseLogin() async {
    if (_neteaseAuthBusy) return;
    setState(() => _neteaseAuthBusy = true);
    try {
      await _ensureNeteaseSidecarResident();
      await _neteaseSidecarGetJson('/v1/auth/logout');
      if (!mounted) return;
      _setNeteaseAuthAutoPolling(false);
      setState(() {
        _neteaseLoginStatus = null;
        _neteaseAuthMessage = '已退出登录';
      });
    } catch (e, s) {
      logger.e('failed to logout netease login', error: e, stackTrace: s);
      if (!mounted) return;
      setState(() => _neteaseAuthMessage = '退出登录失败: $e');
    } finally {
      if (mounted) {
        setState(() => _neteaseAuthBusy = false);
      }
    }
  }

  void _setNeteaseAuthAutoPolling(bool enabled) {
    _neteaseAuthPollTimer?.cancel();
    _neteaseAuthPollTimer = null;
    if (!mounted) return;
    setState(() => _neteaseAuthAutoPolling = enabled);
    if (!enabled) return;
    _neteaseAuthPollTimer = Timer.periodic(const Duration(seconds: 3), (_) {
      unawaited(_pollNeteaseQrStatus(silent: true));
    });
  }

  Uint8List? _decodeDataUrlBytes(String? dataUrl) {
    final raw = dataUrl?.trim() ?? '';
    if (raw.isEmpty) return null;
    final comma = raw.indexOf(',');
    if (comma <= 0 || comma >= raw.length - 1) return null;
    final base64Raw = raw.substring(comma + 1);
    try {
      return base64Decode(base64Raw);
    } catch (_) {
      return null;
    }
  }

  Future<SourceCatalogTypeDescriptor?> _resolveNeteaseSourceType() async {
    final bridge = ref.read(playerBridgeProvider);
    final sourceTypes = await (_sourceTypesFuture ??= bridge.sourceListTypes());
    SourceCatalogTypeDescriptor? picked;
    for (final t in sourceTypes) {
      if (t.typeId.trim() != 'netease') continue;
      if (t.pluginId.trim() == 'dev.stellatune.source.netease') {
        return t;
      }
      picked ??= t;
    }
    return picked;
  }

  Map<String, Object?> _decodeJsonObjectOrEmpty(String raw) {
    final text = raw.trim();
    if (text.isEmpty) return <String, Object?>{};
    try {
      final decoded = jsonDecode(text);
      if (decoded is Map<String, Object?>) return decoded;
      if (decoded is Map) {
        return decoded.cast<String, Object?>();
      }
    } catch (e, s) {
      logger.w('failed to parse source config JSON', error: e, stackTrace: s);
    }
    return <String, Object?>{};
  }

  String? _asText(Object? value) {
    if (value == null) return null;
    final text = value.toString().trim();
    if (text.isEmpty) return null;
    return text;
  }

  int? _asInt(Object? value) {
    if (value is int) return value;
    if (value is num) return value.toInt();
    return int.tryParse(value?.toString().trim() ?? '');
  }

  QueueCover? _asCover(Object? value) {
    if (value is! Map) return null;
    final map = value.cast<Object?, Object?>();
    final kindRaw = _asText(map['kind'])?.toLowerCase();
    final v = _asText(map['value']);
    if (kindRaw == null || v == null) return null;
    final kind = switch (kindRaw) {
      'url' => QueueCoverKind.url,
      'file' => QueueCoverKind.file,
      'data' => QueueCoverKind.data,
      _ => null,
    };
    if (kind == null) return null;
    return QueueCover(kind: kind, value: v, mime: _asText(map['mime']));
  }

  Future<void> _fetchNeteaseDebugItems({required String action}) async {
    if (_neteaseDebugLoading) return;
    setState(() {
      _neteaseDebugLoading = true;
      _neteaseDebugError = null;
    });

    try {
      final sourceType = await _resolveNeteaseSourceType();
      if (sourceType == null) {
        throw Exception('未找到 type_id=netease 的 SourceCatalog');
      }

      final bridge = ref.read(playerBridgeProvider);
      final configJson = _sourceConfigForType(sourceType);
      final config = _decodeJsonObjectOrEmpty(configJson);
      final keywords = _neteaseKeywordsController.text.trim();
      final level = _neteaseLevelController.text.trim();
      final limit = (int.tryParse(_neteaseLimitController.text.trim()) ?? 30)
          .clamp(1, 200);

      final request = <String, Object?>{
        'action': action,
        'keywords': keywords,
        'limit': limit,
        'offset': 0,
      };
      if (level.isNotEmpty) {
        request['level'] = level;
      }
      if (action == 'playlist_tracks') {
        final playlistId =
            int.tryParse(_neteasePlaylistIdController.text.trim()) ?? 0;
        if (playlistId <= 0) {
          throw Exception('请输入有效的歌单 ID');
        }
        request['playlist_id'] = playlistId;
      }

      final raw = await bridge.sourceListItemsJson(
        pluginId: sourceType.pluginId,
        typeId: sourceType.typeId,
        configJson: jsonEncode(config),
        requestJson: jsonEncode(request),
      );

      final decoded = jsonDecode(raw);
      final List<QueueItem> items = <QueueItem>[];
      if (decoded is List) {
        for (final row in decoded) {
          if (row is! Map) continue;
          final map = row.cast<Object?, Object?>();
          final trackObj = map['track'];
          if (trackObj is! Map) continue;
          final track = trackObj.cast<String, Object?>();

          final sourceId = _asText(map['source_id']) ?? 'netease';
          final trackId =
              _asText(map['track_id']) ?? _asText(track['song_id']) ?? '';
          if (trackId.isEmpty) continue;

          final extHint = _asText(map['ext_hint']) ?? '';
          final pathHint = _asText(map['path_hint']) ?? '';
          final title = _asText(map['title']) ?? _asText(track['title']);
          final artist = _asText(map['artist']) ?? _asText(track['artist']);
          final album = _asText(map['album']) ?? _asText(track['album']);
          final durationMs =
              _asInt(map['duration_ms']) ?? _asInt(track['duration_ms']);
          final cover = _asCover(map['cover']) ?? _asCover(track['cover']);

          final trackRef = buildPluginSourceTrackRef(
            sourceId: sourceId,
            trackId: trackId,
            pluginId: sourceType.pluginId,
            typeId: sourceType.typeId,
            config: config,
            track: track,
            extHint: extHint,
            pathHint: pathHint,
            decoderPluginId: sourceType.pluginId,
            decoderTypeId: 'stream_symphonia',
          );

          items.add(
            QueueItem(
              track: trackRef,
              title: title,
              artist: artist,
              album: album,
              durationMs: durationMs,
              cover: cover,
            ),
          );
        }
      }

      setState(() {
        _neteaseDebugItems = items;
        _neteaseDebugError = null;
      });
    } catch (e, s) {
      logger.e(
        'failed to fetch netease debug source items',
        error: e,
        stackTrace: s,
      );
      setState(() {
        _neteaseDebugError = e.toString();
      });
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Netease source fetch failed: $e')),
      );
    } finally {
      if (mounted) {
        setState(() => _neteaseDebugLoading = false);
      }
    }
  }

  Future<void> _playNeteaseDebugIndex(int index) async {
    if (index < 0 || index >= _neteaseDebugItems.length) return;
    await ref
        .read(playbackControllerProvider.notifier)
        .setQueueAndPlayItems(_neteaseDebugItems, startIndex: index);
  }

  Future<void> _playNeteaseDebugAll() async {
    if (_neteaseDebugItems.isEmpty) return;
    await ref
        .read(playbackControllerProvider.notifier)
        .setQueueAndPlayItems(_neteaseDebugItems, startIndex: 0);
  }

  Future<void> _enqueueNeteaseDebugAll() async {
    if (_neteaseDebugItems.isEmpty) return;
    await ref
        .read(playbackControllerProvider.notifier)
        .enqueueItems(_neteaseDebugItems);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final bridge = ref.read(playerBridgeProvider);
    _pluginsFuture ??= _listLoadedPlugins(bridge);
    _sourceTypesFuture ??= _listSourceTypes(bridge);
    _outputSinkTypesFuture ??= bridge.outputSinkListTypes();
    _installedPluginsFuture ??= _listInstalledPlugins();

    final devices = ref.watch(audioDevicesProvider).value ?? const [];
    _persistOutputUiSession();

    final appBar = AppBar(title: Text(l10n.settingsTitle));

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
                  'Netease Source Debug',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _neteaseSidecarBaseUrlController,
                  decoration: const InputDecoration(
                    labelText: 'Sidecar base URL',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    FilledButton.icon(
                      onPressed: _neteaseAuthBusy ? null : _startNeteaseQrLogin,
                      icon: const Icon(Icons.qr_code_2),
                      label: const Text('Generate QR'),
                    ),
                    OutlinedButton.icon(
                      onPressed: _neteaseAuthBusy
                          ? null
                          : () => _pollNeteaseQrStatus(),
                      icon: const Icon(Icons.sync),
                      label: const Text('Check QR'),
                    ),
                    OutlinedButton(
                      onPressed: _neteaseAuthBusy
                          ? null
                          : () => _setNeteaseAuthAutoPolling(
                              !_neteaseAuthAutoPolling,
                            ),
                      child: Text(
                        _neteaseAuthAutoPolling
                            ? 'Stop Auto Poll'
                            : 'Start Auto Poll',
                      ),
                    ),
                    OutlinedButton(
                      onPressed: _neteaseAuthBusy
                          ? null
                          : _fetchNeteaseLoginStatus,
                      child: const Text('Login Status'),
                    ),
                    OutlinedButton(
                      onPressed: _neteaseAuthBusy ? null : _refreshNeteaseLogin,
                      child: const Text('Refresh Login'),
                    ),
                    OutlinedButton(
                      onPressed: _neteaseAuthBusy ? null : _logoutNeteaseLogin,
                      child: const Text('Logout'),
                    ),
                    if ((_neteaseQrUrl ?? '').trim().isNotEmpty)
                      OutlinedButton(
                        onPressed: () async {
                          final url = _neteaseQrUrl!.trim();
                          final uri = Uri.tryParse(url);
                          if (uri == null) return;
                          await launchUrl(uri);
                        },
                        child: const Text('Open QR URL'),
                      ),
                  ],
                ),
                if ((_neteaseAuthMessage ?? '').trim().isNotEmpty) ...[
                  const SizedBox(height: 8),
                  Text(
                    _neteaseAuthMessage!,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ],
                if (_neteaseLoginStatus != null) ...[
                  const SizedBox(height: 4),
                  Text(
                    jsonEncode(_neteaseLoginStatus),
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
                if (_neteaseAuthBusy) ...[
                  const SizedBox(height: 8),
                  const LinearProgressIndicator(),
                ],
                Builder(
                  builder: (context) {
                    final bytes = _decodeDataUrlBytes(_neteaseQrImageDataUrl);
                    if (bytes == null || bytes.isEmpty) {
                      return const SizedBox.shrink();
                    }
                    return Padding(
                      padding: const EdgeInsets.only(top: 8),
                      child: Align(
                        alignment: Alignment.centerLeft,
                        child: ClipRRect(
                          borderRadius: BorderRadius.circular(8),
                          child: Image.memory(
                            bytes,
                            width: 180,
                            height: 180,
                            fit: BoxFit.contain,
                          ),
                        ),
                      ),
                    );
                  },
                ),
                const SizedBox(height: 10),
                TextField(
                  controller: _neteaseKeywordsController,
                  decoration: const InputDecoration(
                    labelText: 'Search keywords',
                    border: OutlineInputBorder(),
                  ),
                  onSubmitted: (_) =>
                      unawaited(_fetchNeteaseDebugItems(action: 'search')),
                ),
                const SizedBox(height: 8),
                Row(
                  children: [
                    Expanded(
                      child: TextField(
                        controller: _neteasePlaylistIdController,
                        decoration: const InputDecoration(
                          labelText: 'Playlist ID',
                          border: OutlineInputBorder(),
                        ),
                        keyboardType: TextInputType.number,
                      ),
                    ),
                    const SizedBox(width: 8),
                    SizedBox(
                      width: 120,
                      child: TextField(
                        controller: _neteaseLevelController,
                        decoration: const InputDecoration(
                          labelText: 'Level',
                          border: OutlineInputBorder(),
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    SizedBox(
                      width: 88,
                      child: TextField(
                        controller: _neteaseLimitController,
                        decoration: const InputDecoration(
                          labelText: 'Limit',
                          border: OutlineInputBorder(),
                        ),
                        keyboardType: TextInputType.number,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Row(
                  children: [
                    FilledButton.icon(
                      onPressed: _neteaseDebugLoading
                          ? null
                          : () => _fetchNeteaseDebugItems(action: 'search'),
                      icon: const Icon(Icons.search),
                      label: const Text('Search'),
                    ),
                    const SizedBox(width: 8),
                    OutlinedButton.icon(
                      onPressed: _neteaseDebugLoading
                          ? null
                          : () => _fetchNeteaseDebugItems(
                              action: 'playlist_tracks',
                            ),
                      icon: const Icon(Icons.queue_music_outlined),
                      label: const Text('Load Playlist'),
                    ),
                    const SizedBox(width: 8),
                    OutlinedButton(
                      onPressed: _neteaseDebugItems.isEmpty
                          ? null
                          : _playNeteaseDebugAll,
                      child: const Text('Play All'),
                    ),
                    const SizedBox(width: 8),
                    OutlinedButton(
                      onPressed: _neteaseDebugItems.isEmpty
                          ? null
                          : _enqueueNeteaseDebugAll,
                      child: const Text('Enqueue All'),
                    ),
                  ],
                ),
                if (_neteaseDebugLoading) ...[
                  const SizedBox(height: 8),
                  const LinearProgressIndicator(),
                ],
                if (_neteaseDebugError != null &&
                    _neteaseDebugError!.trim().isNotEmpty) ...[
                  const SizedBox(height: 8),
                  Text(
                    _neteaseDebugError!,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                ],
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
                  child: _neteaseDebugItems.isEmpty
                      ? const Center(
                          child: Text(
                            'No source items. Use Search or Load Playlist.',
                          ),
                        )
                      : ListView.separated(
                          padding: const EdgeInsets.all(8),
                          itemBuilder: (context, index) {
                            final item = _neteaseDebugItems[index];
                            final subtitleParts = <String>[
                              if ((item.artist ?? '').trim().isNotEmpty)
                                item.artist!.trim(),
                              if ((item.album ?? '').trim().isNotEmpty)
                                item.album!.trim(),
                            ];
                            return ListTile(
                              dense: true,
                              contentPadding: EdgeInsets.zero,
                              title: Text(
                                item.displayTitle,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                              subtitle: subtitleParts.isEmpty
                                  ? null
                                  : Text(
                                      subtitleParts.join(' - '),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                              trailing: Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  IconButton(
                                    tooltip: 'Play',
                                    onPressed: () =>
                                        _playNeteaseDebugIndex(index),
                                    icon: const Icon(Icons.play_arrow),
                                  ),
                                  IconButton(
                                    tooltip: 'Enqueue',
                                    onPressed: () => ref
                                        .read(
                                          playbackControllerProvider.notifier,
                                        )
                                        .enqueueItems([item]),
                                    icon: const Icon(Icons.queue_music),
                                  ),
                                ],
                              ),
                            );
                          },
                          separatorBuilder: (_, _) => const Divider(height: 1),
                          itemCount: _neteaseDebugItems.length,
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
                        } catch (e, s) {
                          logger.e(
                            'failed to apply backend',
                            error: e,
                            stackTrace: s,
                          );
                          messenger.showSnackBar(
                            SnackBar(content: Text('Apply backend failed: $e')),
                          );
                        } finally {
                          try {
                            final refreshedDevices = await ref.refresh(
                              audioDevicesProvider.future,
                            );
                            logger.d(
                              'refreshed devices after backend change: ${refreshedDevices.length}',
                            );
                          } catch (e, s) {
                            logger.w(
                              'failed to refresh devices after backend change',
                              error: e,
                              stackTrace: s,
                            );
                          }
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
                      } catch (e, s) {
                        logger.e(
                          'failed to set output device',
                          error: e,
                          stackTrace: s,
                        );
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
                Row(
                  children: [
                    Text(
                      l10n.settingsPluginsTitle,
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    const Spacer(),
                    IconButton(
                      visualDensity: VisualDensity.compact,
                      tooltip: l10n.settingsInstallPlugin,
                      onPressed: _installPluginArtifact,
                      icon: const Icon(Icons.add),
                    ),
                    IconButton(
                      visualDensity: VisualDensity.compact,
                      tooltip: l10n.refresh,
                      onPressed: () => setState(_refresh),
                      icon: const Icon(Icons.refresh),
                    ),
                    FutureBuilder<String>(
                      future: defaultPluginDir(),
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

                    return FutureBuilder<Set<String>>(
                      future: _disabledPluginIdsFuture,
                      builder: (context, disabledSnap) {
                        final disabledData = disabledSnap.data;
                        if (disabledData != null) {
                          _cachedDisabledPluginIds = disabledData;
                          _cachedDisabledPluginIdsReady = true;
                        } else if (disabledSnap.connectionState ==
                            ConnectionState.done) {
                          _cachedDisabledPluginIds = <String>{};
                          _cachedDisabledPluginIdsReady = true;
                        }
                        final disabled = _cachedDisabledPluginIdsReady
                            ? _cachedDisabledPluginIds
                            : <String>{};

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

                            return FutureBuilder<
                              List<SourceCatalogTypeDescriptor>
                            >(
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
                                        : const <
                                            SourceCatalogTypeDescriptor
                                          >[]);
                                final sourceByPlugin =
                                    <
                                      String,
                                      List<SourceCatalogTypeDescriptor>
                                    >{};
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
                                      _PluginTile(
                                        plugin: p,
                                        isDisabled: p.id != null
                                            ? disabled.contains(p.id)
                                            : false,
                                        isLoaded: p.id != null
                                            ? loadedIds.contains(p.id)
                                            : false,
                                        loadedKnown: loadedKnown,
                                        pluginSourceTypes: p.id == null
                                            ? const []
                                            : (sourceByPlugin[p.id] ??
                                                  const []),
                                        pluginOutputSinkTypes: p.id == null
                                            ? const []
                                            : (outputByPlugin[p.id] ??
                                                  const []),
                                        onToggleEnabled: (v) =>
                                            _setPluginEnabled(
                                              plugin: p,
                                              enabled: v,
                                            ),
                                        onUninstall: () => _uninstallPlugin(p),
                                        sourceConfigForType:
                                            _sourceConfigForType,
                                        outputSinkConfigForType:
                                            _outputSinkConfigForType,
                                        onSourceConfigChanged: (t, json) =>
                                            _sourceConfigDrafts[_sourceTypeKey(
                                                  t,
                                                )] =
                                                json,
                                        onOutputSinkConfigChanged: (t, json) {
                                          final key = _outputSinkTypeKey(t);
                                          _outputSinkConfigDrafts[key] = json;
                                          if (_selectedOutputSinkTypeKey ==
                                              key) {
                                            _outputSinkConfigController.text =
                                                json;
                                            _outputSinkConfigApplyDebounce
                                                ?.cancel();
                                            _outputSinkConfigApplyDebounce = Timer(
                                              const Duration(milliseconds: 350),
                                              () async {
                                                if (!mounted) return;
                                                try {
                                                  await _loadOutputSinkTargets();
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
                                        },
                                        onSaveSourceConfig: _saveSourceConfig,
                                      ),
                                  ],
                                );
                              },
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
                              _formatRuntimeEventLine(e),
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

class _PluginTile extends StatefulWidget {
  final _InstalledPlugin plugin;
  final bool isDisabled;
  final bool isLoaded;
  final bool loadedKnown;
  final List<SourceCatalogTypeDescriptor> pluginSourceTypes;
  final List<OutputSinkTypeDescriptor> pluginOutputSinkTypes;
  final Future<void> Function(bool) onToggleEnabled;
  final Future<void> Function() onUninstall;
  final String? Function(SourceCatalogTypeDescriptor) sourceConfigForType;
  final String? Function(OutputSinkTypeDescriptor) outputSinkConfigForType;
  final void Function(SourceCatalogTypeDescriptor, String)
  onSourceConfigChanged;
  final void Function(OutputSinkTypeDescriptor, String)
  onOutputSinkConfigChanged;
  final Future<void> Function(SourceCatalogTypeDescriptor) onSaveSourceConfig;

  const _PluginTile({
    required this.plugin,
    required this.isDisabled,
    required this.isLoaded,
    required this.loadedKnown,
    required this.pluginSourceTypes,
    required this.pluginOutputSinkTypes,
    required this.onToggleEnabled,
    required this.onUninstall,
    required this.sourceConfigForType,
    required this.outputSinkConfigForType,
    required this.onSourceConfigChanged,
    required this.onOutputSinkConfigChanged,
    required this.onSaveSourceConfig,
  });

  @override
  State<_PluginTile> createState() => _PluginTileState();
}

class _PluginTileState extends State<_PluginTile> {
  String _uninstallErrorMessage(
    AppLocalizations l10n,
    _InstalledPlugin plugin,
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
