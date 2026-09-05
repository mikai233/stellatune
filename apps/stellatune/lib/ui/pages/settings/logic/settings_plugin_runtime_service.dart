import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as p;
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/ui/pages/settings/models/installed_plugin.dart';

class SettingsPluginRuntimeService {
  const SettingsPluginRuntimeService({
    this.bridgeQueryTimeout = const Duration(seconds: 8),
  });

  final Duration bridgeQueryTimeout;

  Future<List<PluginDescriptor>> listLoadedPlugins(PlayerBridge bridge) async {
    try {
      return await bridge.pluginsList().timeout(bridgeQueryTimeout);
    } on TimeoutException catch (e, s) {
      logger.w('pluginsList timed out', error: e, stackTrace: s);
      return const <PluginDescriptor>[];
    } catch (e, s) {
      logger.w('pluginsList failed', error: e, stackTrace: s);
      return const <PluginDescriptor>[];
    }
  }

  Future<List<SourceCatalogTypeDescriptor>> listSourceTypes(
    PlayerBridge bridge,
  ) async {
    try {
      return await bridge.sourceListTypes().timeout(bridgeQueryTimeout);
    } on TimeoutException catch (e, s) {
      logger.w('sourceListTypes timed out', error: e, stackTrace: s);
      return const <SourceCatalogTypeDescriptor>[];
    } catch (e, s) {
      logger.w('sourceListTypes failed', error: e, stackTrace: s);
      return const <SourceCatalogTypeDescriptor>[];
    }
  }

  Future<List<InstalledPlugin>> listInstalledPlugins({
    required PlayerBridge bridge,
    required String pluginDir,
  }) async {
    final raw = await bridge.pluginsListInstalledJson(dir: pluginDir);
    final decoded = jsonDecode(raw);
    if (decoded is! List) return const [];
    final out = <InstalledPlugin>[];
    for (final item in decoded) {
      if (item is! Map) continue;
      final map = item.cast<Object?, Object?>();
      final id = (map['id'] ?? '').toString().trim();
      if (id.isEmpty) continue;
      final dirPath = (map['root_dir'] ?? '').toString().trim();
      final resolvedDirPath = dirPath.isEmpty ? p.join(pluginDir, id) : dirPath;
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
      final hasWebUi = await _pluginHasWebUi(resolvedDirPath);
      out.add(
        InstalledPlugin(
          dirPath: resolvedDirPath,
          id: id,
          name: nameRaw.isEmpty ? null : nameRaw,
          hasWebUi: hasWebUi,
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

  Future<Set<String>> listDisabledPluginIds(LibraryBridge library) async {
    final ids = await library.listDisabledPluginIds();
    return ids.map((id) => id.trim()).where((id) => id.isNotEmpty).toSet();
  }

  Future<bool> _pluginHasWebUi(String pluginDirPath) async {
    final root = pluginDirPath.trim();
    if (root.isEmpty) return false;
    try {
      final manifestPath = p.join(root, 'manifest.json');
      final manifestFile = File(manifestPath);
      if (!await manifestFile.exists()) return false;
      final raw = await manifestFile.readAsString();
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return false;
      final manifest = decoded.cast<Object?, Object?>();
      final uiRaw = manifest['ui'];
      if (uiRaw is! Map) return false;
      final ui = uiRaw.cast<Object?, Object?>();
      return ui['mode'] == 'plugin-hosted';
    } catch (e, s) {
      logger.d(
        'failed to detect plugin web ui plugin_dir=$pluginDirPath',
        error: e,
        stackTrace: s,
      );
      return false;
    }
  }
}
