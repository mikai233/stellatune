import 'queue_identity_resolver.dart';

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/player/decoder_extension_support.dart';
import 'package:stellatune/player/dlna_playback_session.dart';
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
  static const int _volumeRampMs = 6;

  StreamSubscription<Event>? _sub;
  StreamSubscription<PlaybackQueue>? _queueSub;
  int _backendEventGeneration = 0;
  Timer? _volumePersistDebounce;
  BigInt? _currentTrackId;
  int _navigationGeneration = 0;
  double _lastNonZeroVolume = 1.0;
  int _nextVolumeSeq = 1;
  int _latestVolumeCommandSeq = 0;
  int _latestVolumeAckSeq = 0;
  DlnaPlaybackSession? _dlnaSession;
  int _outputGeneration = 0;
  Future<void> _outputTransitions = Future.value();
  BigInt? _activePositionItemId;
  BigInt? _activePositionSessionId;

  @override
  PlaybackState build() {
    ++_navigationGeneration;
    unawaited(_sub?.cancel());
    unawaited(_queueSub?.cancel());
    _volumePersistDebounce?.cancel();
    _volumePersistDebounce = null;
    _currentTrackId = null;
    ++_outputGeneration;
    final oldSession = _dlnaSession;
    oldSession?.invalidate();
    _dlnaSession = null;
    _outputTransitions = _outputTransitions.then((_) => oldSession?.close());
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
    _queueSub = bridge.queueEvents().listen(
      _onBackendQueue,
      onError: (Object error, StackTrace stack) {
        if (!ref.mounted) return;
        ref
            .read(loggerProvider)
            .w('queue events failed', error: error, stackTrace: stack);
      },
    );

    ref.onDispose(() {
      ++_navigationGeneration;
      unawaited(_sub?.cancel());
      unawaited(_queueSub?.cancel());
      _volumePersistDebounce?.cancel();
      ++_outputGeneration;
      unawaited(_dlnaSession?.close());
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
      final renderer = ref.read(dlnaSelectedRendererProvider);
      final output = _outputGeneration;
      unawaited(
        Future<void>.microtask(() async {
          if (ref.mounted &&
              output == _outputGeneration &&
              ref.read(dlnaSelectedRendererProvider) == renderer) {
            await _onOutputChanged(null, renderer);
          }
        }),
      );
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

  void _onBackendQueue(PlaybackQueue snapshot) {
    if (!ref.mounted || _dlnaActive) return;
    final output = _outputGeneration;
    ref.read(queueControllerProvider.notifier).applyBackend(snapshot);
    final paths = ref
        .read(queueControllerProvider)
        .items
        .map((item) => item.path)
        .toList();
    final bridge = ref.read(playerBridgeProvider);
    unawaited(() async {
      try {
        await bridge.retainQueuePaths(paths);
        if (_isLocalOutput(output)) {
          await bridge.releaseRemovedQueuePaths(
            ref.read(queueControllerProvider).items.map((item) => item.path),
          );
        }
      } catch (error) {
        if (ref.mounted) {
          ref.read(loggerProvider).w('queue directory access failed: $error');
        }
      }
    }());
  }

  Future<void> _refreshBackendQueue({BigInt? currentItemId}) async {
    if (!ref.mounted || _dlnaActive) return;
    final output = _outputGeneration;
    try {
      final bridge = ref.read(playerBridgeProvider);
      final snapshot = await bridge.playbackQueue();
      if (!_isLocalOutput(output)) return;
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
      if (!_isLocalOutput(output)) return;
      await bridge.releaseRemovedQueuePaths(
        ref.read(queueControllerProvider).items.map((item) => item.path),
      );
    } catch (error) {
      if (ref.mounted) {
        ref.read(loggerProvider).w('queue refresh failed: $error');
      }
    }
  }

  Future<void> _restoreBackendSnapshot() async {
    if (_dlnaActive) return;
    final eventGeneration = _backendEventGeneration;
    final navigation = _navigationGeneration;
    try {
      final snapshot = await ref.read(playerBridgeProvider).playbackSnapshot();
      final track = snapshot.trackId;
      final queue = await ref.read(playerBridgeProvider).playbackQueue();
      if (!ref.mounted ||
          _dlnaActive ||
          navigation != _navigationGeneration ||
          eventGeneration != _backendEventGeneration) {
        return;
      }
      ref.read(queueControllerProvider.notifier).applyBackend(queue);
      if (snapshot.itemId != null) {
        ref
            .read(queueControllerProvider.notifier)
            .observeCurrent(snapshot.itemId!);
      }
      await ref
          .read(playerBridgeProvider)
          .retainQueuePaths(queue.items.map((item) => item.localPath ?? ''));
      if (!ref.mounted ||
          _dlnaActive ||
          navigation != _navigationGeneration ||
          eventGeneration != _backendEventGeneration) {
        return;
      }
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
      if (ref.mounted) {
        ref.read(loggerProvider).w('backend playback snapshot failed: $e');
      }
    }
  }

  bool _isLocalOutput(int generation) =>
      ref.mounted && generation == _outputGeneration && !_dlnaActive;

  bool _isCurrentSession(DlnaPlaybackSession session, int generation) =>
      ref.mounted &&
      generation == _outputGeneration &&
      identical(_dlnaSession, session) &&
      session.active &&
      ref.read(dlnaSelectedRendererProvider) == session.renderer;

  Future<void> seekMs(int positionMs) async {
    final pos = positionMs.clamp(0, 1 << 31);
    final outputGeneration = _outputGeneration;
    final navigation = _navigationGeneration;
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).seekMs(pos);
      if (_isLocalOutput(outputGeneration) &&
          navigation == _navigationGeneration) {
        state = state.copyWith(positionMs: pos, lastError: null);
      }
      return;
    }
    final session = _dlnaSession;
    if (session != null &&
        await session.seek(pos) &&
        _isCurrentSession(session, outputGeneration)) {
      state = state.copyWith(positionMs: pos, lastError: null);
    }
  }

  Future<void> _onOutputChanged(
    DlnaRenderer? previousRenderer,
    DlnaRenderer? renderer,
  ) async {
    if (!ref.mounted || previousRenderer == renderer) return;
    final generation = ++_outputGeneration;
    final navigation = ++_navigationGeneration;
    final wasPlaying =
        state.playerState == PlayerState.playing ||
        state.playerState == PlayerState.buffering;
    final previousSession = _dlnaSession;
    previousSession?.invalidate();
    _dlnaSession = null;
    final bridge = ref.read(playerBridgeProvider);
    final transition = _outputTransitions.then((_) async {
      await previousSession?.close();
      if (!ref.mounted || generation != _outputGeneration) return;
      if (renderer?.avTransportControlUrl != null) await bridge.stop();
    });
    _outputTransitions = transition.then<void>(
      (_) {},
      onError: (Object _, StackTrace _) {},
    );
    if (renderer?.avTransportControlUrl != null) {
      final accessStore = ref.read(settingsStoreServiceProvider);
      late final DlnaPlaybackSession session;
      session = DlnaPlaybackSession(
        renderer: renderer!,
        bridge: ref.read(dlnaBridgeProvider),
        ready: transition,
        coverDirectory: ref.read(coverDirProvider),
        acquirePath: (path) => DirectoryAccessService.instance.acquireLocalPath(
          path: path,
          store: accessStore,
        ),
        onError: (error) {
          if (_isCurrentSession(session, generation)) {
            state = state.copyWith(lastError: error);
          }
        },
        onUpdate: (update) {
          if (!_isCurrentSession(session, generation)) return;
          state = state.copyWith(
            playerState: update.state,
            positionMs: update.positionMs,
          );
          if (update.advance &&
              ref.read(queueControllerProvider).currentItem?.path ==
                  update.path) {
            unawaited(next(auto: true));
          }
        },
      );
      _dlnaSession = session;
    }
    state = state.copyWith(
      playerState: wasPlaying ? PlayerState.buffering : PlayerState.stopped,
      positionMs: 0,
      pendingItem: null,
      audioStarted: false,
      trackInfo: null,
      lastError: null,
    );
    try {
      await transition;
      if (!ref.mounted || generation != _outputGeneration) return;
      _dlnaSession?.startPolling();
      if (navigation != _navigationGeneration) return;
      final item = ref.read(queueControllerProvider).currentItem;
      if (wasPlaying && item != null) {
        await _loadQueueItemOrStop(item, generation: navigation);
      } else if (!_dlnaActive) {
        await _refreshBackendQueue();
      }
    } catch (error, stack) {
      if (!ref.mounted || generation != _outputGeneration) return;
      ref
          .read(loggerProvider)
          .w('output change failed', error: error, stackTrace: stack);
      state = state.copyWith(lastError: error.toString());
    }
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
    final generation = _beginNavigation(
      items[startIndex.clamp(0, items.length - 1)],
    );
    if (!_dlnaActive) {
      try {
        final bridge = ref.read(playerBridgeProvider);
        await bridge.retainQueuePaths(items.map((item) => item.path));
        if (generation != _navigationGeneration) return;
        final ids = await _resolveTrackIds(items);
        if (generation != _navigationGeneration) return;
        final snapshot = await bridge.replaceQueue(ids);
        if (generation != _navigationGeneration) return;
        ref
            .read(queueControllerProvider.notifier)
            .applyBackend(snapshot, source: source, replaceSource: true);
        await ref.read(settingsStoreProvider.notifier).setQueueSource(source);
        if (generation != _navigationGeneration) return;
        final mode = ref.read(settingsStoreProvider).playMode;
        ref.read(queueControllerProvider.notifier).setPlayMode(mode);
        final selected = ref
            .read(queueControllerProvider)
            .items[startIndex.clamp(0, items.length - 1)];
        await _loadQueueItemOrStop(selected, generation: generation);
      } catch (error) {
        if (generation != _navigationGeneration) return;
        ref
            .read(loggerProvider)
            .w('failed to prepare playback queue', error: error);
        state = state.copyWith(lastError: error.toString(), pendingItem: null);
      }
      return;
    }
    ref
        .read(queueControllerProvider.notifier)
        .setQueue(items, startIndex: startIndex, source: source);
    final item = ref.read(queueControllerProvider).currentItem;
    if (item == null) return;
    await _loadQueueItemOrStop(item, generation: generation);
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
    final output = _outputGeneration;
    final queue = ref.read(queueControllerProvider);
    if (!_dlnaActive) {
      try {
        final bridge = ref.read(playerBridgeProvider);
        await bridge.retainQueuePaths(items.map((item) => item.path));
        if (!_isLocalOutput(output)) return;
        final ids = await _resolveTrackIds(items);
        if (!_isLocalOutput(output)) return;
        final snapshot = await bridge.appendQueue(ids);
        if (!_isLocalOutput(output)) return;
        ref
            .read(queueControllerProvider.notifier)
            .applyBackend(snapshot, preserveCurrent: true);
        if (queue.items.isEmpty) {
          await _loadQueueItemOrStop(
            ref.read(queueControllerProvider).items.first,
          );
        }
      } catch (error) {
        if (!_isLocalOutput(output)) return;
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
    _dlnaSession?.suppressAutoNext();
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

  int _beginNavigation(QueueItem? item) {
    final generation = ++_navigationGeneration;
    state = state.copyWith(pendingItem: item, lastError: null);
    return generation;
  }

  Future<bool> _loadQueueItemOrStop(QueueItem item, {int? generation}) async {
    generation ??= _beginNavigation(item);
    if (generation != _navigationGeneration) return false;
    state = state.copyWith(pendingItem: item);
    final loaded = await _loadAndPlayQueueItem(item, generation);
    if (generation != _navigationGeneration) return false;
    if (loaded == null) {
      state = state.copyWith(pendingItem: null);
      return false;
    }
    if (loaded) {
      unawaited(_refreshBackendQueue());
      return true;
    }

    final loadError = state.lastError;
    // stop() invalidates this request too; only project its result if it stays latest.
    final stopGeneration = _navigationGeneration + 1;
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
    if (stopGeneration != _navigationGeneration) return false;
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
    final session = _dlnaSession;
    final generation = _outputGeneration;
    final item = ref.read(queueControllerProvider).currentItem;
    if (session == null || item == null) return;
    if (session.path != item.path) {
      await _loadQueueItemOrStop(item);
      return;
    }
    if (await session.play() && _isCurrentSession(session, generation)) {
      state = state.copyWith(
        playerState: PlayerState.playing,
        currentPath: item.path,
        lastError: null,
      );
    }
  }

  Future<void> pause() async {
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).pause();
      return;
    }
    final session = _dlnaSession;
    final generation = _outputGeneration;
    if (session != null &&
        await session.pause() &&
        _isCurrentSession(session, generation)) {
      state = state.copyWith(playerState: PlayerState.paused, lastError: null);
    }
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
      unawaited(_dlnaSession?.setVolume(v));
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
    final generation = _beginNavigation(null);
    final output = _outputGeneration;
    if (!_dlnaActive) {
      await ref.read(playerBridgeProvider).stop();
      if (_isLocalOutput(output) && generation == _navigationGeneration) {
        state = state.copyWith(positionMs: 0);
      }
      return;
    }
    final session = _dlnaSession;
    if (session != null &&
        await session.stop() &&
        _isCurrentSession(session, output) &&
        generation == _navigationGeneration) {
      state = state.copyWith(
        playerState: PlayerState.stopped,
        positionMs: 0,
        lastError: null,
      );
    }
  }

  Future<void> next({bool auto = false}) async {
    if (!_dlnaActive) {
      if (auto) return;
      _beginNavigation(null);
      await ref.read(playerBridgeProvider).nextQueueItem();
      return;
    }
    _dlnaSession?.suppressAutoNext(const Duration(seconds: 1));
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
      _beginNavigation(null);
      await ref.read(playerBridgeProvider).previousQueueItem();
      return;
    }
    _dlnaSession?.suppressAutoNext(const Duration(seconds: 1));
    final item = ref.read(queueControllerProvider.notifier).previous();
    if (item == null) return;
    await _loadQueueItemOrStop(item);
  }

  Future<void> _updateTrackInfo() async {
    if (_dlnaActive) return;
    final output = _outputGeneration;
    final itemId = _activePositionItemId;
    try {
      final info = await ref.read(playerBridgeProvider).currentTrackInfo();
      if (!_isLocalOutput(output) || _activePositionItemId != itemId) return;
      state = state.copyWith(trackInfo: info);
    } catch (e) {
      if (ref.mounted) {
        ref.read(loggerProvider).d('fetch track info failed: $e');
      }
    }
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
    final output = _outputGeneration;
    final items = ref.read(queueControllerProvider).items;
    final ids = [
      for (final index in indices)
        if (index >= 0 && index < items.length && items[index].itemId != null)
          items[index].itemId!,
    ];
    final snapshot = await ref.read(playerBridgeProvider).removeQueueItems(ids);
    if (!_isLocalOutput(output)) return 0;
    ref.read(queueControllerProvider.notifier).applyBackend(snapshot);
    return ids.length;
  }

  Future<List<BigInt>> _resolveTrackIds(List<QueueItem> items) async {
    final bridge = ref.read(playerBridgeProvider);
    final ids = await resolveQueueTrackIds(
      items,
      ensureLocalTracks: bridge.ensureLocalTracks,
      ensureProviderTrack: (provider) => bridge.ensureProviderTrack(
        providerId: provider.providerId,
        providerKey: provider.providerKey,
        pluginId: provider.pluginId,
        typeId: provider.typeId,
      ),
    );
    final updates = <QueueMetadataUpdate>[
      for (var i = 0; i < items.length; i++)
        if (!items[i].isLocal &&
            (items[i].title != null ||
                items[i].cover != null ||
                items[i].artist != null ||
                items[i].album != null ||
                items[i].durationMs != null))
          QueueMetadataUpdate(
            trackId: ids[i],
            metadata: TrackPresentation(
              title: items[i].title,
              artist: items[i].artist,
              album: items[i].album,
              durationMs: items[i].durationMs == null
                  ? null
                  : BigInt.from(items[i].durationMs!),
              cover: items[i].cover == null
                  ? null
                  : TrackCover(
                      kind: TrackCoverKind.values.byName(
                        items[i].cover!.kind.name,
                      ),
                      value: items[i].cover!.value,
                      mime: items[i].cover!.mime,
                    ),
            ),
          ),
    ];
    if (updates.isNotEmpty) await bridge.storeQueueMetadata(updates);
    return ids;
  }

  // null means superseded, which must never enter failure cleanup.
  Future<bool?> _loadAndPlayQueueItem(QueueItem item, int generation) async {
    final path = item.path;
    state = state.copyWith(lastError: null, lastLog: '');
    final blockedReason = await _playabilityBlockReason(item);
    if (generation != _navigationGeneration) return null;
    if (blockedReason != null) {
      await _removeCurrentQueueItemIfDisabledPluginBlocked(item, blockedReason);
      if (generation != _navigationGeneration) return null;
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
      final session = _dlnaSession;
      final output = _outputGeneration;
      if (session == null) return null;
      try {
        final loaded = await session.playItem(item);
        if (!_isCurrentSession(session, output) ||
            generation != _navigationGeneration) {
          return null;
        }
        if (!loaded) return null;
        state = state.copyWith(
          currentPath: path,
          pendingItem: null,
          positionMs: 0,
          playerState: PlayerState.playing,
        );
        return true;
      } catch (error) {
        if (!_isCurrentSession(session, output) ||
            generation != _navigationGeneration) {
          return null;
        }
        state = state.copyWith(lastError: error.toString());
        return false;
      }
    }

    final bridge = ref.read(playerBridgeProvider);
    state = state.copyWith(playerState: PlayerState.buffering, lastError: null);
    try {
      var itemId = item.itemId;
      if (itemId == null) {
        // Entering native playback from a DLNA queue materializes every occurrence once.
        final items = ref.read(queueControllerProvider).items;
        await bridge.retainQueuePaths(items.map((item) => item.path));
        if (generation != _navigationGeneration) return null;
        final ids = await _resolveTrackIds(items);
        if (generation != _navigationGeneration) return null;
        final snapshot = await bridge.replaceQueue(ids);
        if (generation != _navigationGeneration) return null;
        final index = items.indexOf(item);
        ref.read(queueControllerProvider.notifier).applyBackend(snapshot);
        itemId = snapshot.items[index < 0 ? 0 : index].itemId;
      }
      if (generation != _navigationGeneration) return null;
      state = state.copyWith(
        pendingItem:
            ref
                .read(queueControllerProvider)
                .items
                .where((entry) => entry.itemId == itemId)
                .firstOrNull ??
            item,
      );
      final accepted = await bridge.selectQueueItem(itemId);
      if (generation != _navigationGeneration || !accepted) return null;
      return true;
    } catch (error) {
      if (generation != _navigationGeneration) return null;
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
    if (!ref.mounted || _dlnaActive) return;
    event.when(
      stateChanged: (s) {
        _backendEventGeneration++;
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
        _backendEventGeneration++;
        state = state.copyWith(positionMs: ms);
      },
      trackChanged: (trackId, itemId) {
        _backendEventGeneration++;
        if (state.pendingItem?.itemId == itemId) {
          state = state.copyWith(pendingItem: null);
        }
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
        _backendEventGeneration++;
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
        _backendEventGeneration++;
        ref.read(loggerProvider).e(message);
        state = state.copyWith(lastError: message, pendingItem: null);
      },
      log: (message) {
        ref.read(loggerProvider).d(message);
        state = state.copyWith(lastLog: message);
      },
    );
  }
}
