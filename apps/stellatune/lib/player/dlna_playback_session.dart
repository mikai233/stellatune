import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/player/queue_models.dart';

final dlnaBridgeProvider = Provider<DlnaBridge>((ref) => const DlnaBridge());

class DlnaPlaybackUpdate {
  const DlnaPlaybackUpdate({
    required this.state,
    required this.positionMs,
    required this.path,
    this.advance = false,
  });
  final PlayerState state;
  final int positionMs;
  final String? path;
  final bool advance;
}

/// One immutable renderer identity, its network operations and file access lease.
/// Invalidating a session synchronously prevents all later asynchronous writes.
class DlnaPlaybackSession {
  DlnaPlaybackSession({
    required this.renderer,
    required this.bridge,
    required this.ready,
    required this.acquirePath,
    required this.coverDirectory,
    required this.onUpdate,
    required this.onError,
    this.pollInterval = const Duration(milliseconds: 600),
  });

  final DlnaRenderer renderer;
  final DlnaBridge bridge;
  final Future<void> ready;
  final Future<DirectoryAccessLease?> Function(String path) acquirePath;
  final String coverDirectory;
  final void Function(DlnaPlaybackUpdate) onUpdate;
  final void Function(String) onError;
  final Duration pollInterval;
  bool _active = true;
  bool _polling = false;
  bool _transportBusy = false;
  Timer? _timer;
  int _transportGeneration = 0;
  int _volumeGeneration = 0;
  Future<void> _commands = Future.value();
  Future<void> _volumeCommands = Future.value();
  Future<void>? _closing;
  DirectoryAccessLease? _lease;
  String? _path;
  int _durationMs = 0;
  String? _lastTransportState;
  PlayerState _state = PlayerState.stopped;
  DateTime? _suppressUntil;
  DateTime? _startedAt;
  bool _volumeUnsupported = false;
  int? _lastReportedVolume;
  int _volumeMismatches = 0;

  bool get active => _active;
  String? get path => _path;
  bool _current(int generation) =>
      _active && generation == _transportGeneration;

  void suppressAutoNext([Duration duration = const Duration(seconds: 2)]) {
    _suppressUntil = DateTime.now().add(duration);
  }

  void startPolling() {
    if (!_active) return;
    _timer ??= Timer.periodic(pollInterval, (_) => unawaited(poll()));
  }

  Future<bool> _command(Future<void> Function() action) {
    // Invalidate old polls, but keep ordered transport commands intact: a seek
    // or pause queued behind publication still needs that media and its lease.
    ++_transportGeneration;
    suppressAutoNext();
    final result = _commands.then((_) async {
      await ready;
      if (!_active) return false;
      _transportBusy = true;
      try {
        await action();
      } catch (_) {
        if (_active) rethrow;
      } finally {
        _transportBusy = false;
      }
      return _active;
    });
    _commands = result.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return result;
  }

  Future<bool> playItem(QueueItem item) => _command(() async {
    final lease = await acquirePath(item.path);
    if (!_active) {
      await lease?.release();
      return;
    }
    var transferred = false;
    try {
      final coverPath = item.id == null
          ? null
          : p.join(coverDirectory, item.id.toString());
      final hasCover = coverPath != null && await File(coverPath).exists();
      if (!_active) return;
      await bridge.playLocalTrack(
        renderer: renderer,
        path: item.path,
        title: item.title,
        artist: item.artist,
        album: item.album,
        coverPath: hasCover ? coverPath : null,
      );
      if (!_active) return;
      final previous = _lease;
      _lease = lease;
      transferred = true;
      await previous?.release();
      if (!_active) return;
      _path = item.path;
      _durationMs = item.durationMs ?? 0;
      _state = PlayerState.playing;
      _startedAt = DateTime.now();
      startPolling();
    } finally {
      if (!transferred) await lease?.release();
    }
  });

  Future<bool> play() => _command(() async {
    await bridge.avTransportPlay(
      controlUrl: renderer.avTransportControlUrl!,
      serviceType: renderer.avTransportServiceType,
    );
    if (!_active) return;
    _state = PlayerState.playing;
    _startedAt = DateTime.now();
    startPolling();
  });

  Future<bool> pause() => _command(() async {
    await bridge.avTransportPause(
      controlUrl: renderer.avTransportControlUrl!,
      serviceType: renderer.avTransportServiceType,
    );
    if (_active) _state = PlayerState.paused;
  });

  Future<bool> seek(int positionMs) => _command(() async {
    await bridge.avTransportSeekMs(
      controlUrl: renderer.avTransportControlUrl!,
      serviceType: renderer.avTransportServiceType,
      positionMs: positionMs,
    );
  });

  Future<bool> stop() => _command(() async {
    await bridge.avTransportStop(
      controlUrl: renderer.avTransportControlUrl!,
      serviceType: renderer.avTransportServiceType,
    );
    // Global publication cleanup must finish before the next serialized command.
    await bridge.httpUnpublishAll();
    await _releaseLease();
    if (!_active) return;
    _path = null;
    _state = PlayerState.stopped;
  });

  Future<void> poll() async {
    if (!_active || _polling || _transportBusy) return;
    _polling = true;
    final generation = _transportGeneration;
    try {
      await ready;
      if (!_current(generation) || _transportBusy) return;
      final info = await bridge.avTransportGetTransportInfo(
        controlUrl: renderer.avTransportControlUrl!,
        serviceType: renderer.avTransportServiceType,
      );
      if (!_current(generation)) return;
      final position = await bridge.avTransportGetPositionInfo(
        controlUrl: renderer.avTransportControlUrl!,
        serviceType: renderer.avTransportServiceType,
      );
      if (!_current(generation)) return;
      final transport = info.currentTransportState.trim().toUpperCase();
      final previous = _lastTransportState;
      _lastTransportState = transport;
      _state = switch (transport) {
        'PLAYING' => PlayerState.playing,
        'PAUSED_PLAYBACK' || 'PAUSED_RECORDING' => PlayerState.paused,
        'TRANSITIONING' => PlayerState.buffering,
        'STOPPED' || 'NO_MEDIA_PRESENT' => PlayerState.stopped,
        _ => _state,
      };
      final elapsed = position.relTimeMs.toInt();
      final duration = position.trackDurationMs?.toInt() ?? _durationMs;
      final now = DateTime.now();
      final advance =
          _path != null &&
          (_suppressUntil == null || !now.isBefore(_suppressUntil!)) &&
          _startedAt != null &&
          now.difference(_startedAt!).inMilliseconds >= 1500 &&
          (transport == 'STOPPED' || transport == 'NO_MEDIA_PRESENT') &&
          (previous == 'PLAYING' || previous == 'TRANSITIONING') &&
          (duration <= 0 || elapsed >= duration - 800);
      if (advance) suppressAutoNext();
      onUpdate(
        DlnaPlaybackUpdate(
          state: _state,
          positionMs: elapsed,
          path: _path,
          advance: advance,
        ),
      );
    } catch (error, stack) {
      if (_active) logger.d('dlna poll failed: $error', stackTrace: stack);
    } finally {
      _polling = false;
    }
  }

  Future<void> setVolume(double value) {
    final generation = ++_volumeGeneration;
    bool current() => _active && generation == _volumeGeneration;
    final next = _volumeCommands.then((_) async {
      await ready;
      if (!current() || _volumeUnsupported) return;
      final url = renderer.renderingControlUrl;
      if (url == null) {
        _volumeUnsupported = true;
        onError('DLNA device does not support volume');
        return;
      }
      final volume = (value.clamp(0.0, 1.0) * 100).round();
      try {
        await bridge.renderingControlSetMute(
          controlUrl: url,
          serviceType: renderer.renderingControlServiceType,
          mute: volume <= 0,
        );
        if (!current()) return;
        await bridge.renderingControlSetVolume(
          controlUrl: url,
          serviceType: renderer.renderingControlServiceType,
          volume0To100: volume,
        );
        if (!current()) return;
        final actual = await bridge.renderingControlGetVolume(
          controlUrl: url,
          serviceType: renderer.renderingControlServiceType,
        );
        if (!current()) return;
        if ((actual - volume).abs() >= 5) {
          _volumeMismatches = _lastReportedVolume == actual
              ? _volumeMismatches + 1
              : 1;
          _lastReportedVolume = actual;
          if (_volumeMismatches >= 3) {
            _volumeUnsupported = true;
            onError('DLNA device ignores volume control');
          }
        } else {
          _volumeMismatches = 0;
        }
      } catch (error) {
        if (current()) onError('DLNA volume failed: $error');
      }
    });
    _volumeCommands = next.then<void>(
      (_) {},
      onError: (Object _, StackTrace _) {},
    );
    return _volumeCommands;
  }

  void invalidate() {
    _active = false;
    ++_transportGeneration;
    ++_volumeGeneration;
    _timer?.cancel();
    _timer = null;
  }

  Future<void> _releaseLease() async {
    final lease = _lease;
    _lease = null;
    await lease?.release();
  }

  Future<void> close() {
    invalidate();
    return _closing ??= _close();
  }

  Future<void> _close() async {
    try {
      await ready;
    } catch (_) {
      /* cleanup still owns the old transport */
    }
    await Future.wait([_commands, _volumeCommands]);
    try {
      await bridge.avTransportStop(
        controlUrl: renderer.avTransportControlUrl!,
        serviceType: renderer.avTransportServiceType,
      );
    } catch (error) {
      logger.w('failed to stop previous DLNA renderer: $error');
    }
    try {
      await bridge.httpUnpublishAll();
    } catch (error) {
      logger.w('failed to unpublish previous DLNA media: $error');
    }
    await _releaseLease();
  }
}
