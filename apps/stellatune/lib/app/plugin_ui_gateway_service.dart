import 'package:stellatune/app/logging.dart';
import 'package:stellatune/bridge/api/player.dart' as player_api;

class PluginUiGatewayService {
  PluginUiGatewayService._();

  static final PluginUiGatewayService instance = PluginUiGatewayService._();

  String? _pluginsDir;
  String? _baseUrl;
  Future<String>? _startInFlight;

  Future<String> ensureStarted({required String pluginsDir}) {
    final normalizedDir = pluginsDir.trim();
    if (normalizedDir.isEmpty) {
      throw ArgumentError.value(pluginsDir, 'pluginsDir', 'must not be empty');
    }
    _pluginsDir = normalizedDir;
    final cached = _baseUrl;
    if (cached != null && cached.isNotEmpty) {
      return Future.value(cached);
    }

    final inFlight = _startInFlight;
    if (inFlight != null) {
      return inFlight;
    }

    final startFuture = _startInternal(normalizedDir);
    _startInFlight = startFuture;
    return startFuture.whenComplete(() {
      if (identical(_startInFlight, startFuture)) {
        _startInFlight = null;
      }
    });
  }

  Future<String?> pluginUiUrl({
    required String pluginId,
    String? pluginsDir,
  }) async {
    final normalizedPluginId = pluginId.trim();
    if (normalizedPluginId.isEmpty) {
      return null;
    }

    final preferredDir = (pluginsDir ?? _pluginsDir ?? '').trim();
    if (preferredDir.isNotEmpty) {
      await ensureStarted(pluginsDir: preferredDir);
    } else {
      final existingBaseUrl = await player_api.pluginUiGatewayBaseUrl();
      final normalizedBase = existingBaseUrl?.trim() ?? '';
      if (normalizedBase.isEmpty) {
        return null;
      }
      _baseUrl = normalizedBase;
    }

    return player_api.pluginUiGatewayPluginUiUrl(pluginId: normalizedPluginId);
  }

  Future<void> stopIfStarted() async {
    try {
      await player_api.pluginUiGatewayStop();
    } finally {
      _baseUrl = null;
      _startInFlight = null;
    }
  }

  Future<String> _startInternal(String pluginsDir) async {
    final baseUrl = (await player_api.pluginUiGatewayStart(
      pluginsDir: pluginsDir,
    )).trim();
    _baseUrl = baseUrl;
    logger.i('plugin ui gateway started base_url=$baseUrl');
    return baseUrl;
  }
}
