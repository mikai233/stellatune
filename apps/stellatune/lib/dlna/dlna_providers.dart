import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';

/// Selected DLNA renderer (output device). When set, playback routes to DLNA.
final dlnaSelectedRendererProvider = StateProvider<DlnaRenderer?>(
  (ref) => null,
);
