import 'dart:convert';
import 'dart:ui';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:flutter/material.dart' show ThemeMode;

import 'package:hive_flutter/hive_flutter.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/app/logging.dart';

class OutputSettingsUiSession {
  bool initialized = false;
  String? selectedOutputBackendKey;
  String? selectedOutputSinkTypeKey;
  String outputSinkConfigJson = '{}';
  String outputSinkTargetJson = '{}';
  List<Object?> outputSinkTargets = const [];
  bool loadingOutputSinkTargets = false;
  final Map<String, String> outputSinkConfigDrafts = <String, String>{};
  List<OutputSinkTypeDescriptor> cachedOutputSinkTypes = const [];
  bool cachedOutputSinkTypesReady = false;
  ResampleQuality resampleQuality = ResampleQuality.high;
}

class SettingsStore extends Notifier<SettingsStore> {
  SettingsStore();
  final OutputSettingsUiSession outputSettingsUiSession =
      OutputSettingsUiSession();

  @override
  SettingsStore build() {
    return this;
  }

  Box get _box => Hive.box(_boxName);

  static const _boxName = 'settings';
  static const _keyVolume = 'volume';
  static const _keyPlayMode = 'play_mode';
  static const _keyResumeTrackRef = 'resume_track_ref';
  static const _keyResumePath = 'resume_path';
  static const _keyResumePositionMs = 'resume_position_ms';
  static const _keyResumeTrackId = 'resume_track_id';
  static const _keyResumeTitle = 'resume_title';
  static const _keyResumeArtist = 'resume_artist';
  static const _keyResumeAlbum = 'resume_album';
  static const _keyResumeDurationMs = 'resume_duration_ms';
  static const _keySelectedBackend = 'selected_backend';
  static const _keySelectedDeviceId = 'selected_device_id';
  static const _keyMatchTrackSampleRate = 'match_track_sample_rate';
  static const _keyGaplessPlayback = 'gapless_playback';
  static const _keySeekTrackFade = 'seek_track_fade';
  static const _keyResampleQuality = 'resample_quality';
  static const _keyOutputSinkRoute = 'output_sink_route';
  static const _keySourceConfigs = 'source_configs';
  static const _keyQueueSource = 'queue_source';
  static const _keyLocale = 'locale';
  static const _keyThemeMode = 'theme_mode';
  static const _keyCloseToTray = 'close_to_tray';

  static Future<void> initHive() async {
    await Hive.initFlutter();
    await Hive.openBox(_boxName);
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

  TrackRef? get resumeTrack {
    final raw = _box.get(_keyResumeTrackRef);
    if (raw is String && raw.trim().isNotEmpty) {
      try {
        final decoded = jsonDecode(raw);
        if (decoded is Map) {
          final map = decoded.cast<String, dynamic>();
          final sourceId = (map['sourceId'] as String?)?.trim() ?? '';
          final trackId = (map['trackId'] as String?)?.trim() ?? '';
          final locator = (map['locator'] as String?)?.trim() ?? '';
          if (sourceId.isNotEmpty && trackId.isNotEmpty && locator.isNotEmpty) {
            return TrackRef(
              sourceId: sourceId,
              trackId: trackId,
              locator: locator,
            );
          }
        }
      } catch (e, s) {
        logger.w('failed to decode resume track', error: e, stackTrace: s);
      }
    }

    final legacyPath = resumePath;
    if (legacyPath == null) return null;
    return TrackRef(
      sourceId: 'local',
      trackId: legacyPath,
      locator: legacyPath,
    );
  }

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
    required TrackRef track,
    required int positionMs,
    int? trackId,
    String? title,
    String? artist,
    String? album,
    int? durationMs,
  }) async {
    await _box.put(
      _keyResumeTrackRef,
      jsonEncode(<String, String>{
        'sourceId': track.sourceId,
        'trackId': track.trackId,
        'locator': track.locator,
      }),
    );
    await _box.put(_keyResumePath, track.locator);
    await _box.put(_keyResumePositionMs, positionMs);
    await _box.put(_keyResumeTrackId, trackId);
    await _box.put(_keyResumeTitle, title);
    await _box.put(_keyResumeArtist, artist);
    await _box.put(_keyResumeAlbum, album);
    await _box.put(_keyResumeDurationMs, durationMs);
  }

  Future<void> clearResume() async {
    await _box.delete(_keyResumeTrackRef);
    await _box.delete(_keyResumePath);
    await _box.delete(_keyResumePositionMs);
    await _box.delete(_keyResumeTrackId);
    await _box.delete(_keyResumeTitle);
    await _box.delete(_keyResumeArtist);
    await _box.delete(_keyResumeAlbum);
    await _box.delete(_keyResumeDurationMs);
  }

  AudioBackend get selectedBackend {
    final raw = _box.get(_keySelectedBackend);
    if (raw is String) {
      switch (raw) {
        case 'shared':
          return AudioBackend.shared;
        case 'wasapiExclusive':
          return AudioBackend.wasapiExclusive;
        default:
          break;
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

  bool get seekTrackFade {
    final v = _box.get(_keySeekTrackFade, defaultValue: true);
    if (v is bool) return v;
    return true;
  }

  Future<void> setSeekTrackFade(bool v) => _box.put(_keySeekTrackFade, v);

  ResampleQuality get resampleQuality {
    final raw = _box.get(_keyResampleQuality);
    if (raw is String) {
      for (final m in ResampleQuality.values) {
        if (m.name == raw) return m;
      }
    }
    return ResampleQuality.high;
  }

  Future<void> setResampleQuality(ResampleQuality v) =>
      _box.put(_keyResampleQuality, v.name);

  OutputSinkRoute? get outputSinkRoute {
    final raw = _box.get(_keyOutputSinkRoute);
    if (raw is! String || raw.trim().isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return null;
      final map = decoded.cast<String, dynamic>();
      final pluginId = (map['pluginId'] as String?)?.trim() ?? '';
      final typeId = (map['typeId'] as String?)?.trim() ?? '';
      if (pluginId.isEmpty || typeId.isEmpty) return null;
      return OutputSinkRoute(
        pluginId: pluginId,
        typeId: typeId,
        configJson: (map['configJson'] as String?) ?? '{}',
        targetJson: (map['targetJson'] as String?) ?? '{}',
      );
    } catch (e, s) {
      logger.w('failed to parse output sink route', error: e, stackTrace: s);
      return null;
    }
  }

  Future<void> setOutputSinkRoute(OutputSinkRoute route) => _box.put(
    _keyOutputSinkRoute,
    jsonEncode(<String, String>{
      'pluginId': route.pluginId,
      'typeId': route.typeId,
      'configJson': route.configJson,
      'targetJson': route.targetJson,
    }),
  );

  Future<void> clearOutputSinkRoute() => _box.delete(_keyOutputSinkRoute);

  Map<String, String> get sourceConfigs {
    final raw = _box.get(_keySourceConfigs, defaultValue: '{}');
    final text = raw is String ? raw : '{}';
    try {
      final decoded = jsonDecode(text);
      if (decoded is! Map) return const <String, String>{};
      final out = <String, String>{};
      for (final entry in decoded.entries) {
        final k = entry.key.toString().trim();
        if (k.isEmpty) continue;
        final v = (entry.value ?? '').toString();
        out[k] = v;
      }
      return out;
    } catch (e, s) {
      logger.w('failed to parse source configs', error: e, stackTrace: s);
      return const <String, String>{};
    }
  }

  String sourceConfigFor({
    required String pluginId,
    required String typeId,
    String defaultValue = '{}',
  }) {
    final key = '${pluginId.trim()}::${typeId.trim()}';
    if (key == '::') return defaultValue;
    return sourceConfigs[key] ?? defaultValue;
  }

  Future<void> setSourceConfigFor({
    required String pluginId,
    required String typeId,
    required String configJson,
  }) async {
    final key = '${pluginId.trim()}::${typeId.trim()}';
    if (key == '::') return;
    final next = Map<String, String>.from(sourceConfigs);
    next[key] = configJson;
    await _box.put(_keySourceConfigs, jsonEncode(next));
  }

  QueueSource? get queueSource {
    final raw = _box.get(_keyQueueSource);
    if (raw is! String || raw.isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) {
        return QueueSource.fromJson(decoded.cast<String, dynamic>());
      }
    } catch (e, s) {
      logger.w('failed to parse queue source', error: e, stackTrace: s);
    }
    return null;
  }

  Future<void> setQueueSource(QueueSource? source) async {
    if (source == null) {
      await _box.delete(_keyQueueSource);
    } else {
      await _box.put(_keyQueueSource, jsonEncode(source.toJson()));
    }
  }

  Locale? get locale {
    final raw = _box.get(_keyLocale);
    if (raw is String && raw.isNotEmpty) {
      final parts = raw.split('_');
      if (parts.length == 1) return Locale(parts[0]);
      if (parts.length == 2) return Locale(parts[0], parts[1]);
    }
    return null;
  }

  Future<void> setLocale(Locale? locale) async {
    if (locale == null) {
      await _box.delete(_keyLocale);
    } else {
      await _box.put(_keyLocale, locale.toString());
    }
    state = this; // trigger update
  }

  ThemeMode get themeMode {
    final raw = _box.get(_keyThemeMode);
    if (raw is String) {
      for (final m in ThemeMode.values) {
        if (m.name == raw) return m;
      }
    }
    return ThemeMode.system;
  }

  Future<void> setThemeMode(ThemeMode mode) async {
    await _box.put(_keyThemeMode, mode.name);
    state = this; // trigger update
  }

  bool get closeToTray {
    final v = _box.get(_keyCloseToTray, defaultValue: true);
    if (v is bool) return v;
    return true;
  }

  Future<void> setCloseToTray(bool v) async {
    await _box.put(_keyCloseToTray, v);
    state = this; // trigger update
  }
}
