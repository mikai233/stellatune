import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/lyrics/lyrics_state.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

final lyricsControllerProvider =
    NotifierProvider<LyricsController, LyricsState>(LyricsController.new);

class LyricsController extends Notifier<LyricsState> {
  StreamSubscription<LyricsEvent>? _sub;
  String? _preparedTrackKey;
  String? _prefetchedTrackKey;
  int? _lastPositionMs;

  @override
  LyricsState build() {
    unawaited(_sub?.cancel());
    _preparedTrackKey = null;
    _prefetchedTrackKey = null;
    _lastPositionMs = null;

    final bridge = ref.read(playerBridgeProvider);
    _sub = bridge.lyricsEvents().listen(
      _onEvent,
      onError: (Object err, StackTrace st) {
        ref
            .read(loggerProvider)
            .e('lyrics events error: $err', error: err, stackTrace: st);
        state = state.copyWith(
          status: LyricsStatus.error,
          lastError: err.toString(),
        );
      },
    );

    ref.onDispose(() {
      unawaited(_sub?.cancel());
    });

    ref.listen<QueueState>(queueControllerProvider, (prev, next) {
      _prepareForQueueItem(next.currentItem);
      _prefetchNextQueueItem(next);
    });

    ref.listen<int>(playbackControllerProvider.select((s) => s.positionMs), (
      prev,
      next,
    ) {
      if (prev == next) return;
      unawaited(_setPosition(next));
    });

    Future.microtask(() {
      final queue = ref.read(queueControllerProvider);
      _prepareForQueueItem(queue.currentItem);
      _prefetchNextQueueItem(queue);
      unawaited(_setPosition(ref.read(playbackControllerProvider).positionMs));
    });

    return const LyricsState.initial();
  }

  Future<void> refreshCurrent() async {
    try {
      await ref.read(playerBridgeProvider).lyricsRefreshCurrent();
    } catch (e, st) {
      ref.read(loggerProvider).w('lyrics refresh failed: $e', stackTrace: st);
    }
  }

  Future<void> clearCache() async {
    try {
      await ref.read(playerBridgeProvider).lyricsClearCache();
    } catch (e, st) {
      ref
          .read(loggerProvider)
          .e('lyrics clear cache failed: $e', error: e, stackTrace: st);
      state = state.copyWith(
        status: LyricsStatus.error,
        lastError: e.toString(),
      );
      rethrow;
    }
    await refreshCurrent();
  }

  void setEnabled(bool enabled) {
    if (state.enabled == enabled) return;
    state = state.copyWith(enabled: enabled);
  }

  Future<List<LyricsSearchCandidate>> searchCandidatesForCurrent() async {
    final item = ref.read(queueControllerProvider).currentItem;
    final query = _queryFromQueueItem(item);
    if (query == null) {
      return const <LyricsSearchCandidate>[];
    }
    return ref.read(playerBridgeProvider).lyricsSearchCandidates(query);
  }

  Future<void> applyCandidate(LyricsSearchCandidate candidate) async {
    final trackKey = _currentTrackKey();
    if (trackKey == null) return;
    await ref
        .read(playerBridgeProvider)
        .lyricsApplyCandidate(trackKey: trackKey, doc: candidate.doc);
  }

  void _prepareForQueueItem(QueueItem? item) {
    if (item == null) {
      _preparedTrackKey = null;
      state = state.copyWith(
        status: LyricsStatus.idle,
        trackKey: null,
        doc: null,
        currentLineIndex: -1,
        lastError: null,
      );
      return;
    }

    final trackKey = item.path.trim();
    if (trackKey.isEmpty) {
      _preparedTrackKey = null;
      state = state.copyWith(
        status: LyricsStatus.idle,
        trackKey: null,
        doc: null,
        currentLineIndex: -1,
        lastError: null,
      );
      return;
    }
    if (_preparedTrackKey == trackKey) {
      return;
    }
    _preparedTrackKey = trackKey;

    final query = _queryFromQueueItem(item);
    if (query == null) return;

    state = state.copyWith(
      status: LyricsStatus.loading,
      trackKey: trackKey,
      doc: null,
      currentLineIndex: -1,
      lastError: null,
    );

    unawaited(
      ref.read(playerBridgeProvider).lyricsPrepare(query).catchError((
        Object err,
        StackTrace st,
      ) {
        ref
            .read(loggerProvider)
            .e('lyrics prepare failed: $err', error: err, stackTrace: st);
        state = state.copyWith(
          status: LyricsStatus.error,
          lastError: err.toString(),
        );
      }),
    );
  }

  void _prefetchNextQueueItem(QueueState queue) {
    final currentPath = queue.currentItem?.path.trim();
    final next = _peekNextQueueItem(queue);
    final nextPath = next?.path.trim();
    if (nextPath == null || nextPath.isEmpty || nextPath == currentPath) {
      _prefetchedTrackKey = null;
      return;
    }
    if (_prefetchedTrackKey == nextPath) return;
    _prefetchedTrackKey = nextPath;

    final query = _queryFromQueueItem(next!);
    if (query == null) return;
    unawaited(
      ref.read(playerBridgeProvider).lyricsPrefetch(query).catchError((
        Object err,
        StackTrace st,
      ) {
        ref
            .read(loggerProvider)
            .d('lyrics prefetch failed: $err', stackTrace: st);
      }),
    );
  }

  String? _currentTrackKey() {
    final path =
        ref.read(queueControllerProvider).currentItem?.path.trim() ?? '';
    if (path.isEmpty) return null;
    return path;
  }

  LyricsQuery? _queryFromQueueItem(QueueItem? item) {
    if (item == null) return null;
    final trackKey = item.path.trim();
    if (trackKey.isEmpty) return null;
    final title = _resolveTitle(item);
    if (title.isEmpty) return null;
    return LyricsQuery(
      trackKey: trackKey,
      title: title,
      artist: _trimOrNull(item.artist),
      album: _trimOrNull(item.album),
      durationMs: item.durationMs,
    );
  }

  QueueItem? _peekNextQueueItem(QueueState queue) {
    final current = queue.currentItem;
    if (current == null || queue.items.isEmpty || queue.order.isEmpty) {
      return null;
    }

    if (queue.repeatMode == RepeatMode.one) {
      return current;
    }

    final nextPos = queue.orderPos + 1;
    if (nextPos < queue.order.length) {
      final nextIndex = queue.order[nextPos];
      if (nextIndex >= 0 && nextIndex < queue.items.length) {
        return queue.items[nextIndex];
      }
      return null;
    }

    if (queue.repeatMode != RepeatMode.all) {
      return null;
    }

    if (queue.shuffle) {
      return null;
    }

    return queue.items.first;
  }

  Future<void> _setPosition(int positionMs) async {
    if (_lastPositionMs == positionMs) return;
    _lastPositionMs = positionMs;

    try {
      await ref.read(playerBridgeProvider).lyricsSetPositionMs(positionMs);
    } catch (e, st) {
      ref
          .read(loggerProvider)
          .d('lyrics set position failed: $e', stackTrace: st);
    }
  }

  void _onEvent(LyricsEvent event) {
    event.when(
      loading: (trackKey) {
        state = state.copyWith(
          status: LyricsStatus.loading,
          trackKey: trackKey,
          doc: null,
          currentLineIndex: -1,
          lastError: null,
        );
      },
      ready: (trackKey, doc) {
        state = state.copyWith(
          status: LyricsStatus.ready,
          trackKey: trackKey,
          doc: doc,
          currentLineIndex: -1,
          lastError: null,
        );
      },
      cursor: (trackKey, lineIndex) {
        if (state.trackKey != trackKey) return;
        state = state.copyWith(currentLineIndex: lineIndex.toInt());
      },
      empty: (trackKey) {
        state = state.copyWith(
          status: LyricsStatus.empty,
          trackKey: trackKey,
          doc: null,
          currentLineIndex: -1,
          lastError: null,
        );
      },
      error: (trackKey, message) {
        state = state.copyWith(
          status: LyricsStatus.error,
          trackKey: trackKey,
          doc: null,
          currentLineIndex: -1,
          lastError: message,
        );
      },
    );
  }

  static String _resolveTitle(QueueItem item) {
    final title = item.title?.trim() ?? '';
    if (title.isNotEmpty) return title;
    final name = item.displayTitle.trim();
    if (name.isNotEmpty) return name;
    return item.path.trim();
  }

  static String? _trimOrNull(String? value) {
    final v = value?.trim() ?? '';
    return v.isEmpty ? null : v;
  }
}
