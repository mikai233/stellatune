import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/player/playback_models.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

final playbackControllerProvider =
    NotifierProvider<PlaybackController, PlaybackState>(PlaybackController.new);

class PlaybackController extends Notifier<PlaybackState> {
  static const DlnaBridge _dlna = DlnaBridge();

  StreamSubscription<Event>? _sub;
  Timer? _volumePersistDebounce;
  Timer? _resumePersistTimer;
  TrackRef? _resumePendingTrack;
  int _resumePendingPositionMs = 0;
  double _lastNonZeroVolume = 1.0;
  String? _dlnaLastPath;
  Timer? _dlnaPollTimer;
  bool _dlnaPollInFlight = false;
  String? _dlnaLastTransportState;
  DateTime? _dlnaSuppressAutoNextUntil;
  DateTime? _dlnaLastPlayStartedAt;
  bool _reportedNoDlnaVolume = false;
  int _dlnaVolumeMismatchCount = 0;
  int? _dlnaLastReportedDlnaVolume;
  bool _dlnaVolumeUnsupported = false;
  String? _lastPreloadedNextTrackKey;

  @override
  PlaybackState build() {
    unawaited(_sub?.cancel());
    _volumePersistDebounce?.cancel();
    _volumePersistDebounce = null;
    _resumePersistTimer?.cancel();
    _resumePersistTimer = null;
    _resumePendingTrack = null;
    _dlnaPollTimer?.cancel();
    _dlnaPollTimer = null;
    _dlnaPollInFlight = false;
    _dlnaLastTransportState = null;
    _dlnaSuppressAutoNextUntil = null;
    _dlnaLastPlayStartedAt = null;
    _lastPreloadedNextTrackKey = null;

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

    ref.onDispose(() {
      unawaited(_sub?.cancel());
      _volumePersistDebounce?.cancel();
      _resumePersistTimer?.cancel();
      _dlnaPollTimer?.cancel();
    });

    final savedVolume = ref.read(settingsStoreProvider).volume.clamp(0.0, 1.0);
    if (savedVolume > 0) {
      _lastNonZeroVolume = savedVolume;
    }

    ref.listen<DlnaRenderer?>(dlnaSelectedRendererProvider, (prev, next) {
      unawaited(_onOutputChanged(prev, next));
    });

    if (!_dlnaActive) {
      unawaited(bridge.setVolume(savedVolume));
    } else {
      _ensureDlnaPoller();
    }

    // Defer resume restoration to avoid mutating other providers during build.
    unawaited(Future<void>.microtask(_restoreResume));
    return const PlaybackState.initial().copyWith(volume: savedVolume);
  }

  bool get _dlnaActive =>
      ref.read(dlnaSelectedRendererProvider)?.avTransportControlUrl != null;

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

    // Repeat-all with shuffle rebuilds order dynamically; skip preload to avoid wrong guesses.
    if (queue.shuffle) {
      return null;
    }

    return queue.items.first;
  }

  Future<void> _requestPreloadNext() async {
    if (_dlnaActive) {
      _lastPreloadedNextTrackKey = null;
      return;
    }

    final queue = ref.read(queueControllerProvider);
    final nextItem = _peekNextQueueItem(queue);
    final currentTrackKey = queue.currentItem?.stableTrackKey.trim();
    final nextTrackKey = nextItem?.stableTrackKey.trim();

    if (nextTrackKey == null ||
        nextTrackKey.isEmpty ||
        nextTrackKey == currentTrackKey ||
        nextItem == null) {
      _lastPreloadedNextTrackKey = null;
      return;
    }
    if (_lastPreloadedNextTrackKey == nextTrackKey) {
      return;
    }

    _lastPreloadedNextTrackKey = nextTrackKey;
    try {
      await ref
          .read(playerBridgeProvider)
          .preloadTrackRef(nextItem.track, positionMs: 0);
    } catch (e) {
      // Best-effort optimization; ignore failures to avoid affecting playback flow.
      ref.read(loggerProvider).d('preload next failed: $e');
      _lastPreloadedNextTrackKey = null;
    }
  }

  Future<void> _restoreResume() async {
    if (_dlnaActive) return;

    final settings = ref.read(settingsStoreProvider);
    final track = settings.resumeTrack;
    if (track == null) return;
    final pos = settings.resumePositionMs.clamp(0, 1 << 31);

    // Try to restore the full queue first.
    final restoredQueue = await _restoreQueue();

    // Ensure the UI shows a sensible "current track" even before any user action.
    if (!restoredQueue) {
      final queue = ref.read(queueControllerProvider);
      if (queue.items.isEmpty) {
        ref.read(queueControllerProvider.notifier).setQueue([
          QueueItem(
            track: track,
            id: settings.resumeTrackId,
            title: settings.resumeTitle,
            artist: settings.resumeArtist,
            album: settings.resumeAlbum,
            durationMs: settings.resumeDurationMs,
          ),
        ], startIndex: 0);
      }
    }

    final bridge = ref.read(playerBridgeProvider);
    try {
      await bridge.loadTrackRef(track);
      if (pos > 0) {
        await bridge.seekMs(pos);
      }
      state = state.copyWith(
        currentPath: track.locator,
        positionMs: pos,
        lastError: null,
      );
      unawaited(
        bridge.preloadTrackRef(track, positionMs: pos).catchError((Object e) {
          ref.read(loggerProvider).d('resume preload failed: $e');
        }),
      );
    } catch (e) {
      ref.read(loggerProvider).w('resume failed: $e');
    }
  }

  Future<bool> _restoreQueue() async {
    final settings = ref.read(settingsStoreProvider);
    final source = settings.queueSource;
    final resumeTrack = settings.resumeTrack;
    final logger = ref.read(loggerProvider);

    if (source == null || resumeTrack == null) {
      logger.d(
        'restore queue skipped: source=$source, resumeTrack=$resumeTrack',
      );
      return false;
    }

    logger.d(
      'restoring queue from source: ${source.type.name} (label: ${source.label})',
    );

    final bridge = ref.read(libraryBridgeProvider);
    final completer = Completer<List<QueueItem>?>();

    StreamSubscription? sub;
    sub = bridge.events().listen((event) {
      if (completer.isCompleted) return;
      event.maybeWhen(
        tracks: (folder, recursive, query, items) {
          final isFolderMatch =
              source.type == QueueSourceType.folder &&
              source.folderPath == folder &&
              source.includeSubfolders == recursive;
          final isAllMatch =
              source.type == QueueSourceType.all &&
              folder.isEmpty &&
              recursive == true;

          if ((isFolderMatch || isAllMatch) && query.isEmpty) {
            logger.d('restore queue: received ${items.length} tracks');
            completer.complete(
              items
                  .map(
                    (t) => QueueItem(
                      track: _localTrackRef(t.path),
                      id: t.id.toInt() >= 0 ? t.id.toInt() : null,
                      title: t.title,
                      artist: t.artist,
                      album: t.album,
                      durationMs: t.durationMs?.toInt(),
                    ),
                  )
                  .toList(),
            );
          }
        },
        playlistTracks: (playlistId, query, items) {
          if (source.type == QueueSourceType.playlist &&
              source.playlistId == playlistId &&
              query.isEmpty) {
            logger.d('restore queue: received ${items.length} playlist tracks');
            completer.complete(
              items
                  .map(
                    (t) => QueueItem(
                      track: _localTrackRef(t.path),
                      id: t.id.toInt() >= 0 ? t.id.toInt() : null,
                      title: t.title,
                      artist: t.artist,
                      album: t.album,
                      durationMs: t.durationMs?.toInt(),
                    ),
                  )
                  .toList(),
            );
          }
        },
        error: (msg) {
          logger.e('restore queue error event: $msg');
          if (!completer.isCompleted) completer.complete(null);
        },
        orElse: () {},
      );
    });

    try {
      if (source.type == QueueSourceType.folder) {
        await bridge.listTracks(
          folder: source.folderPath ?? '',
          recursive: source.includeSubfolders,
          query: '',
        );
      } else if (source.type == QueueSourceType.playlist) {
        await bridge.listPlaylistTracks(
          playlistId: source.playlistId ?? 0,
          query: '',
        );
      } else if (source.type == QueueSourceType.all) {
        await bridge.listTracks(folder: '', recursive: true, query: '');
      }

      final items = await completer.future.timeout(
        const Duration(seconds: 10),
        onTimeout: () {
          logger.w('restore queue timed out waiting for events');
          return null;
        },
      );
      if (items == null || items.isEmpty) {
        logger.d('restore queue failed: no items');
        return false;
      }

      // Find the index of the resume track.
      int startIndex = -1;
      final resumeKey = '${resumeTrack.sourceId}:${resumeTrack.trackId}';
      for (var i = 0; i < items.length; i++) {
        if (items[i].stableTrackKey == resumeKey) {
          startIndex = i;
          break;
        }
      }

      if (startIndex == -1) {
        logger.d('restore queue failed: track $resumeKey not in list');
        return false;
      }

      logger.d(
        'restore queue success: items=${items.length}, start=$startIndex',
      );
      ref
          .read(queueControllerProvider.notifier)
          .setQueue(items, startIndex: startIndex, source: source);
      return true;
    } catch (e) {
      logger.e('restore queue failed with exception', error: e);
      return false;
    } finally {
      unawaited(sub.cancel());
    }
  }

  TrackRef? _resolveCurrentTrackForResume() {
    final queueItem = ref.read(queueControllerProvider).currentItem;
    if (queueItem != null) return queueItem.track;
    final path = state.currentPath?.trim() ?? '';
    if (path.isEmpty) return null;
    return _localTrackRef(path);
  }

  void _scheduleResumePersist(TrackRef track, int positionMs) {
    _resumePendingTrack = track;
    _resumePendingPositionMs = positionMs;
    if (_resumePersistTimer != null) return;
    _resumePersistTimer = Timer(const Duration(seconds: 1), () {
      _resumePersistTimer = null;
      final track = _resumePendingTrack;
      if (track == null) return;
      final ms = _resumePendingPositionMs.clamp(0, 1 << 31);

      final currentItem = ref.read(queueControllerProvider).currentItem;
      unawaited(
        ref
            .read(settingsStoreProvider)
            .setResume(
              track: track,
              positionMs: ms,
              trackId: currentItem?.id,
              title: currentItem?.title,
              artist: currentItem?.artist,
              album: currentItem?.album,
              durationMs: currentItem?.durationMs,
            ),
      );
    });
  }

  Future<void> _persistResumeNow({
    required TrackRef track,
    required int positionMs,
  }) async {
    final locator = track.locator.trim();
    if (locator.isEmpty) return;
    final ms = positionMs.clamp(0, 1 << 31);

    final currentItem = ref.read(queueControllerProvider).currentItem;
    await ref
        .read(settingsStoreProvider)
        .setResume(
          track: track,
          positionMs: ms,
          trackId: currentItem?.id,
          title: currentItem?.title,
          artist: currentItem?.artist,
          album: currentItem?.album,
          durationMs: currentItem?.durationMs,
        );
  }

  Future<void> seekMs(int positionMs) async {
    final pos = positionMs.clamp(0, 1 << 31);
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).seekMs(pos);
      // Optimistically update the UI; engine events will resync shortly.
      state = state.copyWith(positionMs: pos, lastError: null);
      final track = _resolveCurrentTrackForResume();
      if (track != null) {
        unawaited(_persistResumeNow(track: track, positionMs: pos));
      }
      return;
    }

    final renderer = ref.read(dlnaSelectedRendererProvider);
    final controlUrl = renderer?.avTransportControlUrl;
    if (renderer == null || controlUrl == null) return;

    _dlnaSuppressAutoNext(const Duration(seconds: 2));
    await _dlna.avTransportSeekMs(
      controlUrl: controlUrl,
      serviceType: renderer.avTransportServiceType,
      positionMs: pos,
    );
    state = state.copyWith(positionMs: pos, lastError: null);
    _ensureDlnaPoller();

    final track = _resolveCurrentTrackForResume();
    if (track != null) {
      unawaited(_persistResumeNow(track: track, positionMs: pos));
    }
  }

  void _ensureDlnaPoller() {
    if (!_dlnaActive) {
      _dlnaPollTimer?.cancel();
      _dlnaPollTimer = null;
      return;
    }
    _dlnaPollTimer ??= Timer.periodic(const Duration(milliseconds: 600), (_) {
      unawaited(_pollDlna());
    });
  }

  void _dlnaSuppressAutoNext([Duration duration = const Duration(seconds: 2)]) {
    _dlnaSuppressAutoNextUntil = DateTime.now().add(duration);
  }

  PlayerState _playerStateFromDlna(String s) {
    switch (s.trim().toUpperCase()) {
      case 'PLAYING':
        return PlayerState.playing;
      case 'PAUSED_PLAYBACK':
      case 'PAUSED_RECORDING':
        return PlayerState.paused;
      case 'TRANSITIONING':
        return PlayerState.buffering;
      case 'STOPPED':
      case 'NO_MEDIA_PRESENT':
        return PlayerState.stopped;
    }
    return state.playerState;
  }

  Future<void> _pollDlna() async {
    if (!_dlnaActive) return;
    if (_dlnaPollInFlight) return;
    _dlnaPollInFlight = true;
    try {
      final renderer = ref.read(dlnaSelectedRendererProvider);
      final controlUrl = renderer?.avTransportControlUrl;
      if (renderer == null || controlUrl == null) return;

      final info = await _dlna.avTransportGetTransportInfo(
        controlUrl: controlUrl,
        serviceType: renderer.avTransportServiceType,
      );
      final pos = await _dlna.avTransportGetPositionInfo(
        controlUrl: controlUrl,
        serviceType: renderer.avTransportServiceType,
      );

      final transportState = info.currentTransportState.trim().toUpperCase();
      final prev = _dlnaLastTransportState;
      _dlnaLastTransportState = transportState;

      final mapped = _playerStateFromDlna(transportState);
      final relMs = pos.relTimeMs.toInt();
      if (mapped != state.playerState || relMs != state.positionMs) {
        state = state.copyWith(playerState: mapped, positionMs: relMs);
      }

      // Detect track end and follow play mode (next/repeat/shuffle).
      final now = DateTime.now();
      final suppressUntil = _dlnaSuppressAutoNextUntil;
      if (suppressUntil != null && now.isBefore(suppressUntil)) return;

      final startedAt = _dlnaLastPlayStartedAt;
      final startedOk =
          startedAt != null && now.difference(startedAt).inMilliseconds >= 1500;
      if (!startedOk) return;

      final endedState =
          transportState == 'STOPPED' || transportState == 'NO_MEDIA_PRESENT';
      final transitionedFromPlaying =
          prev == 'PLAYING' || prev == 'TRANSITIONING';
      if (!endedState || !transitionedFromPlaying) return;

      final currentItem = ref.read(queueControllerProvider).currentItem;
      final currentPath = currentItem?.path;
      if (currentPath == null || _dlnaLastPath != currentPath) return;

      final durationMs =
          pos.trackDurationMs?.toInt() ?? currentItem?.durationMs ?? 0;
      final nearEnd = durationMs <= 0 ? true : relMs >= durationMs - 800;
      if (!nearEnd) return;

      _dlnaSuppressAutoNext();
      unawaited(next(auto: true));
    } catch (e, st) {
      // Polling is best-effort; don't surface as UI error.
      ref.read(loggerProvider).d('dlna poll failed: $e', stackTrace: st);
    } finally {
      _dlnaPollInFlight = false;
    }
  }

  Future<void> _applyDlnaVolume(double v) async {
    final controlUrl = ref
        .read(dlnaSelectedRendererProvider)
        ?.renderingControlUrl;
    final serviceType = ref
        .read(dlnaSelectedRendererProvider)
        ?.renderingControlServiceType;
    if (_dlnaVolumeUnsupported) return;
    if (controlUrl == null) {
      if (!_reportedNoDlnaVolume) {
        _reportedNoDlnaVolume = true;
        ref.read(loggerProvider).w('dlna renderer has no RenderingControl URL');
        state = state.copyWith(
          lastError: 'DLNA device does not support volume',
        );
      }
      return;
    }

    final vv = (v.clamp(0.0, 1.0) * 100).round().clamp(0, 100);
    try {
      if (vv <= 0) {
        // Many renderers keep audible output even with volume=0; mute is more reliable.
        await _dlna.renderingControlSetMute(
          controlUrl: controlUrl,
          serviceType: serviceType,
          mute: true,
        );
      } else {
        // Ensure unmuted before setting audible volume.
        await _dlna.renderingControlSetMute(
          controlUrl: controlUrl,
          serviceType: serviceType,
          mute: false,
        );
      }
      await _dlna.renderingControlSetVolume(
        controlUrl: controlUrl,
        serviceType: serviceType,
        volume0To100: vv,
      );

      // Best-effort verification; some devices ignore SetVolume but still return 200.
      final current = await _dlna.renderingControlGetVolume(
        controlUrl: controlUrl,
        serviceType: serviceType,
      );
      if ((current - vv).abs() >= 5) {
        if (_dlnaLastReportedDlnaVolume == current) {
          _dlnaVolumeMismatchCount++;
        } else {
          _dlnaVolumeMismatchCount = 1;
          _dlnaLastReportedDlnaVolume = current;
        }
        ref
            .read(loggerProvider)
            .w('dlna volume mismatch: requested=$vv current=$current');
        if (_dlnaVolumeMismatchCount >= 3) {
          _dlnaVolumeUnsupported = true;
          state = state.copyWith(
            lastError: 'DLNA device ignores volume control',
          );
        }
      }
    } catch (e, st) {
      ref
          .read(loggerProvider)
          .e('dlna set volume failed: $e', error: e, stackTrace: st);
      state = state.copyWith(lastError: 'DLNA volume failed: $e');
    }
  }

  Future<void> _onOutputChanged(DlnaRenderer? prev, DlnaRenderer? next) async {
    if (prev?.usn == next?.usn) return;

    final wasPlaying =
        state.playerState == PlayerState.playing ||
        state.playerState == PlayerState.buffering;
    final currentItem = ref.read(queueControllerProvider).currentItem;
    _dlnaSuppressAutoNext();

    // Stop whichever output was previously active.
    if (prev?.avTransportControlUrl != null) {
      try {
        await _dlna.avTransportStop(
          controlUrl: prev!.avTransportControlUrl!,
          serviceType: prev.avTransportServiceType,
        );
      } catch (e, s) {
        ref
            .read(loggerProvider)
            .w(
              'failed to stop DLNA transport during output change',
              error: e,
              stackTrace: s,
            );
      }
      try {
        await _dlna.httpUnpublishAll();
      } catch (e, s) {
        ref
            .read(loggerProvider)
            .w(
              'failed to unpublish DLNA HTTP services during output change',
              error: e,
              stackTrace: s,
            );
      }
      _dlnaLastPath = null;
      state = state.copyWith(playerState: PlayerState.stopped, positionMs: 0);
    }

    if (next?.avTransportControlUrl != null) {
      // Switching to DLNA: stop local engine to avoid double playback.
      await ref.read(playerBridgeProvider).stop();
      _reportedNoDlnaVolume = false;
      _dlnaVolumeMismatchCount = 0;
      _dlnaLastReportedDlnaVolume = null;
      _dlnaVolumeUnsupported = false;
      // Clear any local-engine error (e.g. "no track loaded") that is irrelevant to DLNA output.
      state = state.copyWith(lastError: null);
      _ensureDlnaPoller();
    } else {
      // Switching to local: stop DLNA if we can.
      final prevUrl = prev?.avTransportControlUrl;
      if (prevUrl != null) {
        try {
          await _dlna.avTransportStop(
            controlUrl: prevUrl,
            serviceType: prev?.avTransportServiceType,
          );
        } catch (e, s) {
          ref
              .read(loggerProvider)
              .w(
                'failed to stop DLNA transport during output change',
                error: e,
                stackTrace: s,
              );
        }
      }
    }

    if (!wasPlaying || currentItem == null) return;
    await _loadAndPlayQueueItem(currentItem);
  }

  Future<void> setQueueAndPlay(
    List<String> paths, {
    int startIndex = 0,
    QueueSource? source,
  }) => setQueueAndPlayTracks(
    paths.map((p) => TrackLite(id: -1, path: p)).toList(),
    startIndex: startIndex,
    source: source,
  );

  Future<void> setQueueAndPlayItems(
    List<QueueItem> items, {
    int startIndex = 0,
    QueueSource? source,
  }) async {
    if (items.isEmpty) return;
    ref
        .read(queueControllerProvider.notifier)
        .setQueue(items, startIndex: startIndex, source: source);
    final item = ref.read(queueControllerProvider).currentItem;
    if (item == null) return;
    unawaited(_requestPreloadNext());
    await _loadAndPlayQueueItem(item);
  }

  Future<void> setQueueAndPlayTracks(
    List<TrackLite> tracks, {
    int startIndex = 0,
    QueueSource? source,
  }) => setQueueAndPlayItems(
    tracks
        .map(
          (t) => QueueItem(
            track: _localTrackRef(t.path),
            id: t.id.toInt() >= 0 ? t.id.toInt() : null,
            title: t.title,
            artist: t.artist,
            album: t.album,
            durationMs: t.durationMs?.toInt(),
          ),
        )
        .toList(),
    startIndex: startIndex,
    source: source,
  );

  Future<void> enqueueItems(List<QueueItem> items) async {
    if (items.isEmpty) return;
    final queue = ref.read(queueControllerProvider);
    ref.read(queueControllerProvider.notifier).enqueue(items);
    unawaited(_requestPreloadNext());

    // If nothing is loaded yet, start playing immediately from the first enqueued item.
    if (queue.currentItem == null && items.isNotEmpty) {
      await _loadAndPlayQueueItem(items.first);
    }
  }

  Future<void> enqueueTracks(List<TrackLite> tracks) => enqueueItems(
    tracks
        .map(
          (t) => QueueItem(
            track: _localTrackRef(t.path),
            id: t.id.toInt() >= 0 ? t.id.toInt() : null,
            title: t.title,
            artist: t.artist,
            album: t.album,
            durationMs: t.durationMs?.toInt(),
          ),
        )
        .toList(),
  );

  Future<void> enqueue(List<String> paths) =>
      enqueueTracks(paths.map((p) => TrackLite(id: -1, path: p)).toList());

  Future<void> playIndex(int index) async {
    _dlnaSuppressAutoNext();
    ref.read(queueControllerProvider.notifier).selectIndex(index);
    final item = ref.read(queueControllerProvider).currentItem;
    if (item == null) return;
    unawaited(_requestPreloadNext());
    await _loadAndPlayQueueItem(item);
  }

  Future<void> play() async {
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).play();
      return;
    }

    final renderer = ref.read(dlnaSelectedRendererProvider);
    final controlUrl = renderer?.avTransportControlUrl;
    if (renderer == null || controlUrl == null) return;

    final currentItem = ref.read(queueControllerProvider).currentItem;
    final path = currentItem?.path;
    if (currentItem == null || path == null) return;

    if (_dlnaLastPath == path) {
      await _dlna.avTransportPlay(
        controlUrl: controlUrl,
        serviceType: renderer.avTransportServiceType,
      );
      _dlnaLastPlayStartedAt = DateTime.now();
      _ensureDlnaPoller();
      state = state.copyWith(
        playerState: PlayerState.playing,
        currentPath: path,
        lastError: null,
      );
      return;
    }

    await _loadAndPlayQueueItem(currentItem);
  }

  Future<void> pause() async {
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).pause();
      return;
    }

    final controlUrl = ref
        .read(dlnaSelectedRendererProvider)
        ?.avTransportControlUrl;
    if (controlUrl == null) return;
    await _dlna.avTransportPause(
      controlUrl: controlUrl,
      serviceType: ref
          .read(dlnaSelectedRendererProvider)
          ?.avTransportServiceType,
    );
    _dlnaSuppressAutoNext();
    state = state.copyWith(playerState: PlayerState.paused, lastError: null);
  }

  void setVolume(double volume) {
    final v = volume.clamp(0.0, 1.0);
    if (state.volume == v) return;
    state = state.copyWith(volume: v);
    if (v > 0) {
      _lastNonZeroVolume = v;
    }

    // No throttling for audio: keep loudness in sync with the slider.
    if (_dlnaActive) {
      unawaited(_applyDlnaVolume(v));
    } else {
      unawaited(ref.read(playerBridgeProvider).setVolume(v));
    }

    // Debounce persistence only (doesn't affect loudness).
    _volumePersistDebounce?.cancel();
    _volumePersistDebounce = Timer(const Duration(milliseconds: 250), () {
      unawaited(ref.read(settingsStoreProvider).setVolume(v));
    });
  }

  void toggleMute() {
    if (state.volume > 0) {
      _lastNonZeroVolume = state.volume;
      setVolume(0);
      return;
    }
    final restore = _lastNonZeroVolume.clamp(0.0, 1.0);
    setVolume(restore > 0 ? restore : 1.0);
  }

  Future<void> stop() async {
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).stop();
      state = state.copyWith(positionMs: 0);
      _lastPreloadedNextTrackKey = null;
      final track = _resolveCurrentTrackForResume();
      if (track != null) {
        unawaited(_persistResumeNow(track: track, positionMs: 0));
      }
      return;
    }

    _dlnaSuppressAutoNext();
    final controlUrl = ref
        .read(dlnaSelectedRendererProvider)
        ?.avTransportControlUrl;
    if (controlUrl != null) {
      await _dlna.avTransportStop(
        controlUrl: controlUrl,
        serviceType: ref
            .read(dlnaSelectedRendererProvider)
            ?.avTransportServiceType,
      );
    }
    unawaited(_dlna.httpUnpublishAll());
    _dlnaLastPath = null;
    state = state.copyWith(
      playerState: PlayerState.stopped,
      positionMs: 0,
      lastError: null,
    );

    final track = _resolveCurrentTrackForResume();
    if (track != null) {
      unawaited(_persistResumeNow(track: track, positionMs: 0));
    }
  }

  Future<void> next({bool auto = false}) async {
    _dlnaSuppressAutoNext(const Duration(seconds: 1));
    final item = ref
        .read(queueControllerProvider.notifier)
        .next(fromAuto: auto);
    if (item == null) {
      await stop();
      return;
    }
    unawaited(_requestPreloadNext());
    await _loadAndPlayQueueItem(item);
  }

  Future<void> previous() async {
    _dlnaSuppressAutoNext(const Duration(seconds: 1));
    final item = ref.read(queueControllerProvider.notifier).previous();
    if (item == null) return;
    unawaited(_requestPreloadNext());
    await _loadAndPlayQueueItem(item);
  }

  Future<void> _updateTrackInfo() async {
    if (_dlnaActive) return;
    try {
      final info = await ref.read(playerBridgeProvider).currentTrackInfo();
      state = state.copyWith(trackInfo: info);
    } catch (e) {
      ref.read(loggerProvider).d('fetch track info failed: $e');
    }
  }

  TrackRef _localTrackRef(String path) =>
      TrackRef(sourceId: 'local', trackId: path, locator: path);

  Future<void> _loadAndPlayQueueItem(QueueItem item) async {
    final path = item.path;
    state = state.copyWith(lastError: null, lastLog: '');
    if (_dlnaActive) {
      if (item.track.sourceId.toLowerCase() != 'local') {
        state = state.copyWith(
          lastError: 'DLNA output currently only supports local tracks',
        );
        return;
      }
      final renderer = ref.read(dlnaSelectedRendererProvider);
      if (renderer == null) return;
      final coverPath = item.id == null
          ? null
          : p.join(ref.read(coverDirProvider), item.id.toString());
      final coverExists = coverPath != null && File(coverPath).existsSync();
      await ref.read(playerBridgeProvider).stop();
      await _dlna.playLocalTrack(
        renderer: renderer,
        path: path,
        title: item.title,
        artist: item.artist,
        album: item.album,
        coverPath: coverExists ? coverPath : null,
      );
      _dlnaLastPath = path;
      _dlnaLastPlayStartedAt = DateTime.now();
      _ensureDlnaPoller();
      state = state.copyWith(
        currentPath: path,
        positionMs: 0,
        playerState: PlayerState.playing,
      );
      return;
    }

    final bridge = ref.read(playerBridgeProvider);
    await bridge.loadTrackRef(item.track);
    unawaited(_updateTrackInfo());
    await bridge.play();
  }

  void _onEvent(Event event) {
    if (_dlnaActive) return;
    event.when(
      stateChanged: (s) {
        state = state.copyWith(playerState: s);
      },
      position: (ms) {
        state = state.copyWith(positionMs: ms);
        final track = _resolveCurrentTrackForResume();
        if (track != null) {
          _scheduleResumePersist(track, ms);
        }
      },
      trackChanged: (path) {
        state = state.copyWith(currentPath: path);
        unawaited(
          _persistResumeNow(
            track: _resolveCurrentTrackForResume() ?? _localTrackRef(path),
            positionMs: 0,
          ),
        );
        unawaited(_updateTrackInfo());
        unawaited(_requestPreloadNext());
      },
      playbackEnded: (path) {
        ref.read(loggerProvider).i('playback ended: $path');
        unawaited(next(auto: true));
      },
      volumeChanged: (volume) {
        state = state.copyWith(volume: volume);
        if (volume > 0) {
          _lastNonZeroVolume = volume.clamp(0.0, 1.0);
        }
      },
      error: (message) {
        ref.read(loggerProvider).e(message);
        state = state.copyWith(lastError: message);
      },
      log: (message) {
        ref.read(loggerProvider).d(message);
        state = state.copyWith(lastLog: message);
      },
      outputDevicesChanged: (devices) {
        // Handled by SettingsPage, but must be present for exhaustive matching.
        ref.read(loggerProvider).d('output devices updated: ${devices.length}');
      },
    );
  }
}
