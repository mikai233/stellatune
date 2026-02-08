import 'package:stellatune/l10n/app_localizations.dart';

const String kPlayabilityErrorPrefix = 'playability:';

String encodePlayabilityError(String reasonCode) {
  final code = reasonCode.trim();
  if (code.isEmpty) return '${kPlayabilityErrorPrefix}unsupported_track';
  return '$kPlayabilityErrorPrefix$code';
}

String localizePlayabilityReason(AppLocalizations l10n, String? reasonCode) {
  final code = reasonCode?.trim() ?? '';
  return switch (code) {
    'plugins_unavailable' => l10n.playabilityReasonPluginsUnavailable,
    'local_track_locator_empty' => l10n.playabilityReasonLocalTrackLocatorEmpty,
    'no_decoder_for_local_track' =>
      l10n.playabilityReasonNoDecoderForLocalTrack,
    'decoder_probe_failed' => l10n.playabilityReasonDecoderProbeFailed,
    'invalid_source_track_locator' =>
      l10n.playabilityReasonInvalidSourceTrackLocator,
    'source_catalog_unavailable' =>
      l10n.playabilityReasonSourceCatalogUnavailable,
    'source_decoder_unavailable' =>
      l10n.playabilityReasonSourceDecoderUnavailable,
    '' => l10n.playabilityReasonUnsupportedTrack,
    _ => l10n.playabilityReasonUnknown(code),
  };
}

String localizePlaybackError(AppLocalizations l10n, String raw) {
  if (!raw.startsWith(kPlayabilityErrorPrefix)) {
    return raw;
  }
  final code = raw.substring(kPlayabilityErrorPrefix.length).trim();
  return l10n.playbackUnavailable(localizePlayabilityReason(l10n, code));
}
