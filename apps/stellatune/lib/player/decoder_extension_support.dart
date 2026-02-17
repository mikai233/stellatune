import 'package:path/path.dart' as p;
import 'package:stellatune/bridge/bridge.dart';

class DecoderExtensionSupportSnapshot {
  const DecoderExtensionSupportSnapshot({
    required this.supportedExtensions,
    required this.wildcard,
  });

  final Set<String> supportedExtensions;
  final bool wildcard;

  bool canPlayLocalPath(String path) {
    if (wildcard) {
      return true;
    }
    final ext = DecoderExtensionSupportCache.normalizeExtension(
      p.extension(path),
    );
    if (ext.isEmpty) {
      return false;
    }
    return supportedExtensions.contains(ext);
  }
}

class DecoderExtensionSupportCache {
  DecoderExtensionSupportCache._();

  static final DecoderExtensionSupportCache instance =
      DecoderExtensionSupportCache._();

  static const DecoderExtensionSupportSnapshot _emptySnapshot =
      DecoderExtensionSupportSnapshot(
        supportedExtensions: <String>{},
        wildcard: false,
      );

  DecoderExtensionSupportSnapshot _snapshot = _emptySnapshot;
  Future<DecoderExtensionSupportSnapshot>? _inFlight;
  bool _ready = false;

  bool get ready => _ready;

  DecoderExtensionSupportSnapshot? get snapshotOrNull =>
      _ready ? _snapshot : null;

  void invalidate() {
    _ready = false;
    _snapshot = _emptySnapshot;
  }

  Future<DecoderExtensionSupportSnapshot> refresh(
    PlayerBridge bridge, {
    bool force = false,
  }) {
    if (_ready && !force) {
      return Future<DecoderExtensionSupportSnapshot>.value(_snapshot);
    }
    final inFlight = _inFlight;
    if (inFlight != null) {
      return inFlight;
    }
    final future = _refreshImpl(bridge);
    _inFlight = future;
    return future;
  }

  Future<DecoderExtensionSupportSnapshot> _refreshImpl(
    PlayerBridge bridge,
  ) async {
    try {
      final entries = await bridge.decoderSupportedExtensions();
      final exts = <String>{};
      var wildcard = false;
      for (final entry in entries) {
        final normalized = normalizeExtension(entry);
        if (normalized.isEmpty) {
          continue;
        }
        if (normalized == '*') {
          wildcard = true;
          continue;
        }
        exts.add(normalized);
      }
      final snapshot = DecoderExtensionSupportSnapshot(
        supportedExtensions: Set.unmodifiable(exts),
        wildcard: wildcard,
      );
      _snapshot = snapshot;
      _ready = true;
      return snapshot;
    } finally {
      _inFlight = null;
    }
  }

  static String normalizeExtension(String raw) {
    final trimmed = raw.trim().toLowerCase();
    if (trimmed == '*') {
      return '*';
    }
    return trimmed.startsWith('.') ? trimmed.substring(1) : trimmed;
  }
}
