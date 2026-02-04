import 'package:hive_flutter/hive_flutter.dart';
import 'package:stellatune/player/queue_models.dart';

class SettingsStore {
  SettingsStore._(this._box);

  static const _boxName = 'settings';
  static const _keyVolume = 'volume';
  static const _keyPlayMode = 'play_mode';

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
}
