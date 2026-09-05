import 'package:stellatune/player/decoder_extension_support.dart';
import 'package:stellatune/player/queue_models.dart';

typedef TrackLocatorPluginIds = ({
  String? sourcePluginId,
  String? decoderPluginId,
});

const Set<String> kDisabledPluginPruneReasons = {
  'source_catalog_unavailable',
  'source_decoder_unavailable',
};

class PlaybackPlayabilityUtils {
  static bool isLocalTrack(QueueItem item) => item.isLocal;

  static TrackLocatorPluginIds extractPluginIds(QueueItem item) {
    final provider = item.providerTrack;
    if (provider == null) {
      return (sourcePluginId: null, decoderPluginId: null);
    }
    return (
      sourcePluginId: provider.sourcePluginId,
      decoderPluginId: provider.decoderPluginId,
    );
  }

  static Set<String> trackPluginIds(QueueItem item) {
    final out = <String>{};
    final ids = extractPluginIds(item);
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
    QueueItem item,
    DecoderExtensionSupportSnapshot? snapshot,
  ) {
    if (!isLocalTrack(item) || snapshot == null) {
      return null;
    }
    return snapshot.canPlayLocalPath(item.path)
        ? null
        : 'no_decoder_for_local_track';
  }

  static String? disabledPluginBlockReason({
    required QueueItem item,
    required Set<String> disabledPluginIds,
  }) {
    if (disabledPluginIds.isEmpty) {
      return null;
    }
    final ids = extractPluginIds(item);
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
