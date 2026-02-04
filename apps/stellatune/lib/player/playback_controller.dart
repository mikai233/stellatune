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
  Timer? _volumeDebounce;
  double? _pendingVolume;
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

  @override
  PlaybackState build() {
    unawaited(_sub?.cancel());
    _volumeDebounce?.cancel();
    _volumeDebounce = null;
    _pendingVolume = null;
    _dlnaPollTimer?.cancel();
    _dlnaPollTimer = null;
    _dlnaPollInFlight = false;
    _dlnaLastTransportState = null;
    _dlnaSuppressAutoNextUntil = null;
    _dlnaLastPlayStartedAt = null;

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
      _volumeDebounce?.cancel();
      _dlnaPollTimer?.cancel();
    });

    final savedVolume = ref.read(settingsStoreProvider).volume.clamp(0.0, 1.0);

    ref.listen<DlnaRenderer?>(dlnaSelectedRendererProvider, (prev, next) {
      unawaited(_onOutputChanged(prev, next));
    });

    if (!_dlnaActive) {
      unawaited(bridge.setVolume(savedVolume));
    } else {
      _ensureDlnaPoller();
    }
    return const PlaybackState.initial().copyWith(volume: savedVolume);
  }

  bool get _dlnaActive =>
      ref.read(dlnaSelectedRendererProvider)?.avTransportControlUrl != null;

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
    final currentPath = ref.read(queueControllerProvider).currentItem?.path;
    _dlnaSuppressAutoNext();

    // Stop whichever output was previously active.
    if (prev?.avTransportControlUrl != null) {
      try {
        await _dlna.avTransportStop(
          controlUrl: prev!.avTransportControlUrl!,
          serviceType: prev.avTransportServiceType,
        );
      } catch (_) {}
      try {
        await _dlna.httpUnpublishAll();
      } catch (_) {}
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
        } catch (_) {}
      }
    }

    if (!wasPlaying || currentPath == null) return;
    await _loadAndPlay(currentPath);
  }

  Future<void> setQueueAndPlay(List<String> paths, {int startIndex = 0}) =>
      setQueueAndPlayTracks(
        paths.map((p) => TrackLite(id: -1, path: p)).toList(),
        startIndex: startIndex,
      );

  Future<void> setQueueAndPlayTracks(
    List<TrackLite> tracks, {
    int startIndex = 0,
  }) async {
    final items = tracks
        .map(
          (t) => QueueItem(
            id: t.id.toInt() >= 0 ? t.id.toInt() : null,
            path: t.path,
            title: t.title,
            artist: t.artist,
            album: t.album,
            durationMs: t.durationMs?.toInt(),
          ),
        )
        .toList();
    ref
        .read(queueControllerProvider.notifier)
        .setQueue(items, startIndex: startIndex);
    final item = ref.read(queueControllerProvider).currentItem;
    if (item == null) return;
    await _loadAndPlay(item.path);
  }

  Future<void> enqueue(List<String> paths) =>
      enqueueTracks(paths.map((p) => TrackLite(id: -1, path: p)).toList());

  Future<void> enqueueTracks(List<TrackLite> tracks) async {
    final items = tracks
        .map(
          (t) => QueueItem(
            id: t.id.toInt() >= 0 ? t.id.toInt() : null,
            path: t.path,
            title: t.title,
            artist: t.artist,
            album: t.album,
            durationMs: t.durationMs?.toInt(),
          ),
        )
        .toList();
    final queue = ref.read(queueControllerProvider);
    ref.read(queueControllerProvider.notifier).enqueue(items);

    // If nothing is loaded yet, start playing immediately from the first enqueued item.
    if (queue.currentItem == null && items.isNotEmpty) {
      await _loadAndPlay(items.first.path);
    }
  }

  Future<void> playIndex(int index) async {
    _dlnaSuppressAutoNext();
    ref.read(queueControllerProvider.notifier).selectIndex(index);
    final item = ref.read(queueControllerProvider).currentItem;
    if (item == null) return;
    await _loadAndPlay(item.path);
  }

  Future<void> play() async {
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).play();
      return;
    }

    final renderer = ref.read(dlnaSelectedRendererProvider);
    final controlUrl = renderer?.avTransportControlUrl;
    if (renderer == null || controlUrl == null) return;

    final path = ref.read(queueControllerProvider).currentItem?.path;
    if (path == null) return;

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

    await _loadAndPlay(path);
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

    _pendingVolume = v;
    _volumeDebounce?.cancel();
    final delay = _dlnaActive
        ? const Duration(milliseconds: 180)
        : const Duration(milliseconds: 30);
    _volumeDebounce = Timer(delay, () {
      final toSend = _pendingVolume;
      if (toSend == null) return;
      if (_dlnaActive) {
        unawaited(_applyDlnaVolume(toSend));
      } else {
        unawaited(ref.read(playerBridgeProvider).setVolume(toSend));
      }
      unawaited(ref.read(settingsStoreProvider).setVolume(toSend));
    });
  }

  Future<void> stop() async {
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).stop();
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
    state = state.copyWith(
      playerState: PlayerState.stopped,
      positionMs: 0,
      lastError: null,
    );
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
    await _loadAndPlay(item.path);
  }

  Future<void> previous() async {
    _dlnaSuppressAutoNext(const Duration(seconds: 1));
    final item = ref.read(queueControllerProvider.notifier).previous();
    if (item == null) return;
    await _loadAndPlay(item.path);
  }

  Future<void> _loadAndPlay(String path) async {
    state = state.copyWith(lastError: null, lastLog: '');
    if (_dlnaActive) {
      final renderer = ref.read(dlnaSelectedRendererProvider);
      if (renderer == null) return;
      final current = ref.read(queueControllerProvider).currentItem;
      final coverPath = current?.id == null
          ? null
          : p.join(ref.read(coverDirProvider), current!.id.toString());
      final coverExists = coverPath != null && File(coverPath).existsSync();
      await ref.read(playerBridgeProvider).stop();
      await _dlna.playLocalTrack(
        renderer: renderer,
        path: path,
        title: current?.title,
        artist: current?.artist,
        album: current?.album,
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
    await bridge.load(path);
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
      },
      trackChanged: (path) {
        state = state.copyWith(currentPath: path);
      },
      playbackEnded: (path) {
        ref.read(loggerProvider).i('playback ended: $path');
        unawaited(next(auto: true));
      },
      volumeChanged: (volume) {
        state = state.copyWith(volume: volume);
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
