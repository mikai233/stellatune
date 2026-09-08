import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/bridge/bridge.dart';

enum TranscodeResult { completed, canceled, failed }

class TranscodeOutcome {
  const TranscodeOutcome(this.result, {required this.outputPath, this.error});
  final TranscodeResult result;
  final String outputPath;
  final Object? error;
}

/// A single task owns its stream and cancellation, with no list/page dependency.
class TranscodeTaskController extends ChangeNotifier {
  TranscodeTaskController({
    required this._start,
    required this._cancelTask,
    required this._outputPath,
  });

  final Stream<TranscodeProgressEvent> Function() _start;
  final Future<void> Function() _cancelTask;
  final Completer<TranscodeOutcome> _done = Completer<TranscodeOutcome>();
  StreamSubscription<TranscodeProgressEvent>? _subscription;
  Future<void>? _cancelRequest;
  Future<void>? _closing;
  String _outputPath;
  bool _started = false;
  bool _disposed = false;
  bool _canceling = false;
  Object? _cancelError;
  TranscodeProgressEvent? _progress;
  TranscodeOutcome? _outcome;

  TranscodeProgressEvent? get progress => _progress;
  TranscodeOutcome? get outcome => _outcome;
  bool get canceling => _canceling;
  Object? get cancelError => _cancelError;

  Future<TranscodeOutcome> run() {
    if (_started || _outcome != null) return _done.future;
    _started = true;
    try {
      _subscription = _start().listen(
        _onEvent,
        onError: (Object error, StackTrace stack) =>
            _finish(TranscodeResult.failed, error: error),
        onDone: () => _finish(
          TranscodeResult.failed,
          error: StateError('Transcoder closed without a terminal result.'),
        ),
      );
    } catch (error) {
      _finish(TranscodeResult.failed, error: error);
    }
    return _done.future;
  }

  void _onEvent(TranscodeProgressEvent event) {
    if (_outcome != null) return;
    _progress = event;
    final path = event.outputPath?.trim();
    if (path != null && path.isNotEmpty) _outputPath = path;
    switch (event.phase.trim().toLowerCase()) {
      case 'completed':
        _finish(TranscodeResult.completed);
      case 'canceled':
        _finish(TranscodeResult.canceled);
      case 'failed':
        _finish(
          TranscodeResult.failed,
          error: event.message?.trim() ?? 'Transcoding failed',
        );
      default:
        _notify();
    }
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  void _finish(TranscodeResult result, {Object? error}) {
    if (_outcome != null) return;
    _outcome = TranscodeOutcome(result, outputPath: _outputPath, error: error);
    _notify();
    // Deferring also handles a synchronously emitting stream's listen callback:
    // the subscription must be assigned before terminal cleanup runs.
    scheduleMicrotask(() async {
      try {
        await _subscription?.cancel();
      } catch (error, stack) {
        logger.w(
          'failed to close transcode subscription',
          error: error,
          stackTrace: stack,
        );
      } finally {
        _subscription = null;
        if (!_done.isCompleted) _done.complete(_outcome!);
      }
    });
  }

  Future<void> cancel() {
    if (_outcome != null || !_started) return Future.value();
    final pending = _cancelRequest;
    if (pending != null) return pending;
    _canceling = true;
    _cancelError = null;
    _notify();
    return _cancelRequest = Future.sync(_cancelTask).catchError((Object error) {
      _cancelRequest = null;
      if (_outcome != null) return;
      _canceling = false;
      _cancelError = error;
      _notify();
    });
  }

  /// Stops observing immediately when the owning view is removed, then asks the
  /// backend to cancel an unfinished task. Repeated close/cancel calls coalesce.
  Future<void> close() => _closing ??= _close();

  Future<void> _close() async {
    final needsCancel = _started && _outcome == null;
    _finish(TranscodeResult.canceled);
    if (needsCancel) {
      try {
        await (_cancelRequest ?? Future.sync(_cancelTask));
      } catch (error, stack) {
        logger.w(
          'failed to cancel disposed transcode task',
          error: error,
          stackTrace: stack,
        );
      }
    }
    await _done.future;
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    unawaited(close());
    super.dispose();
  }
}
