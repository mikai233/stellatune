import 'dart:convert';
import 'dart:ui';

import 'package:flutter/material.dart' show ThemeMode;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hive_flutter/hive_flutter.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/platform/directory_access_store.dart';
import 'package:stellatune/player/queue_models.dart';

const _unset = Object();

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

class SettingsState {
  SettingsState({
    required this.volume,
    required this.playMode,
    required this.selectedBackend,
    required this.selectedDeviceId,
    required this.matchTrackSampleRate,
    required this.gaplessPlayback,
    required this.seekTrackFade,
    required this.resampleQuality,
    required this.outputSinkRoute,
    required this.queueSource,
    required this.locale,
    required this.themeMode,
    required this.closeToTray,
  });

  final double volume;
  final PlayMode playMode;
  final AudioBackend selectedBackend;
  final String? selectedDeviceId;
  final bool matchTrackSampleRate;
  final bool gaplessPlayback;
  final bool seekTrackFade;
  final ResampleQuality resampleQuality;
  final OutputSinkRoute? outputSinkRoute;
  final QueueSource? queueSource;
  final Locale? locale;
  final ThemeMode themeMode;
  final bool closeToTray;

  SettingsState copyWith({
    double? volume,
    PlayMode? playMode,
    AudioBackend? selectedBackend,
    Object? selectedDeviceId = _unset,
    bool? matchTrackSampleRate,
    bool? gaplessPlayback,
    bool? seekTrackFade,
    ResampleQuality? resampleQuality,
    Object? outputSinkRoute = _unset,
    Object? queueSource = _unset,
    Object? locale = _unset,
    ThemeMode? themeMode,
    bool? closeToTray,
  }) {
    return SettingsState(
      volume: volume ?? this.volume,
      playMode: playMode ?? this.playMode,
      selectedBackend: selectedBackend ?? this.selectedBackend,
      selectedDeviceId: identical(selectedDeviceId, _unset)
          ? this.selectedDeviceId
          : selectedDeviceId as String?,
      matchTrackSampleRate: matchTrackSampleRate ?? this.matchTrackSampleRate,
      gaplessPlayback: gaplessPlayback ?? this.gaplessPlayback,
      seekTrackFade: seekTrackFade ?? this.seekTrackFade,
      resampleQuality: resampleQuality ?? this.resampleQuality,
      outputSinkRoute: identical(outputSinkRoute, _unset)
          ? this.outputSinkRoute
          : outputSinkRoute as OutputSinkRoute?,
      queueSource: identical(queueSource, _unset)
          ? this.queueSource
          : queueSource as QueueSource?,
      locale: identical(locale, _unset) ? this.locale : locale as Locale?,
      themeMode: themeMode ?? this.themeMode,
      closeToTray: closeToTray ?? this.closeToTray,
    );
  }
}

class SettingsStore implements DirectoryAccessStore {
  SettingsStore();

  final OutputSettingsUiSession outputSettingsUiSession =
      OutputSettingsUiSession();

  static const _boxName = 'settings';
  static const _keyVolume = 'volume';
  static const _keyPlayMode = 'play_mode';
  static const _keySelectedBackend = 'selected_backend';
  static const _keySelectedDeviceId = 'selected_device_id';
  static const _keyMatchTrackSampleRate = 'match_track_sample_rate';
  static const _keyGaplessPlayback = 'gapless_playback';
  static const _keySeekTrackFade = 'seek_track_fade';
  static const _keyResampleQuality = 'resample_quality';
  static const _keyOutputSinkRoute = 'output_sink_route';
  static const _keyQueueSource = 'queue_source';
  static const _keyLocale = 'locale';
  static const _keyThemeMode = 'theme_mode';
  static const _keyCloseToTray = 'close_to_tray';
  static const _keyMacosDirectoryBookmarks = 'macos_directory_bookmarks';

  static Future<void> initHive() async {
    await Hive.initFlutter();
    await Hive.openBox(_boxName);
  }

  Box get _box => Hive.box(_boxName);

  SettingsState readState() {
    return SettingsState(
      volume: volume,
      playMode: playMode,
      selectedBackend: selectedBackend,
      selectedDeviceId: selectedDeviceId,
      matchTrackSampleRate: matchTrackSampleRate,
      gaplessPlayback: gaplessPlayback,
      seekTrackFade: seekTrackFade,
      resampleQuality: resampleQuality,
      outputSinkRoute: outputSinkRoute,
      queueSource: queueSource,
      locale: locale,
      themeMode: themeMode,
      closeToTray: closeToTray,
    );
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

  Future<void> setSelectedDeviceId(String? id) =>
      _box.put(_keySelectedDeviceId, id);

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

  Future<void> setThemeMode(ThemeMode mode) =>
      _box.put(_keyThemeMode, mode.name);

  bool get closeToTray {
    final v = _box.get(_keyCloseToTray, defaultValue: true);
    if (v is bool) return v;
    return true;
  }

  Future<void> setCloseToTray(bool v) => _box.put(_keyCloseToTray, v);

  @override
  Map<String, String> get macosDirectoryBookmarks {
    final raw = _box.get(_keyMacosDirectoryBookmarks, defaultValue: '{}');
    final text = raw is String ? raw : '{}';
    try {
      final decoded = jsonDecode(text);
      if (decoded is! Map) return const <String, String>{};
      final out = <String, String>{};
      for (final entry in decoded.entries) {
        final key = entry.key.toString().trim();
        final value = (entry.value ?? '').toString().trim();
        if (key.isEmpty || value.isEmpty) continue;
        out[key] = value;
      }
      return out;
    } catch (e, s) {
      logger.w(
        'failed to parse macos directory bookmarks',
        error: e,
        stackTrace: s,
      );
      return const <String, String>{};
    }
  }

  @override
  String? macosDirectoryBookmarkForPath(String path) {
    final normalized = _normalizeBookmarkPath(path);
    if (normalized.isEmpty) return null;
    return macosDirectoryBookmarks[normalized];
  }

  @override
  Future<void> setMacosDirectoryBookmark({
    required String path,
    required String bookmark,
  }) async {
    final normalized = _normalizeBookmarkPath(path);
    final trimmedBookmark = bookmark.trim();
    if (normalized.isEmpty || trimmedBookmark.isEmpty) return;
    final next = Map<String, String>.from(macosDirectoryBookmarks);
    next[normalized] = trimmedBookmark;
    await _box.put(_keyMacosDirectoryBookmarks, jsonEncode(next));
  }

  @override
  Future<void> removeMacosDirectoryBookmark(String path) async {
    final normalized = _normalizeBookmarkPath(path);
    if (normalized.isEmpty) return;
    final next = Map<String, String>.from(macosDirectoryBookmarks);
    if (next.remove(normalized) == null) return;
    await _box.put(_keyMacosDirectoryBookmarks, jsonEncode(next));
  }

  static String _normalizeBookmarkPath(String path) {
    var value = path.trim().replaceAll('\\', '/');
    while (value.length > 1 && value.endsWith('/')) {
      value = value.substring(0, value.length - 1);
    }
    return value;
  }
}

final settingsStoreServiceProvider = Provider<SettingsStore>((ref) {
  throw UnimplementedError(
    'settingsStoreServiceProvider must be overridden in main()',
  );
});

final settingsUiSessionProvider = Provider<OutputSettingsUiSession>((ref) {
  return ref.watch(settingsStoreServiceProvider).outputSettingsUiSession;
});

class SettingsController extends Notifier<SettingsState> {
  SettingsStore get _store => ref.read(settingsStoreServiceProvider);

  @override
  SettingsState build() {
    return _store.readState();
  }

  Future<void> _persist(
    Future<void> Function(SettingsStore store) action,
  ) async {
    await action(_store);
    state = _store.readState();
  }

  Future<void> setVolume(double v) => _persist((store) => store.setVolume(v));

  Future<void> setPlayMode(PlayMode mode) =>
      _persist((store) => store.setPlayMode(mode));

  Future<void> setSelectedBackend(AudioBackend backend) =>
      _persist((store) => store.setSelectedBackend(backend));

  Future<void> setSelectedDeviceId(String? id) =>
      _persist((store) => store.setSelectedDeviceId(id));

  Future<void> setMatchTrackSampleRate(bool v) =>
      _persist((store) => store.setMatchTrackSampleRate(v));

  Future<void> setGaplessPlayback(bool v) =>
      _persist((store) => store.setGaplessPlayback(v));

  Future<void> setSeekTrackFade(bool v) =>
      _persist((store) => store.setSeekTrackFade(v));

  Future<void> setResampleQuality(ResampleQuality v) =>
      _persist((store) => store.setResampleQuality(v));

  Future<void> setOutputSinkRoute(OutputSinkRoute route) =>
      _persist((store) => store.setOutputSinkRoute(route));

  Future<void> clearOutputSinkRoute() =>
      _persist((store) => store.clearOutputSinkRoute());

  Future<void> setQueueSource(QueueSource? source) =>
      _persist((store) => store.setQueueSource(source));

  Future<void> setLocale(Locale? locale) =>
      _persist((store) => store.setLocale(locale));

  Future<void> setThemeMode(ThemeMode mode) =>
      _persist((store) => store.setThemeMode(mode));

  Future<void> setCloseToTray(bool v) =>
      _persist((store) => store.setCloseToTray(v));
}

final settingsStoreProvider =
    NotifierProvider<SettingsController, SettingsState>(SettingsController.new);
