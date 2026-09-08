import 'package:stellatune/app/logging.dart';
import 'package:stellatune/platform/directory_access_service.dart';

/// Owns the queue's directory access, including acquisitions still in flight.
class QueuePathLeases {
  QueuePathLeases(this._acquire);
  final Future<DirectoryAccessLease?> Function(String path) _acquire;
  final Map<String, _PendingLease> _entries = {};
  final Set<Future<void>> _releases = {};
  bool _closed = false;
  Future<void>? _disposal;

  Future<void> retain(Iterable<String> paths) async {
    if (_closed) throw StateError('Queue directory access is disposed');
    final requested = <String, _PendingLease>{};
    // Reserve all entries before awaiting, so overlapping requests share them.
    for (final path in paths.where((path) => path.isNotEmpty).toSet()) {
      final entry = _entries.putIfAbsent(
        path,
        () => _PendingLease(() => _acquire(path)),
      );
      entry.waiters++;
      requested[path] = entry;
    }
    try {
      await Future.wait(requested.values.map((entry) => entry.ready));
      for (final entry in requested.values) {
        entry.retained = true;
      }
    } finally {
      final abandoned = <_PendingLease>[];
      for (final MapEntry(:key, :value) in requested.entries) {
        value.waiters--;
        if (!value.retained &&
            value.waiters == 0 &&
            identical(_entries[key], value)) {
          _entries.remove(key);
          abandoned.add(value);
        }
      }
      // A failed batch must not strand the paths it alone acquired. Other
      // concurrent requests and the existing queue keep their own claims.
      await _release(abandoned);
    }
  }

  Future<void> releaseExcept(Iterable<String> paths) {
    final retained = paths.toSet();
    final removed = <_PendingLease>[];
    for (final path in _entries.keys.toList()) {
      if (!retained.contains(path)) removed.add(_entries.remove(path)!);
    }
    return _release(removed);
  }

  Future<void> dispose() {
    _closed = true;
    return _disposal ??= _dispose();
  }

  Future<void> _dispose() async {
    await releaseExcept(const []);
    // A prior queue update may already have detached an entry for release.
    await Future.wait(_releases.toList());
  }

  Future<void> _release(Iterable<_PendingLease> entries) {
    late final Future<void> releasing;
    releasing = Future.wait(
      entries.map((entry) async {
        try {
          await entry.release();
        } catch (error, stack) {
          logger.w(
            'failed to release queue directory access',
            error: error,
            stackTrace: stack,
          );
        }
      }),
    ).then<void>((_) {}).whenComplete(() => _releases.remove(releasing));
    _releases.add(releasing);
    return releasing;
  }
}

class _PendingLease {
  _PendingLease(Future<DirectoryAccessLease?> Function() acquire)
    : ready = Future.sync(acquire);

  final Future<DirectoryAccessLease?> ready;
  int waiters = 0;
  bool retained = false;
  Future<void>? _release;

  Future<void> release() => _release ??= _releaseWhenReady();

  Future<void> _releaseWhenReady() async {
    DirectoryAccessLease? lease;
    try {
      lease = await ready;
    } catch (_) {
      // The retain operation reports acquisition failures to its caller.
      return;
    }
    await lease?.release();
  }
}
