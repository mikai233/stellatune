import 'queue_identity_resolver.dart';

import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/player/decoder_extension_support.dart';
import 'package:stellatune/player/playback_playability_utils.dart';
import 'package:stellatune/player/playback_models.dart';
import 'package:stellatune/player/playability_messages.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/player/playback_resume_queue_utils.dart';
import 'package:stellatune/platform/directory_access_service.dart';

final playbackControllerProvider =
    NotifierProvider<PlaybackController, PlaybackState>(PlaybackController.new);

class PlaybackController extends Notifier<PlaybackState> {
  static const DlnaBridge _dlna = DlnaBridge();
  static const int _volumeRampMs = 6;

  StreamSubscription<Event>? _sub;
  Timer? _volumePersistDebounce;
  BigInt? _currentTrackId;
  double _lastNonZeroVolume = 1.0;
  int _nextVolumeSeq = 1;
  int _latestVolumeCommandSeq = 0;
  int _latestVolumeAckSeq = 0;
  DirectoryAccessLease? _activeDlnaTrackLease;
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
  BigInt? _activePositionItemId;
  BigInt? _activePositionSessionId;

  @override
  PlaybackState build() {
    unawaited(_sub?.cancel());
    _volumePersistDebounce?.cancel();
    _volumePersistDebounce = null;
    _currentTrackId = null;
    unawaited(_releaseDlnaTrackLease());
    _dlnaPollTimer?.cancel();
    _dlnaPollTimer = null;
    _dlnaPollInFlight = false;
    _dlnaLastTransportState = null;
    _dlnaSuppressAutoNextUntil = null;
    _dlnaLastPlayStartedAt = null;
    _activePositionItemId = null;
    _activePositionSessionId = null;
    _nextVolumeSeq = 1;
    _latestVolumeCommandSeq = 0;
    _latestVolumeAckSeq = 0;

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
      _dlnaPollTimer?.cancel();
      unawaited(_releaseDlnaTrackLease());
    });

    final savedVolume = ref.read(settingsStoreProvider).volume.clamp(0.0, 1.0);
    if (savedVolume > 0) {
      _lastNonZeroVolume = savedVolume;
    }

    ref.listen<DlnaRenderer?>(dlnaSelectedRendererProvider, (prev, next) {
      unawaited(_onOutputChanged(prev, next));
    });

    if (!_dlnaActive) {
      final seq = _nextVolumeSeq++;
      _latestVolumeCommandSeq = seq;
      unawaited(bridge.setVolume(savedVolume, seq: seq, rampMs: 0));
    } else {
      _ensureDlnaPoller();
    }
    unawaited(_refreshDecoderExtensionSupport());

    // The native PlaybackStateStore is the only playback-resume fact source.
    // Defer its snapshot projection to avoid mutating other providers during build.
    unawaited(Future<void>.microtask(_restoreBackendSnapshot));
    return const PlaybackState.initial().copyWith(
      desiredVolume: savedVolume,
      appliedVolume: savedVolume,
    );
  }

  bool get _dlnaActive =>
      ref.read(dlnaSelectedRendererProvider)?.avTransportControlUrl != null;

  Future<void> _refreshBackendQueue({BigInt? currentItemId}) async {
    if (_dlnaActive) return;
    try {
      final bridge = ref.read(playerBridgeProvider);
      final snapshot = await bridge.playbackQueue();
      ref
          .read(queueControllerProvider.notifier)
          .applyBackend(snapshot, preserveCurrent: true);
      if (currentItemId != null && _activePositionItemId == currentItemId) {
        ref
            .read(queueControllerProvider.notifier)
            .observeCurrent(currentItemId);
      }
      final items = ref.read(queueControllerProvider).items;
      await bridge.retainQueuePaths(items.map((item) => item.path));
      await bridge.releaseRemovedQueuePaths(items.map((item) => item.path));
    } catch (error) {
      ref.read(loggerProvider).w('queue refresh failed: $error');
    }
  }

  Future<void> _restoreBackendSnapshot() async {
    if (_dlnaActive) return;
    try {
      final snapshot = await ref.read(playerBridgeProvider).playbackSnapshot();
      final track = snapshot.trackId;
      final queue = await ref.read(playerBridgeProvider).playbackQueue();
      ref.read(queueControllerProvider.notifier).applyBackend(queue);
      if (snapshot.itemId != null) {
        ref
            .read(queueControllerProvider.notifier)
            .observeCurrent(snapshot.itemId!);
      }
      await ref
          .read(playerBridgeProvider)
          .retainQueuePaths(queue.items.map((item) => item.localPath ?? ''));
      if (track == null) return;
      _currentTrackId = track;
      _activePositionItemId = snapshot.itemId;
      state = state.copyWith(
        positionMs: snapshot.positionMs.toInt().clamp(0, 1 << 31),
        playerState: snapshot.state,
        lastError: null,
      );
      unawaited(_refreshBackendQueue());
    } catch (e) {
      ref.read(loggerProvider).w('backend playback snapshot failed: $e');
    }
  }

  Future<void> seekMs(int positionMs) async {
    final pos = positionMs.clamp(0, 1 << 31);
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).seekMs(pos);
      // Optimistically update the UI; engine events will resync shortly.
      state = state.copyWith(positionMs: pos, lastError: null);
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
      _applyDlnaPolledState(mapped: mapped, positionMs: relMs);
      final currentItem = ref.read(queueControllerProvider).currentItem;
      final currentPath = currentItem?.path;
      final durationMs =
          pos.trackDurationMs?.toInt() ?? currentItem?.durationMs ?? 0;
      final shouldAutoAdvance = _shouldAutoAdvanceAfterDlnaPoll(
        now: DateTime.now(),
        transportState: transportState,
        previousTransportState: prev,
        currentPath: currentPath,
        relMs: relMs,
        durationMs: durationMs,
      );
      if (!shouldAutoAdvance) return;

      _dlnaSuppressAutoNext();
      unawaited(next(auto: true));
    } catch (e, st) {
      // Polling is best-effort; don't surface as UI error.
      ref.read(loggerProvider).d('dlna poll failed: $e', stackTrace: st);
    } finally {
      _dlnaPollInFlight = false;
    }
  }

  void _applyDlnaPolledState({
    required PlayerState mapped,
    required int positionMs,
  }) {
    if (mapped == state.playerState && positionMs == state.positionMs) {
      return;
    }
    state = state.copyWith(playerState: mapped, positionMs: positionMs);
  }

  bool _shouldAutoAdvanceAfterDlnaPoll({
    required DateTime now,
    required String transportState,
    required String? previousTransportState,
    required String? currentPath,
    required int relMs,
    required int durationMs,
  }) {
    final suppressUntil = _dlnaSuppressAutoNextUntil;
    if (suppressUntil != null && now.isBefore(suppressUntil)) {
      return false;
    }

    final startedAt = _dlnaLastPlayStartedAt;
    final startedOk =
        startedAt != null && now.difference(startedAt).inMilliseconds >= 1500;
    if (!startedOk) {
      return false;
    }

    final endedState =
        transportState == 'STOPPED' || transportState == 'NO_MEDIA_PRESENT';
    final transitionedFromPlaying =
        previousTransportState == 'PLAYING' ||
        previousTransportState == 'TRANSITIONING';
    if (!endedState || !transitionedFromPlaying) {
      return false;
    }

    if (currentPath == null || _dlnaLastPath != currentPath) {
      return false;
    }

    final nearEnd = durationMs <= 0 ? true : relMs >= durationMs - 800;
    return nearEnd;
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
      await _releaseDlnaTrackLease();
      state = state.copyWith(playerState: PlayerState.stopped, positionMs: 0);
    }

    if (next?.avTransportControlUrl != null) {
      // Switching to DLNA: stop local engine to avoid double playback.
      await ref.read(playerBridgeProvider).stop();
      await _releaseDlnaTrackLease();
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
    await _loadQueueItemOrStop(currentItem);
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
    if (!_dlnaActive) {
      try {
        final bridge = ref.read(playerBridgeProvider);
        await bridge.retainQueuePaths(items.map((item) => item.path));
        final ids = await _resolveTrackIds(items);
        final snapshot = await bridge.replaceQueue(ids);
        ref
            .read(queueControllerProvider.notifier)
            .applyBackend(
              snapshot,
              metadata: items,
              source: source,
              replaceSource: true,
            );
        await ref.read(settingsStoreProvider.notifier).setQueueSource(source);
        final mode = ref.read(settingsStoreProvider).playMode;
        ref.read(queueControllerProvider.notifier).setPlayMode(mode);
        final selected = ref
            .read(queueControllerProvider)
            .items[startIndex.clamp(0, items.length - 1)];
        await _loadQueueItemOrStop(selected);
      } catch (error) {
        ref
            .read(loggerProvider)
            .w('failed to prepare playback queue', error: error);
        state = state.copyWith(lastError: error.toString());
      }
      return;
    }
    ref
        .read(queueControllerProvider.notifier)
        .setQueue(items, startIndex: startIndex, source: source);
    final item = ref.read(queueControllerProvider).currentItem;
    if (item == null) return;
    await _loadQueueItemOrStop(item);
  }

  Future<void> setQueueAndPlayTracks(
    List<TrackLite> tracks, {
    int startIndex = 0,
    QueueSource? source,
  }) => setQueueAndPlayItems(
    PlaybackResumeQueueUtils.buildLocalQueueItems(tracks),
    startIndex: startIndex,
    source: source,
  );

  Future<void> enqueueItems(List<QueueItem> items) async {
    if (items.isEmpty) return;
    final queue = ref.read(queueControllerProvider);
    if (!_dlnaActive) {
      try {
        final bridge = ref.read(playerBridgeProvider);
        await bridge.retainQueuePaths(items.map((item) => item.path));
        final ids = await _resolveTrackIds(items);
        final snapshot = await bridge.appendQueue(ids);
        ref
            .read(queueControllerProvider.notifier)
            .applyBackend(
              snapshot,
              metadata: [...queue.items, ...items],
              preserveCurrent: true,
            );
        if (queue.items.isEmpty) {
          await _loadQueueItemOrStop(
            ref.read(queueControllerProvider).items.first,
          );
        }
      } catch (error) {
        ref
            .read(loggerProvider)
            .w('failed to prepare playback queue', error: error);
        state = state.copyWith(lastError: error.toString());
      }
      return;
    }
    ref.read(queueControllerProvider.notifier).enqueue(items);
    // If nothing is loaded yet, start playing immediately from the first enqueued item.
    if (queue.currentItem == null && items.isNotEmpty) {
      await _loadQueueItemOrStop(items.first);
    } else {
      unawaited(_refreshBackendQueue());
    }
  }

  Future<void> enqueueTracks(List<TrackLite> tracks) =>
      enqueueItems(PlaybackResumeQueueUtils.buildLocalQueueItems(tracks));

  Future<void> enqueue(List<String> paths) =>
      enqueueTracks(paths.map((p) => TrackLite(id: -1, path: p)).toList());

  Future<void> playIndex(int index) async {
    _dlnaSuppressAutoNext();
    if (!_dlnaActive) {
      final items = ref.read(queueControllerProvider).items;
      if (index >= 0 && index < items.length) {
        await _loadQueueItemOrStop(items[index]);
      }
      return;
    }
    ref.read(queueControllerProvider.notifier).selectIndex(index);
    final item = ref.read(queueControllerProvider).currentItem;
    if (item == null) return;
    await _loadQueueItemOrStop(item);
  }

  Future<bool> _loadQueueItemOrStop(QueueItem item) async {
    final loaded = await _loadAndPlayQueueItem(item);
    if (loaded) {
      unawaited(_refreshBackendQueue());
      return true;
    }

    final loadError = state.lastError;
    try {
      await stop();
    } catch (error, stackTrace) {
      ref
          .read(loggerProvider)
          .w(
            'failed to stop after track load failure',
            error: error,
            stackTrace: stackTrace,
          );
    }
    state = state.copyWith(
      playerState: PlayerState.stopped,
      audioStarted: false,
      lastError: loadError,
    );
    return false;
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

    await _loadQueueItemOrStop(currentItem);
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
    final v = volume.clamp(0.0, 1.0).toDouble();
    if (state.desiredVolume == v) return;
    state = state.copyWith(desiredVolume: v);
    if (v > 0) {
      _lastNonZeroVolume = v;
    }

    // No throttling for audio: keep loudness in sync with the slider.
    if (_dlnaActive) {
      state = state.copyWith(appliedVolume: v);
      unawaited(_applyDlnaVolume(v));
    } else {
      final seq = _nextVolumeSeq++;
      _latestVolumeCommandSeq = seq;
      unawaited(
        ref
            .read(playerBridgeProvider)
            .setVolume(v, seq: seq, rampMs: _volumeRampMs),
      );
    }

    // Debounce persistence only (doesn't affect loudness).
    _volumePersistDebounce?.cancel();
    _volumePersistDebounce = Timer(const Duration(milliseconds: 250), () {
      unawaited(ref.read(settingsStoreProvider.notifier).setVolume(v));
    });
  }

  void toggleMute() {
    if (state.desiredVolume > 0) {
      _lastNonZeroVolume = state.desiredVolume;
      setVolume(0);
      return;
    }
    final restore = _lastNonZeroVolume.clamp(0.0, 1.0);
    setVolume(restore > 0 ? restore : 1.0);
  }

  Future<void> stop() async {
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).stop();
      await _releaseDlnaTrackLease();
      state = state.copyWith(positionMs: 0);
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
    await _releaseDlnaTrackLease();
    state = state.copyWith(
      playerState: PlayerState.stopped,
      positionMs: 0,
      lastError: null,
    );
  }

  Future<void> next({bool auto = false}) async {
    if (!_dlnaActive) {
      if (auto) return;
      await ref.read(playerBridgeProvider).nextQueueItem();
      return;
    }
    _dlnaSuppressAutoNext(const Duration(seconds: 1));
    if (ref.read(queueControllerProvider).items.isEmpty) {
      ref.read(loggerProvider).w('next aborted: empty queue');
      await stop();
      return;
    }
    final item = ref
        .read(queueControllerProvider.notifier)
        .next(fromAuto: auto);
    if (item == null) {
      ref.read(loggerProvider).w('next reached end of queue: auto=$auto');
      await stop();
      return;
    }
    await _loadQueueItemOrStop(item);
  }

  Future<void> previous() async {
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).previousQueueItem();
      return;
    }
    _dlnaSuppressAutoNext(const Duration(seconds: 1));
    final item = ref.read(queueControllerProvider.notifier).previous();
    if (item == null) return;
    await _loadQueueItemOrStop(item);
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

  Future<void> _releaseDlnaTrackLease() async {
    final lease = _activeDlnaTrackLease;
    _activeDlnaTrackLease = null;
    await lease?.release();
  }

  Future<Set<String>> _loadDisabledPluginIdSet() async {
    try {
      final disabled = await ref
          .read(libraryBridgeProvider)
          .listDisabledPluginIds();
      return disabled.map((v) => v.trim()).where((v) => v.isNotEmpty).toSet();
    } catch (e, st) {
      ref
          .read(loggerProvider)
          .w('failed to load disabled plugin ids', error: e, stackTrace: st);
      return const <String>{};
    }
  }

  Future<void> _refreshDecoderExtensionSupport() async {
    try {
      await DecoderExtensionSupportCache.instance.refresh(
        ref.read(playerBridgeProvider),
      );
    } catch (e, st) {
      ref
          .read(loggerProvider)
          .d(
            'decoderSupportedExtensions refresh failed',
            error: e,
            stackTrace: st,
          );
    }
  }

  Future<String?> _playabilityBlockReason(QueueItem item) async {
    if (PlaybackPlayabilityUtils.isLocalTrack(item)) {
      final fastReason =
          PlaybackPlayabilityUtils.localTrackPlayabilityBlockReasonFast(
            item,
            DecoderExtensionSupportCache.instance.snapshotOrNull,
          );
      if (fastReason != null) {
        return fastReason;
      }
      try {
        await _refreshDecoderExtensionSupport();
      } catch (_) {
        // `_refreshDecoderExtensionSupport` already logs details.
      }
      return PlaybackPlayabilityUtils.localTrackPlayabilityBlockReasonFast(
        item,
        DecoderExtensionSupportCache.instance.snapshotOrNull,
      );
    }

    final disabledPluginIds = await _loadDisabledPluginIdSet();
    if (disabledPluginIds.isEmpty) {
      return null;
    }
    final ids = PlaybackPlayabilityUtils.extractPluginIds(item);
    final reason = PlaybackPlayabilityUtils.disabledPluginBlockReason(
      item: item,
      disabledPluginIds: disabledPluginIds,
    );
    if (reason != null) {
      ref
          .read(loggerProvider)
          .w(
            'playability blocked by disabled plugin: '
            'track=${item.stableTrackKey} '
            'source_plugin_id=${ids.sourcePluginId ?? "<none>"} '
            'decoder_plugin_id=${ids.decoderPluginId ?? "<none>"} '
            'reason=$reason',
          );
    }
    return reason;
  }

  Future<bool> _removeCurrentQueueItemIfDisabledPluginBlocked(
    QueueItem item,
    String blockedReason,
  ) async {
    if (!PlaybackPlayabilityUtils.isDisabledPluginPruneReason(blockedReason)) {
      return false;
    }
    final pluginIds = PlaybackPlayabilityUtils.trackPluginIds(item);
    if (pluginIds.isEmpty) {
      return false;
    }
    final disabledPluginIds = await _loadDisabledPluginIdSet();
    if (disabledPluginIds.isEmpty ||
        !pluginIds.any(disabledPluginIds.contains)) {
      return false;
    }

    final queue = ref.read(queueControllerProvider);
    final currentIndex = queue.currentIndex;
    if (currentIndex == null ||
        currentIndex < 0 ||
        currentIndex >= queue.items.length) {
      return false;
    }
    if (queue.items[currentIndex].stableTrackKey != item.stableTrackKey) {
      return false;
    }
    final removed = await _removeQueueIndices({currentIndex});
    if (removed > 0) {
      ref
          .read(loggerProvider)
          .i(
            'queue item pruned after plugin disable: '
            '${item.stableTrackKey}',
          );
      return true;
    }
    return false;
  }

  Future<int> removeUnplayableQueuedItemsDueToDisabledPlugins({
    String? pluginId,
  }) async {
    final queue = ref.read(queueControllerProvider);
    if (queue.items.isEmpty) return 0;

    final targetPluginId = pluginId?.trim();
    final disabledPluginIds =
        targetPluginId != null && targetPluginId.isNotEmpty
        ? <String>{targetPluginId}
        : await _loadDisabledPluginIdSet();
    if (disabledPluginIds.isEmpty) return 0;

    final candidateIndexes = <int>[];
    for (var i = 0; i < queue.items.length; i++) {
      if (i == queue.currentIndex) {
        continue;
      }
      final item = queue.items[i];
      final pluginIds = PlaybackPlayabilityUtils.trackPluginIds(item);
      if (pluginIds.isEmpty || !pluginIds.any(disabledPluginIds.contains)) {
        continue;
      }
      candidateIndexes.add(i);
    }
    if (candidateIndexes.isEmpty) return 0;

    final removed = await _removeQueueIndices(candidateIndexes.toSet());
    if (removed > 0) {
      ref
          .read(loggerProvider)
          .i(
            'queue pruned after plugin disable: '
            'removed=$removed candidates=${candidateIndexes.length}',
          );
    }
    return removed;
  }

  Future<int> _removeQueueIndices(Set<int> indices) async {
    if (_dlnaActive) {
      return ref.read(queueControllerProvider.notifier).removeIndices(indices);
    }
    final items = ref.read(queueControllerProvider).items;
    final ids = [
      for (final index in indices)
        if (index >= 0 && index < items.length && items[index].itemId != null)
          items[index].itemId!,
    ];
    final snapshot = await ref.read(playerBridgeProvider).removeQueueItems(ids);
    ref.read(queueControllerProvider.notifier).applyBackend(snapshot);
    return ids.length;
  }

  Future<List<BigInt>> _resolveTrackIds(List<QueueItem> items) {
    final bridge = ref.read(playerBridgeProvider);
    return resolveQueueTrackIds(
      items,
      ensureLocalTracks: bridge.ensureLocalTracks,
      ensureProviderTrack: (provider) => bridge.ensureProviderTrack(
        providerId: provider.providerId,
        providerKey: provider.providerKey,
        pluginId: provider.pluginId,
        typeId: provider.typeId,
        configJson: provider.configJson,
      ),
    );
  }

  Future<bool> _loadAndPlayQueueItem(QueueItem item) async {
    final path = item.path;
    state = state.copyWith(lastError: null, lastLog: '');
    final blockedReason = await _playabilityBlockReason(item);
    if (blockedReason != null) {
      await _removeCurrentQueueItemIfDisabledPluginBlocked(item, blockedReason);
      state = state.copyWith(lastError: encodePlayabilityError(blockedReason));
      return false;
    }
    if (_dlnaActive) {
      if (!PlaybackPlayabilityUtils.isLocalTrack(item)) {
        state = state.copyWith(
          lastError: 'DLNA output currently only supports local tracks',
        );
        return false;
      }
      final renderer = ref.read(dlnaSelectedRendererProvider);
      if (renderer == null) return false;
      final nextLease = await DirectoryAccessService.instance.acquireLocalPath(
        path: path,
        store: ref.read(settingsStoreServiceProvider),
      );
      final previousLease = _activeDlnaTrackLease;
      final coverPath = item.id == null
          ? null
          : p.join(ref.read(coverDirProvider), item.id.toString());
      final coverExists = coverPath != null && File(coverPath).existsSync();
      try {
        await ref.read(playerBridgeProvider).stop();
        await _dlna.playLocalTrack(
          renderer: renderer,
          path: path,
          title: item.title,
          artist: item.artist,
          album: item.album,
          coverPath: coverExists ? coverPath : null,
        );
        _activeDlnaTrackLease = nextLease;
        if (previousLease != null && !identical(previousLease, nextLease)) {
          await previousLease.release();
        }
      } catch (_) {
        await nextLease?.release();
        rethrow;
      }
      _dlnaLastPath = path;
      _dlnaLastPlayStartedAt = DateTime.now();
      _ensureDlnaPoller();
      state = state.copyWith(
        currentPath: path,
        positionMs: 0,
        playerState: PlayerState.playing,
      );
      return true;
    }

    final bridge = ref.read(playerBridgeProvider);
    state = state.copyWith(playerState: PlayerState.buffering, lastError: null);
    try {
      var itemId = item.itemId;
      if (itemId == null) {
        // Entering native playback from a DLNA queue materializes every occurrence once.
        final items = ref.read(queueControllerProvider).items;
        await bridge.retainQueuePaths(items.map((item) => item.path));
        final ids = await _resolveTrackIds(items);
        final snapshot = await bridge.replaceQueue(ids);
        final index = items.indexOf(item);
        ref
            .read(queueControllerProvider.notifier)
            .applyBackend(snapshot, metadata: items);
        itemId = snapshot.items[index < 0 ? 0 : index].itemId;
      }
      await bridge.selectQueueItem(itemId);
      return true;
    } catch (error) {
      ref.read(loggerProvider).w('failed to open track: $path', error: error);
      state = state.copyWith(
        playerState: PlayerState.stopped,
        audioStarted: false,
        lastError: error.toString(),
      );
      return false;
    }
  }

  void _onEvent(Event event) {
    if (_dlnaActive) return;
    event.when(
      stateChanged: (s) {
        state = state.copyWith(playerState: s);
      },
      position: (ms, trackId, itemId, sessionId) {
        if (_currentTrackId != null && trackId != _currentTrackId) {
          return;
        }
        if (_activePositionItemId == null || _activePositionItemId != itemId) {
          _activePositionItemId = itemId;
          _activePositionSessionId = sessionId;
        } else if (_activePositionSessionId != null &&
            sessionId != _activePositionSessionId) {
          if (sessionId > _activePositionSessionId!) {
            _activePositionSessionId = sessionId;
          } else {
            return;
          }
        }
        state = state.copyWith(positionMs: ms);
      },
      trackChanged: (trackId, itemId) {
        ref.read(queueControllerProvider.notifier).observeCurrent(itemId);
        _currentTrackId = trackId;
        _activePositionItemId = itemId;
        _activePositionSessionId = null;
        final currentItem = ref.read(queueControllerProvider).currentItem;
        state = state.copyWith(
          currentPath: currentItem?.path,
          positionMs: 0,
          audioStarted: false,
          trackInfo: null,
        );
        unawaited(_updateTrackInfo());
        unawaited(_refreshBackendQueue(currentItemId: itemId));
      },
      playbackEnded: (trackId, itemId) {
        ref
            .read(loggerProvider)
            .i('playback ended: track=$trackId item=$itemId');
        state = state.copyWith(audioStarted: false);
        if (_currentTrackId != null && _currentTrackId != trackId) {
          ref
              .read(loggerProvider)
              .d(
                'ignore stale playbackEnded: ended=$trackId current=$_currentTrackId',
              );
          return;
        }
        unawaited(_refreshBackendQueue());
      },
      audioStart: () {
        state = state.copyWith(audioStarted: true);
      },
      audioEnd: () {
        state = state.copyWith(audioStarted: false);
      },
      volumeChanged: (volume, seq) {
        final normalized = volume.clamp(0.0, 1.0).toDouble();
        final seqInt = seq.toInt();
        if (seqInt <= _latestVolumeAckSeq) {
          return;
        }
        _latestVolumeAckSeq = seqInt;
        if (seqInt < _latestVolumeCommandSeq) {
          return;
        }
        state = state.copyWith(
          desiredVolume: normalized,
          appliedVolume: normalized,
        );
        if (normalized > 0) {
          _lastNonZeroVolume = normalized;
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
    );
  }
}
