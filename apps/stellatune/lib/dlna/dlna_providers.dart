import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/third_party/stellatune_core.dart';

class DlnaSelectedRendererNotifier extends Notifier<DlnaRenderer?> {
  @override
  DlnaRenderer? build() => null;

  void set(DlnaRenderer? renderer) => state = renderer;
}

/// Selected DLNA renderer (output device). When set, playback routes to DLNA.
final dlnaSelectedRendererProvider =
    NotifierProvider<DlnaSelectedRendererNotifier, DlnaRenderer?>(
      DlnaSelectedRendererNotifier.new,
    );
