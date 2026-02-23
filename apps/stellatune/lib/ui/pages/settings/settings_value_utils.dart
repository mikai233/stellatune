import 'dart:convert';

import 'package:stellatune/bridge/bridge.dart';

class SettingsValueUtils {
  static String sourceTypeKey(SourceCatalogTypeDescriptor t) =>
      '${t.pluginId}::${t.typeId}';

  static String outputSinkTypeKey(OutputSinkTypeDescriptor t) =>
      '${t.pluginId}::${t.typeId}';

  static String localBackendKey(AudioBackend backend) =>
      'local:${backend.name}';

  static String pluginBackendKey(String pluginId, String typeId) =>
      'plugin:$pluginId::$typeId';

  static AudioBackend? parseLocalBackendKey(String? key) {
    if (key == null || !key.startsWith('local:')) return null;
    final raw = key.substring('local:'.length);
    for (final backend in AudioBackend.values) {
      if (backend.name == raw) return backend;
    }
    return null;
  }

  static String? parsePluginTypeKey(String? key) {
    if (key == null || !key.startsWith('plugin:')) return null;
    final raw = key.substring('plugin:'.length);
    final parts = raw.split('::');
    if (parts.length != 2) return null;
    if (parts[0].trim().isEmpty || parts[1].trim().isEmpty) return null;
    return '${parts[0]}::${parts[1]}';
  }

  static String targetValueOf(Object? target) =>
      target is String ? target : jsonEncode(target);

  static String targetLabelOf(Object? target) {
    if (target is Map) {
      final map = target.cast<Object?, Object?>();
      final name = (map['name'] ?? '').toString().trim();
      if (name.isNotEmpty) return name;
      final id = (map['id'] ?? '').toString().trim();
      if (id.isNotEmpty) return id;
    }
    final text = targetValueOf(target);
    return text.length <= 96 ? text : '${text.substring(0, 93)}...';
  }

  static String targetDebugSummary(Object? target) {
    final map = _targetAsMap(target);
    if (map != null) {
      return 'id=${map['id']} session=${map['selection_session_id']} name=${map['name']}';
    }
    final raw = (target ?? '')
        .toString()
        .replaceAll(RegExp(r'\s+'), ' ')
        .trim();
    if (raw.length <= 180) {
      return raw;
    }
    return '${raw.substring(0, 180)}...';
  }

  static bool jsonTextsEquivalent(String left, String right) {
    final leftTrimmed = left.trim();
    final rightTrimmed = right.trim();
    if (leftTrimmed == rightTrimmed) {
      return true;
    }
    final leftCanonical = _canonicalizeJsonText(leftTrimmed);
    final rightCanonical = _canonicalizeJsonText(rightTrimmed);
    if (leftCanonical == null || rightCanonical == null) {
      return false;
    }
    return leftCanonical == rightCanonical;
  }

  static Map<String, Object?>? _targetAsMap(Object? target) {
    if (target is Map) {
      return target.map((k, v) => MapEntry(k.toString(), v));
    }
    if (target is String) {
      final raw = target.trim();
      if (raw.startsWith('{') && raw.endsWith('}')) {
        try {
          final decoded = jsonDecode(raw);
          if (decoded is Map) {
            return decoded.map((k, v) => MapEntry(k.toString(), v));
          }
        } catch (_) {
          // Ignore parse failures and fallback to plain text.
        }
      }
    }
    return null;
  }

  static String? _canonicalizeJsonText(String raw) {
    if (raw.isEmpty) {
      return null;
    }
    try {
      final decoded = jsonDecode(raw);
      final normalized = _normalizeJsonValue(decoded);
      return jsonEncode(normalized);
    } catch (_) {
      return null;
    }
  }

  static Object? _normalizeJsonValue(Object? value) {
    if (value is Map) {
      final entries = value.entries
          .map(
            (entry) => MapEntry(
              entry.key.toString(),
              _normalizeJsonValue(entry.value),
            ),
          )
          .toList();
      entries.sort((a, b) => a.key.compareTo(b.key));
      return <String, Object?>{
        for (final entry in entries) entry.key: entry.value,
      };
    }
    if (value is List) {
      return value.map(_normalizeJsonValue).toList(growable: false);
    }
    return value;
  }
}
