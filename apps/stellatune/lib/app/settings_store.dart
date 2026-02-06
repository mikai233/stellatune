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
  static const _keyResumeTrackId = 'resume_track_id';
  static const _keyResumeTitle = 'resume_title';
  static const _keyResumeArtist = 'resume_artist';
  static const _keyResumeAlbum = 'resume_album';
  static const _keyResumeDurationMs = 'resume_duration_ms';
  static const _keyDspChain = 'dsp_chain';
  static const _keyDisabledPlugins = 'disabled_plugins';
  static const _keySelectedBackend = 'selected_backend';
  static const _keySelectedDeviceId = 'selected_device_id';
  static const _keyMatchTrackSampleRate = 'match_track_sample_rate';
  static const _keyGaplessPlayback = 'gapless_playback';

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

  int? get resumeTrackId => _box.get(_keyResumeTrackId);
  String? get resumeTitle => _box.get(_keyResumeTitle);
  String? get resumeArtist => _box.get(_keyResumeArtist);
  String? get resumeAlbum => _box.get(_keyResumeAlbum);
  int? get resumeDurationMs => _box.get(_keyResumeDurationMs);

  Future<void> setResume({
    required String path,
    required int positionMs,
    int? trackId,
    String? title,
    String? artist,
    String? album,
    int? durationMs,
  }) async {
    await _box.put(_keyResumePath, path);
    await _box.put(_keyResumePositionMs, positionMs);
    await _box.put(_keyResumeTrackId, trackId);
    await _box.put(_keyResumeTitle, title);
    await _box.put(_keyResumeArtist, artist);
    await _box.put(_keyResumeAlbum, album);
    await _box.put(_keyResumeDurationMs, durationMs);
  }

  Future<void> clearResume() async {
    await _box.delete(_keyResumePath);
    await _box.delete(_keyResumePositionMs);
    await _box.delete(_keyResumeTrackId);
    await _box.delete(_keyResumeTitle);
    await _box.delete(_keyResumeArtist);
    await _box.delete(_keyResumeAlbum);
    await _box.delete(_keyResumeDurationMs);
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

  AudioBackend get selectedBackend {
    final raw = _box.get(_keySelectedBackend);
    if (raw is String) {
      for (final b in AudioBackend.values) {
        if (b.name == raw) return b;
      }
    }
    return AudioBackend.shared;
  }

  Future<void> setSelectedBackend(AudioBackend backend) =>
      _box.put(_keySelectedBackend, backend.name);

  String? get selectedDeviceId {
    final v = _box.get(_keySelectedDeviceId);
    if (v is String && v.trim().isNotEmpty) return v;
    return null;
  }

  Future<void> setSelectedDeviceId(String? id) async {
    await _box.put(_keySelectedDeviceId, id);
  }

  bool get matchTrackSampleRate {
    final v = _box.get(_keyMatchTrackSampleRate, defaultValue: false);
    if (v is bool) return v;
    return false;
  }

  Future<void> setMatchTrackSampleRate(bool v) =>
      _box.put(_keyMatchTrackSampleRate, v);

  bool get gaplessPlayback {
    final v = _box.get(_keyGaplessPlayback, defaultValue: true);
    if (v is bool) return v;
    return true;
  }

  Future<void> setGaplessPlayback(bool v) => _box.put(_keyGaplessPlayback, v);
}
