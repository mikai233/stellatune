import 'dart:convert';

import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/decoder_extension_support.dart';

typedef TrackLocatorPluginIds = ({
  String? sourcePluginId,
  String? decoderPluginId,
});

const Set<String> kDisabledPluginPruneReasons = {
  'source_catalog_unavailable',
  'source_decoder_unavailable',
};

class PlaybackPlayabilityUtils {
  static bool isLocalTrack(TrackRef track) =>
      track.sourceId.trim().toLowerCase() == 'local';

  static TrackLocatorPluginIds extractTrackLocatorPluginIds(TrackRef track) {
    if (isLocalTrack(track)) {
      return (sourcePluginId: null, decoderPluginId: null);
    }
    final locator = track.locator.trim();
    if (locator.isEmpty) {
      return (sourcePluginId: null, decoderPluginId: null);
    }
    try {
      final decoded = jsonDecode(locator);
      if (decoded is! Map) {
        return (sourcePluginId: null, decoderPluginId: null);
      }
      final sourcePluginId = (decoded['plugin_id'] as Object?)
          ?.toString()
          .trim();
      final decoderPluginId = (decoded['decoder_plugin_id'] as Object?)
          ?.toString()
          .trim();
      return (
        sourcePluginId: sourcePluginId == null || sourcePluginId.isEmpty
            ? null
            : sourcePluginId,
        decoderPluginId: decoderPluginId == null || decoderPluginId.isEmpty
            ? null
            : decoderPluginId,
      );
    } catch (_) {
      // Ignore non-JSON locator payloads.
      return (sourcePluginId: null, decoderPluginId: null);
    }
  }

  static Set<String> trackPluginIds(TrackRef track) {
    final out = <String>{};
    final ids = extractTrackLocatorPluginIds(track);
    if (ids.sourcePluginId != null) {
      out.add(ids.sourcePluginId!);
    }
    if (ids.decoderPluginId != null) {
      out.add(ids.decoderPluginId!);
    }
    return out;
  }

  static bool isDisabledPluginPruneReason(String? reasonCode) {
    final code = reasonCode?.trim() ?? '';
    return kDisabledPluginPruneReasons.contains(code);
  }

  static String? localTrackPlayabilityBlockReasonFast(
    TrackRef track,
    DecoderExtensionSupportSnapshot? snapshot,
  ) {
    if (!isLocalTrack(track) || snapshot == null) {
      return null;
    }
    return snapshot.canPlayLocalPath(track.locator)
        ? null
        : 'no_decoder_for_local_track';
  }

  static String? disabledPluginBlockReason({
    required TrackRef track,
    required Set<String> disabledPluginIds,
  }) {
    if (disabledPluginIds.isEmpty) {
      return null;
    }
    final ids = extractTrackLocatorPluginIds(track);
    final sourcePluginId = ids.sourcePluginId;
    final decoderPluginId = ids.decoderPluginId;
    if (sourcePluginId != null && disabledPluginIds.contains(sourcePluginId)) {
      return 'source_catalog_unavailable';
    }
    if (decoderPluginId != null &&
        disabledPluginIds.contains(decoderPluginId)) {
      return 'source_decoder_unavailable';
    }
    return null;
  }
}
