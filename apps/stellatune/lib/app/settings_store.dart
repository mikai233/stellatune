import 'dart:convert';

import 'package:hive_flutter/hive_flutter.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/bridge/bridge.dart';

class SettingsStore {
  SettingsStore._(this._box);

  static const _boxName = 'settings';
  static const _keyVolume = 'volume';
  static const _keyPlayMode = 'play_mode';
  static const _keyResumePath = 'resume_path';
  static const _keyResumePositionMs = 'resume_position_ms';
  static const _keyDspChain = 'dsp_chain';
  static const _keyDisabledPlugins = 'disabled_plugins';

  final Box _box;

  static Future<SettingsStore> open() async {
    await Hive.initFlutter();
    final box = await Hive.openBox(_boxName);
    return SettingsStore._(box);
  }

  double get volume {
    final v = _box.get(_keyVolume, defaultValue: 1.0);
    if (v is num) return v.toDouble();
    return 1.0;
  }

  Future<void> setVolume(double v) => _box.put(_keyVolume, v);

  PlayMode get playMode {
    final raw = _box.get(_keyPlayMode);
    if (raw is String) {
      for (final m in PlayMode.values) {
        if (m.name == raw) return m;
      }
    }
    return PlayMode.sequential;
  }

  Future<void> setPlayMode(PlayMode mode) => _box.put(_keyPlayMode, mode.name);

  String? get resumePath {
    final v = _box.get(_keyResumePath);
    if (v is String && v.trim().isNotEmpty) return v;
    return null;
  }

  int get resumePositionMs {
    final v = _box.get(_keyResumePositionMs, defaultValue: 0);
    if (v is int) return v;
    if (v is num) return v.toInt();
    return 0;
  }

  Future<void> setResume({
    required String path,
    required int positionMs,
  }) async {
    await _box.put(_keyResumePath, path);
    await _box.put(_keyResumePositionMs, positionMs);
  }

  Future<void> clearResume() async {
    await _box.delete(_keyResumePath);
    await _box.delete(_keyResumePositionMs);
  }

  List<DspChainItem> get dspChain {
    final raw = _box.get(_keyDspChain, defaultValue: '[]');
    final text = raw is String ? raw : '[]';
    try {
      final decoded = jsonDecode(text);
      if (decoded is! List) return const [];
      return decoded
          .whereType<Map>()
          .map((m) => m.cast<String, dynamic>())
          .map(
            (m) => DspChainItem(
              pluginId: (m['pluginId'] as String?) ?? '',
              typeId: (m['typeId'] as String?) ?? '',
              configJson: (m['configJson'] as String?) ?? '{}',
            ),
          )
          .where((x) => x.pluginId.isNotEmpty && x.typeId.isNotEmpty)
          .toList(growable: false);
    } catch (_) {
      return const [];
    }
  }

  Future<void> setDspChain(List<DspChainItem> chain) async {
    final encoded = jsonEncode(
      chain
          .map(
            (x) => <String, dynamic>{
              'pluginId': x.pluginId,
              'typeId': x.typeId,
              'configJson': x.configJson,
            },
          )
          .toList(growable: false),
    );
    await _box.put(_keyDspChain, encoded);
  }

  Set<String> get disabledPluginIds {
    final raw = _box.get(_keyDisabledPlugins, defaultValue: '[]');
    final text = raw is String ? raw : '[]';
    try {
      final decoded = jsonDecode(text);
      if (decoded is! List) return <String>{};
      return decoded
          .whereType<String>()
          .map((s) => s.trim())
          .where((s) => s.isNotEmpty)
          .toSet();
    } catch (_) {
      return <String>{};
    }
  }

  Future<void> setPluginEnabled({
    required String pluginId,
    required bool enabled,
  }) async {
    final id = pluginId.trim();
    if (id.isEmpty) return;
    final disabled = disabledPluginIds;
    if (enabled) {
      disabled.remove(id);
    } else {
      disabled.add(id);
    }
    await _box.put(_keyDisabledPlugins, jsonEncode(disabled.toList()));
  }
}
