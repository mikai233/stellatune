import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/playback_models.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

final playbackControllerProvider =
    NotifierProvider<PlaybackController, PlaybackState>(PlaybackController.new);

class PlaybackController extends Notifier<PlaybackState> {
  StreamSubscription<Event>? _sub;

  @override
  PlaybackState build() {
    unawaited(_sub?.cancel());

    final bridge = ref.read(playerBridgeProvider);
    _sub = bridge.events().listen(
      _onEvent,
      onError: (Object err, StackTrace st) {
        ref
            .read(loggerProvider)
            .e('rust events error: $err', error: err, stackTrace: st);
        state = state.copyWith(lastError: err.toString());
      },
    );

    ref.onDispose(() => unawaited(_sub?.cancel()));
    return const PlaybackState.initial();
  }

  Future<void> setQueueAndPlay(List<String> paths, {int startIndex = 0}) async {
    final items = paths.map((p) => QueueItem(path: p)).toList();
    ref
        .read(queueControllerProvider.notifier)
        .setQueue(items, startIndex: startIndex);
    final item = ref.read(queueControllerProvider).currentItem;
    if (item == null) return;
    await _loadAndPlay(item.path);
  }

  Future<void> enqueue(List<String> paths) async {
    final items = paths.map((p) => QueueItem(path: p)).toList();
    final queue = ref.read(queueControllerProvider);
    ref.read(queueControllerProvider.notifier).enqueue(items);

    // If nothing is loaded yet, start playing immediately from the first enqueued item.
    if (queue.currentItem == null && items.isNotEmpty) {
      await _loadAndPlay(items.first.path);
    }
  }

  Future<void> playIndex(int index) async {
    ref.read(queueControllerProvider.notifier).selectIndex(index);
    final item = ref.read(queueControllerProvider).currentItem;
    if (item == null) return;
    await _loadAndPlay(item.path);
  }

  Future<void> play() => ref.read(playerBridgeProvider).play();
  Future<void> pause() => ref.read(playerBridgeProvider).pause();

  Future<void> stop() async {
    await ref.read(playerBridgeProvider).stop();
    state = state.copyWith(positionMs: 0);
  }

  Future<void> next({bool auto = false}) async {
    final item = ref
        .read(queueControllerProvider.notifier)
        .next(fromAuto: auto);
    if (item == null) {
      await stop();
      return;
    }
    await _loadAndPlay(item.path);
  }

  Future<void> previous() async {
    final item = ref.read(queueControllerProvider.notifier).previous();
    if (item == null) return;
    await _loadAndPlay(item.path);
  }

  Future<void> _loadAndPlay(String path) async {
    state = state.copyWith(lastError: null, lastLog: '');
    final bridge = ref.read(playerBridgeProvider);
    await bridge.load(path);
    await bridge.play();
  }

  void _onEvent(Event event) {
    event.when(
      stateChanged: (s) {
        state = state.copyWith(playerState: s);
      },
      position: (ms) {
        state = state.copyWith(positionMs: ms);
      },
      trackChanged: (path) {
        state = state.copyWith(currentPath: path);
      },
      playbackEnded: (path) {
        ref.read(loggerProvider).i('playback ended: $path');
        unawaited(next(auto: true));
      },
      error: (message) {
        ref.read(loggerProvider).e(message);
        state = state.copyWith(lastError: message);
      },
      log: (message) {
        ref.read(loggerProvider).d(message);
        state = state.copyWith(lastLog: message);
      },
    );
  }
}
