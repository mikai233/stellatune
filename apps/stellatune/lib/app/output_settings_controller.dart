import 'dart:async';

import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/output_settings_runtime.dart';
import 'package:stellatune/app/output_settings_values.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';

class OutputSettingsDraft {
  const OutputSettingsDraft({
    required this.backendKey,
    this.deviceId,
    this.configJson = '{}',
    this.targetJson,
  });

  factory OutputSettingsDraft.fromSettings(SettingsState settings) {
    final route = settings.outputSinkRoute;
    return OutputSettingsDraft(
      backendKey: route == null
          ? OutputSettingsValues.localBackendKey(settings.selectedBackend)
          : OutputSettingsValues.pluginBackendKey(route.pluginId, route.typeId),
      deviceId: settings.selectedDeviceId,
      configJson: route?.configJson ?? '{}',
      targetJson: route?.targetJson,
    );
  }

  final String backendKey;
  final String? deviceId;
  final String configJson;
  final String? targetJson;
  AudioBackend? get localBackend =>
      OutputSettingsValues.parseLocalBackendKey(backendKey);

  OutputSinkRoute? get route {
    if (localBackend != null) return null;
    final key = OutputSettingsValues.parsePluginTypeKey(backendKey);
    if (key == null || targetJson == null) {
      throw StateError('当前后端没有可用的输出设备。');
    }
    final parts = key.split('::');
    return OutputSinkRoute(
      pluginId: parts[0],
      typeId: parts[1],
      configJson: configJson,
      targetJson: targetJson!,
    );
  }

  OutputSettingsDraft withTarget(String? target) => OutputSettingsDraft(
    backendKey: backendKey,
    deviceId: localBackend == null ? deviceId : target,
    configJson: configJson,
    targetJson: localBackend == null ? target : null,
  );
}

class OutputSettingsState {
  const OutputSettingsState({
    this.draft,
    this.types = const [],
    this.devices = const [],
    this.targets = const [],
    this.loading = false,
    this.loadingTargets = false,
    this.applying = false,
    this.error,
    this.configDrafts = const {},
    this.unavailableTargets = const {},
    this.revision = 0,
  });

  // Confirmed values live only in settingsStoreProvider.
  final OutputSettingsDraft? draft;
  final List<OutputSinkTypeDescriptor> types;
  final List<AudioDevice> devices;
  final List<Object?> targets;
  final Map<String, String> configDrafts;
  // Session-only failures; refreshing devices permits a new attempt.
  final Map<(String, String), Object> unavailableTargets;
  final bool loading;
  final bool loadingTargets;
  final bool applying;
  final Object? error;
  final int revision;

  OutputSettingsState copyWith({
    OutputSettingsDraft? draft,
    bool clearDraft = false,
    List<OutputSinkTypeDescriptor>? types,
    List<AudioDevice>? devices,
    List<Object?>? targets,
    Map<String, String>? configDrafts,
    Map<(String, String), Object>? unavailableTargets,
    bool? loading,
    bool? loadingTargets,
    bool? applying,
    Object? error,
    bool clearError = false,
  }) => OutputSettingsState(
    draft: clearDraft ? null : draft ?? this.draft,
    types: types ?? this.types,
    devices: devices ?? this.devices,
    targets: targets ?? this.targets,
    configDrafts: configDrafts ?? this.configDrafts,
    unavailableTargets: unavailableTargets ?? this.unavailableTargets,
    loading: loading ?? this.loading,
    loadingTargets: loadingTargets ?? this.loadingTargets,
    applying: applying ?? this.applying,
    error: clearError ? null : error ?? this.error,
    revision: revision + 1,
  );
}

final outputSettingsControllerProvider =
    NotifierProvider<OutputSettingsController, OutputSettingsState>(
      OutputSettingsController.new,
    );

/// Owns device requests and serialized runtime changes independently of a page.
class OutputSettingsController extends Notifier<OutputSettingsState> {
  int _selectionGeneration = 0;
  int _refreshGeneration = 0;
  int _lifetime = 0;
  Timer? _configDebounce;
  Future<void> _pending = Future.value();

  PlayerBridge get _bridge => ref.read(playerBridgeProvider);
  SettingsState get _confirmed => ref.read(settingsStoreProvider);
  SettingsController get _settings => ref.read(settingsStoreProvider.notifier);
  OutputSettingsDraft get selection =>
      state.draft ?? OutputSettingsDraft.fromSettings(_confirmed);

  @override
  OutputSettingsState build() {
    final lifetime = ++_lifetime;
    ++_selectionGeneration;
    ++_refreshGeneration;
    ref.onDispose(() {
      if (lifetime != _lifetime) return;
      ++_lifetime;
      ++_selectionGeneration;
      ++_refreshGeneration;
      _configDebounce?.cancel();
    });
    return const OutputSettingsState();
  }

  bool _isActive(int lifetime) => ref.mounted && lifetime == _lifetime;

  bool _isCurrent(int generation) =>
      ref.mounted && generation == _selectionGeneration;

  Future<void> _serialize(Future<void> Function() action) {
    final lifetime = _lifetime;
    final next = _pending.then((_) async {
      if (!_isActive(lifetime)) return;
      state = state.copyWith(applying: true, clearError: true);
      DiagnosticsService.instance.beginOutputOperation();
      try {
        await action();
      } finally {
        DiagnosticsService.instance.endOutputOperation();
        if (_isActive(lifetime)) state = state.copyWith(applying: false);
      }
    });
    _pending = next.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return next;
  }

  void _recordError(
    Object error,
    StackTrace stack, {
    int? generation,
    int? lifetime,
    bool preservePluginSelection = false,
  }) {
    logger.w(
      'output settings operation failed',
      error: error,
      stackTrace: stack,
    );
    if (!ref.mounted ||
        (lifetime != null && !_isActive(lifetime)) ||
        (generation != null && !_isCurrent(generation))) {
      return;
    }
    state = state.copyWith(
      error: error,
      clearDraft: !preservePluginSelection,
      loadingTargets: false,
      targets: preservePluginSelection ? state.targets : const [],
    );
    DiagnosticsService.instance.report(
      error,
      stack: stack,
      operation: 'output_settings',
    );
  }

  Future<void> refresh() async {
    final lifetime = _lifetime;
    final generation = ++_refreshGeneration;
    final selectionGeneration = _selectionGeneration;
    state = state.copyWith(
      loading: true,
      clearError: true,
      unavailableTargets: const {},
    );
    try {
      final results = await Future.wait<Object>([
        _bridge.outputSinkListTypes(),
        _bridge.refreshDevices(),
      ]);
      if (!_isActive(lifetime) || generation != _refreshGeneration) return;
      state = state.copyWith(
        types: results[0] as List<OutputSinkTypeDescriptor>,
        devices: results[1] as List<AudioDevice>,
        loading: false,
      );
      // Enumeration only populates choices; opening/reopening a page never applies a route.
      if (selectionGeneration == _selectionGeneration) {
        await refreshTargets();
      }
    } catch (error, stack) {
      if (!_isActive(lifetime) || generation != _refreshGeneration) return;
      state = state.copyWith(loading: false);
      _recordError(error, stack, generation: selectionGeneration);
    }
  }

  String configForType(OutputSinkTypeDescriptor type) {
    final key = OutputSettingsValues.outputSinkTypeKey(type);
    final route = _confirmed.outputSinkRoute;
    return state.configDrafts[key] ??
        (route != null && key == '${route.pluginId}::${route.typeId}'
            ? route.configJson
            : type.defaultConfigJson);
  }

  Future<void> selectBackend(String key) async {
    final generation = ++_selectionGeneration;
    _configDebounce?.cancel();
    final local = OutputSettingsValues.parseLocalBackendKey(key);
    final type = state.types
        .where(
          (t) =>
              OutputSettingsValues.pluginBackendKey(t.pluginId, t.typeId) ==
              key,
        )
        .firstOrNull;
    if (local == null && type == null) return;
    final draft = OutputSettingsDraft(
      backendKey: key,
      configJson: type == null ? '{}' : configForType(type),
    );
    state = state.copyWith(
      draft: draft,
      targets: const [],
      clearError: true,
      loadingTargets: local == null,
    );
    try {
      var resolved = draft;
      if (type != null) {
        final targets = await _loadTargets(draft);
        if (!_isCurrent(generation)) return;
        state = state.copyWith(targets: targets, loadingTargets: false);
        final available = targets.where(
          (target) => !state.unavailableTargets.containsKey((
            key,
            OutputSettingsValues.targetValueOf(target),
          )),
        );
        resolved = draft.withTarget(
          available.isEmpty
              ? null
              : OutputSettingsValues.targetValueOf(available.first),
        );
        state = state.copyWith(draft: resolved);
        if (available.isEmpty) return;
      }
      if (!_isCurrent(generation)) return;
      await _commitSelection(resolved, generation);
    } catch (error, stack) {
      // A failing first driver must not remove the device picker: the user
      // still needs to select another driver without changing the active route.
      _recordError(
        error,
        stack,
        generation: generation,
        preservePluginSelection: type != null,
      );
    }
  }

  Future<List<Object?>> _loadTargets(OutputSettingsDraft draft) async {
    final key = OutputSettingsValues.parsePluginTypeKey(draft.backendKey)!;
    final parts = key.split('::');
    return OutputSettingsValues.parseOutputSinkTargetsJson(
      await _bridge.outputSinkListTargetsJson(
        pluginId: parts[0],
        typeId: parts[1],
        configJson: draft.configJson,
      ),
    );
  }

  Future<void> refreshTargets() async {
    final current = selection;
    if (current.localBackend != null) return;
    final generation = ++_selectionGeneration;
    state = state.copyWith(loadingTargets: true);
    try {
      final targets = await _loadTargets(current);
      if (!_isCurrent(generation)) return;
      state = state.copyWith(targets: targets, loadingTargets: false);
    } catch (error, stack) {
      _recordError(error, stack, generation: generation);
    }
  }

  Future<void> selectDevice(String? value) async {
    if (value != null &&
        state.unavailableTargets.containsKey((selection.backendKey, value))) {
      return;
    }
    final generation = ++_selectionGeneration;
    final draft = selection.withTarget(value);
    state = state.copyWith(draft: draft, clearError: true);
    try {
      await _commitSelection(draft, generation);
    } catch (error, stack) {
      _recordError(
        error,
        stack,
        generation: generation,
        preservePluginSelection: draft.localBackend == null,
      );
    }
  }

  Future<void> _commitSelection(OutputSettingsDraft draft, int generation) =>
      _serialize(() async {
        if (!_isCurrent(generation)) return;
        final previous = _confirmed;
        final bridge = _bridge;
        final settings = _settings;
        final route = draft.route;
        final backend = draft.localBackend ?? previous.selectedBackend;
        final deviceId = draft.localBackend == null
            ? previous.selectedDeviceId
            : draft.deviceId;
        try {
          await applyOutputSelection(
            bridge,
            backend: backend,
            deviceId: deviceId,
            route: route,
          );
        } catch (error) {
          final target = draft.localBackend == null
              ? draft.targetJson
              : draft.deviceId;
          if (_isCurrent(generation) && target != null) {
            state = state.copyWith(
              unavailableTargets: {
                ...state.unavailableTargets,
                (draft.backendKey, target): error,
              },
            );
          }
          rethrow;
        }
        try {
          // Persist even if a newer selection is waiting: this route really was applied.
          await settings.saveOutputSelection(
            backend: backend,
            deviceId: deviceId,
            route: route,
          );
        } catch (_) {
          await applyOutputSelection(
            bridge,
            backend: previous.selectedBackend,
            deviceId: previous.selectedDeviceId,
            route: previous.outputSinkRoute,
          );
          rethrow;
        }
        if (_isCurrent(generation)) state = state.copyWith(clearDraft: true);
      });

  Future<void> setOptions({
    bool? matchTrackSampleRate,
    bool? gaplessPlayback,
    bool? seekTrackFade,
    ResampleQuality? resampleQuality,
  }) async {
    final lifetime = _lifetime;
    try {
      await _serialize(() async {
        final previous = _confirmed;
        final bridge = _bridge;
        final settings = _settings;
        final next = previous.copyWith(
          matchTrackSampleRate: matchTrackSampleRate,
          gaplessPlayback: gaplessPlayback,
          seekTrackFade: seekTrackFade,
          resampleQuality: resampleQuality,
        );
        await applyOutputOptions(bridge, next);
        try {
          await settings.saveOutputOptions(next);
        } catch (_) {
          await applyOutputOptions(bridge, previous);
          rethrow;
        }
      });
    } catch (error, stack) {
      _recordError(error, stack, lifetime: lifetime);
    }
  }

  Future<void> setLatency(PlaybackLatency value) async {
    final lifetime = _lifetime;
    try {
      await _serialize(() async {
        final previous = _confirmed.playbackLatency;
        final bridge = _bridge;
        final settings = _settings;
        await bridge.setPlaybackLatency(value);
        try {
          await settings.setPlaybackLatency(value);
        } catch (_) {
          await bridge.setPlaybackLatency(previous);
          rethrow;
        }
      });
    } catch (error, stack) {
      _recordError(error, stack, lifetime: lifetime);
    }
  }

  void updateConfig(OutputSinkTypeDescriptor type, String json) {
    final key = OutputSettingsValues.outputSinkTypeKey(type);
    state = state.copyWith(
      configDrafts: {...state.configDrafts, key: json},
      unavailableTargets: Map.of(state.unavailableTargets)
        ..removeWhere(
          (target, _) =>
              target.$1 ==
              OutputSettingsValues.pluginBackendKey(type.pluginId, type.typeId),
        ),
    );
    _configDebounce?.cancel();
    if (OutputSettingsValues.parsePluginTypeKey(selection.backendKey) != key) {
      return;
    }
    final generation = ++_selectionGeneration;
    final current = selection;
    final draft = OutputSettingsDraft(
      backendKey: current.backendKey,
      configJson: json.trim().isEmpty ? '{}' : json,
      targetJson: current.targetJson,
    );
    _configDebounce = Timer(const Duration(milliseconds: 350), () async {
      if (!_isCurrent(generation)) return;
      state = state.copyWith(draft: draft);
      try {
        await _commitSelection(draft, generation);
      } catch (error, stack) {
        _recordError(error, stack, generation: generation);
      }
    });
  }

  /// Package mutations and output changes share the same application-side queue.
  Future<void> changePlugins(Future<void> Function() change) async {
    ++_selectionGeneration;
    _configDebounce?.cancel();
    state = state.copyWith(
      clearDraft: true,
      loadingTargets: false,
      unavailableTargets: const {},
    );
    await _serialize(() async {
      final bridge = _bridge;
      final settings = _settings;
      final hadPluginRoute = _confirmed.outputSinkRoute != null;
      // The Rust plugin manager releases native hosts before package mutation.
      // Reflect its local fallback even if installation subsequently fails.
      try {
        await change();
      } finally {
        if (hadPluginRoute) {
          await bridge.clearOutputSinkRoute();
          await settings.clearOutputSinkRoute();
        }
      }
    });
  }
}
