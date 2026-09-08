import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/output_settings_controller.dart';
import 'package:stellatune/app/plugin_paths.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/bridge/api/player.dart' as player_api;
import 'package:stellatune/player/decoder_extension_support.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/plugins/installed_plugin.dart';
import 'package:stellatune/plugins/plugin_runtime_service.dart';

class PluginSettingsState {
  const PluginSettingsState({
    this.plugins = const [],
    this.disabledIds = const {},
    this.loadedIds = const {},
    this.sourceTypes = const [],
    this.directory,
    this.loading = false,
    this.busy = false,
    this.loaded = false,
    this.error,
  });

  final List<InstalledPlugin> plugins;
  final Set<String> disabledIds;
  final Set<String> loadedIds;
  final List<SourceCatalogTypeDescriptor> sourceTypes;
  final String? directory;
  final bool loading;
  final bool busy;
  final bool loaded;
  final Object? error;

  PluginSettingsState copyWith({
    List<InstalledPlugin>? plugins,
    Set<String>? disabledIds,
    Set<String>? loadedIds,
    List<SourceCatalogTypeDescriptor>? sourceTypes,
    String? directory,
    bool? loading,
    bool? busy,
    bool? loaded,
    Object? error,
    bool clearError = false,
  }) => PluginSettingsState(
    plugins: plugins ?? this.plugins,
    disabledIds: disabledIds ?? this.disabledIds,
    loadedIds: loadedIds ?? this.loadedIds,
    sourceTypes: sourceTypes ?? this.sourceTypes,
    directory: directory ?? this.directory,
    loading: loading ?? this.loading,
    busy: busy ?? this.busy,
    loaded: loaded ?? this.loaded,
    error: clearError ? null : error ?? this.error,
  );
}

final pluginSettingsControllerProvider =
    NotifierProvider<PluginSettingsController, PluginSettingsState>(
      PluginSettingsController.new,
    );

class PluginSettingsController extends Notifier<PluginSettingsState> {
  PluginSettingsController({Future<String> Function()? resolveDirectory})
    : _resolveDirectory = resolveDirectory ?? defaultPluginDir;

  final _runtime = const PluginRuntimeService();
  final Future<String> Function() _resolveDirectory;
  int _refreshGeneration = 0;
  int _lifetime = 0;
  Future<String>? _directory;

  @override
  PluginSettingsState build() {
    final lifetime = ++_lifetime;
    ++_refreshGeneration;
    ref.onDispose(() {
      if (lifetime != _lifetime) return;
      ++_lifetime;
      ++_refreshGeneration;
    });
    return const PluginSettingsState();
  }

  bool _isActive(int lifetime) => ref.mounted && lifetime == _lifetime;

  Future<String> _ensureDirectory() async {
    final pending = _directory ??= _resolveDirectory();
    try {
      return await pending;
    } catch (_) {
      if (identical(_directory, pending)) _directory = null;
      rethrow;
    }
  }

  Future<String> openUi(String pluginId) =>
      player_api.pluginOpenUi(pluginId: pluginId);

  Future<void> refresh() async {
    final lifetime = _lifetime;
    final generation = ++_refreshGeneration;
    state = state.copyWith(loading: true, clearError: true);
    try {
      final bridge = ref.read(playerBridgeProvider);
      final library = ref.read(libraryBridgeProvider);
      final directory = await _ensureDirectory();
      if (!_isActive(lifetime) || generation != _refreshGeneration) return;
      final results = await Future.wait<Object>([
        _runtime.listInstalledPlugins(bridge: bridge, pluginDir: directory),
        _runtime.listDisabledPluginIds(library),
        _runtime.listLoadedPlugins(bridge),
        _runtime.listSourceTypes(bridge),
      ]);
      if (!_isActive(lifetime) || generation != _refreshGeneration) return;
      state = state.copyWith(
        plugins: results[0] as List<InstalledPlugin>,
        disabledIds: results[1] as Set<String>,
        loadedIds: (results[2] as List<PluginDescriptor>)
            .map((p) => p.id)
            .toSet(),
        sourceTypes: results[3] as List<SourceCatalogTypeDescriptor>,
        directory: directory,
        loading: false,
        loaded: true,
      );
    } catch (error, stack) {
      logger.w(
        'failed to refresh plugin settings',
        error: error,
        stackTrace: stack,
      );
      if (!_isActive(lifetime) || generation != _refreshGeneration) return;
      state = state.copyWith(loading: false, error: error);
    }
  }

  Future<void> _mutate(Future<void> Function() action) async {
    final lifetime = _lifetime;
    if (state.busy) {
      throw StateError('A plugin operation is already in progress.');
    }
    state = state.copyWith(busy: true, clearError: true);
    final output = ref.read(outputSettingsControllerProvider.notifier);
    final bridge = ref.read(playerBridgeProvider);
    try {
      await output.changePlugins(action);
      DecoderExtensionSupportCache.instance.invalidate();
      if (!_isActive(lifetime)) return;
      try {
        await DecoderExtensionSupportCache.instance.refresh(bridge);
      } catch (error, stack) {
        logger.w(
          'failed to refresh decoder extensions after plugin change',
          error: error,
          stackTrace: stack,
        );
      }
    } catch (error, stack) {
      logger.w(
        'plugin settings operation failed',
        error: error,
        stackTrace: stack,
      );
      if (_isActive(lifetime)) state = state.copyWith(error: error);
      rethrow;
    } finally {
      if (_isActive(lifetime)) {
        state = state.copyWith(busy: false);
        await Future.wait([refresh(), output.refresh()]);
      }
    }
  }

  Future<void> install(String artifactPath) {
    final bridge = ref.read(playerBridgeProvider);
    return _mutate(() async {
      await bridge.pluginsInstallFromFile(
        dir: await _ensureDirectory(),
        artifactPath: artifactPath,
      );
    });
  }

  Future<void> uninstall(InstalledPlugin plugin) {
    final bridge = ref.read(playerBridgeProvider);
    final library = ref.read(libraryBridgeProvider);
    return _mutate(() async {
      final id = plugin.id?.trim();
      if (id != null && id.isNotEmpty) {
        await bridge.pluginsUninstallById(
          dir: await _ensureDirectory(),
          pluginId: id,
        );
      } else {
        await Directory(plugin.dirPath).delete(recursive: true);
        await library.pluginApplyState();
      }
    });
  }

  Future<void> setEnabled({
    required InstalledPlugin plugin,
    required bool enabled,
  }) {
    final library = ref.read(libraryBridgeProvider);
    final playback = enabled
        ? null
        : ref.read(playbackControllerProvider.notifier);
    return _mutate(() async {
      final id = plugin.id?.trim();
      if (id == null || id.isEmpty) throw StateError('Missing plugin ID');
      if (enabled) {
        await library.pluginEnable(pluginId: id);
      } else {
        await library.pluginDisable(pluginId: id);
      }
      await library.pluginApplyState();
      if (playback != null) {
        DecoderExtensionSupportCache.instance.invalidate();
        await playback.removeUnplayableQueuedItemsDueToDisabledPlugins(
          pluginId: id,
        );
      }
    });
  }
}
