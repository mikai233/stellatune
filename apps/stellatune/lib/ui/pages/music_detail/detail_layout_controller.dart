import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/lyrics/lyrics_controller.dart';
import 'package:stellatune/player/queue_controller.dart';

typedef DetailLayoutInput = ({
  String trackKey,
  int orderPos,
  int orderLength,
  bool hasLyrics,
});

final detailLayoutInputProvider = Provider<DetailLayoutInput>((ref) {
  final track = ref.watch(
    queueControllerProvider.select(
      (queue) => (
        key: queue.currentItem?.stableTrackKey ?? '',
        position: queue.orderPos,
        length: queue.order.length,
      ),
    ),
  );
  final hasLyrics = ref.watch(
    lyricsControllerProvider.select(
      (lyrics) =>
          lyrics.trackKey == track.key && lyrics.enabled && lyrics.hasLyrics,
    ),
  );
  return (
    trackKey: track.key,
    orderPos: track.position,
    orderLength: track.length,
    hasLyrics: hasLyrics,
  );
});

typedef DetailLayoutState = ({int slideDirection, bool hasLyrics});

final detailLayoutControllerProvider =
    NotifierProvider.autoDispose<DetailLayoutController, DetailLayoutState>(
      DetailLayoutController.new,
    );

/// Coordinates only layout transitions. Playback position and lyric line
/// changes are deliberately absent from its input, so they cannot cancel a delay.
class DetailLayoutController extends Notifier<DetailLayoutState> {
  Timer? _delay;

  @override
  DetailLayoutState build() {
    _delay?.cancel();
    ref.onDispose(() => _delay?.cancel());
    final initial = ref.read(detailLayoutInputProvider);
    ref.listen(detailLayoutInputProvider, (previous, next) {
      if (previous != null) _update(previous, next);
    });
    return (slideDirection: 0, hasLyrics: initial.hasLyrics);
  }

  void _update(DetailLayoutInput previous, DetailLayoutInput next) {
    var direction = state.slideDirection;
    final trackChanged = previous.trackKey != next.trackKey;
    if (trackChanged) {
      direction = 0;
      if (previous.trackKey.isNotEmpty && next.orderLength > 1) {
        if (next.orderPos == 0 && previous.orderPos == next.orderLength - 1) {
          direction = 1;
        } else if (next.orderPos == next.orderLength - 1 &&
            previous.orderPos == 0) {
          direction = -1;
        } else {
          direction = next.orderPos > previous.orderPos ? 1 : -1;
        }
      }
    }

    // An unrelated queue edit while the same track is active must also leave
    // an existing transition's deadline intact.
    if (!trackChanged && previous.hasLyrics == next.hasLyrics) return;
    _delay?.cancel();
    _delay = null;
    final conflict =
        trackChanged &&
        ((state.hasLyrics && !next.hasLyrics && direction == 1) ||
            (!state.hasLyrics && next.hasLyrics && direction == -1));
    if (conflict) {
      state = (slideDirection: direction, hasLyrics: state.hasLyrics);
      _delay = Timer(const Duration(milliseconds: 350), () {
        _delay = null;
        if (ref.mounted) {
          state = (slideDirection: direction, hasLyrics: next.hasLyrics);
        }
      });
    } else {
      state = (slideDirection: direction, hasLyrics: next.hasLyrics);
    }
  }
}
