import 'dart:convert';

import 'package:stellatune/player/queue_models.dart';

class PlaylistsPluginValueUtils {
  static String? asText(Object? value) {
    if (value == null) return null;
    final text = value.toString().trim();
    if (text.isEmpty) return null;
    return text;
  }

  static int? asInt(Object? value) {
    if (value is int) return value;
    if (value is num) return value.toInt();
    return int.tryParse(value?.toString().trim() ?? '');
  }

  static QueueCover? asCover(Object? value) {
    if (value is! Map) return null;
    final map = value.cast<Object?, Object?>();
    final kindRaw = asText(map['kind'])?.toLowerCase();
    final v = asText(map['value']);
    if (kindRaw == null || v == null) return null;
    final kind = switch (kindRaw) {
      'url' => QueueCoverKind.url,
      'file' => QueueCoverKind.file,
      'data' => QueueCoverKind.data,
      _ => null,
    };
    if (kind == null) return null;
    return QueueCover(kind: kind, value: v, mime: asText(map['mime']));
  }

  static Map<String, Object?> decodeJsonObjectOrEmpty(String raw) {
    final text = raw.trim();
    if (text.isEmpty) return <String, Object?>{};
    try {
      final decoded = jsonDecode(text);
      if (decoded is Map<String, Object?>) return decoded;
      if (decoded is Map) return decoded.cast<String, Object?>();
    } catch (_) {}
    return <String, Object?>{};
  }
}
