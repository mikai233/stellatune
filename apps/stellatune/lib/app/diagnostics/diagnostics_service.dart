import 'dart:async';
import 'dart:collection';

import 'local_log_store.dart';

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:stellatune/bridge/api/diagnostics.dart' as api;
import 'package:stellatune/bridge/api/error.dart';
import 'package:stellatune/bridge/third_party/stellatune_backend_api/diagnostics/model.dart';

export 'package:stellatune/bridge/third_party/stellatune_backend_api/diagnostics/model.dart';

class ErrorNotice {
  const ErrorNotice(this.message, this.logId);
  final String message;
  final String? logId;
}

/// Application lifetime service, independent of routes and of Rust availability.
class DiagnosticsService {
  static final instance = DiagnosticsService();
  final revision = ValueNotifier(0);
  final visible = ValueNotifier(false);
  final unread = ValueNotifier(0);
  final notice = ValueNotifier<ErrorNotice?>(null);
  final records = ListQueue<LogRecord>();
  final _recordIds = <String>{};
  int _recordsRevision = 0;
  int get recordsRevision => _recordsRevision;
  final _pending = ListQueue<LogRecord>();
  final _reported = Expando<bool>();
  final _notifications = <String, DateTime>{};
  StreamSubscription<LogBatch>? _subscription;
  Timer? _timer;
  Timer? _reconnect;
  Timer? _lossTimer;
  bool _connected = false;
  bool _flushing = false;
  bool _dirty = false;
  bool _closing = false;
  bool _deferred = false;
  int _unreadPending = 0;
  int _lost = 0;
  int _sequence = 0;
  int _bytes = 0;
  int _pendingBytes = 0;
  int _outputOperations = 0;
  DateTime? _lastOutputOperation;
  final _startupSession =
      'session-${DateTime.now().millisecondsSinceEpoch}-flutter';
  String? session;
  String? focusedId;
  int focusRevision = 0;
  String? logDirectory;
  LocalLogStore? _localStore;
  bool chinese = true;
  bool get connected => _connected;

  Future<void> prepareDirectory(String path) async {
    logDirectory = path;
    try {
      _localStore = LocalLogStore(Directory(path), _startupSession);
      await _localStore!.initialize();
      for (final record in records) {
        if (record.source == 'flutter') _localStore!.append(record);
      }
    } catch (_) {
      /* Memory and stderr remain available if storage is unavailable. */
    }
  }

  Future<void> connect() async {
    _closing = false;
    final directory = logDirectory;
    if (directory == null || _connected) return;
    try {
      session = await api.diagnosticsInitialize(logDir: directory);
      _connected = true;
      await _subscription?.cancel();
      _subscribe();
      _timer ??= Timer.periodic(const Duration(milliseconds: 100), (_) {
        unawaited(flush());
        if (_dirty) {
          _dirty = false;
          revision.value++;
        }
      });
      await flush();
    } catch (error, stack) {
      record(
        'ERROR',
        'diagnostics',
        'Unable to connect diagnostic storage',
        error: error,
        stack: stack,
      );
    }
  }

  void _subscribe() {
    _subscription = api.diagnosticsEvents().listen(
      acceptBatch,
      onError: (Object error, StackTrace stack) {
        _scheduleReconnect();
      },
      onDone: _scheduleReconnect,
    );
  }

  /// Merge native batches without rescanning the entire retained buffer.
  void acceptBatch(LogBatch batch) {
    for (final record in batch.records) {
      if (_recordIds.contains(record.id)) continue;
      _add(record);
    }
    if (batch.resync) _dirty = true;
  }

  void _scheduleReconnect() {
    if (!_connected || _reconnect != null) return;
    _reconnect = Timer(const Duration(seconds: 1), () async {
      _reconnect = null;
      await _subscription?.cancel();
      if (_connected) _subscribe();
    });
  }

  static final _sensitiveField = RegExp(
    r'authorization|cookie|access_token|refresh_token|password|passwd|token=|token:|"token"|api_key',
    caseSensitive: false,
  );
  static String redact(String value) => value
      .split('\n')
      .map((line) {
        final match = _sensitiveField.firstMatch(line);
        return match == null
            ? line
            : '${line.substring(0, match.start)}[redacted]';
      })
      .join('\n');
  String record(
    String level,
    String target,
    String message, {
    Object? error,
    StackTrace? stack,
  }) {
    final id = 'flutter:$_startupSession:${++_sequence}';
    final record = LogRecord(
      id: id,
      session: session ?? _startupSession,
      timestampMs: DateTime.now().millisecondsSinceEpoch.toDouble(),
      level: level,
      source: 'flutter',
      target: target,
      message: redact(message),
      details: redact(
        '${error ?? ''}${stack == null ? '' : '\nDart stack:\n$stack'}',
      ),
    );
    _add(record);
    _pending.add(record);
    _pendingBytes += _size(record);
    var dropped = 0;
    while (_pending.length > 5000 || _pendingBytes > 8 * 1024 * 1024) {
      _pendingBytes -= _size(_pending.removeFirst());
      dropped++;
    }
    _lost += dropped;
    if (!_connected && dropped > 0) {
      _lossTimer ??= Timer(const Duration(milliseconds: 100), () {
        _lossTimer = null;
        if (!_connected) _recordLocalLoss();
      });
    }
    if (!_connected) _localStore?.append(record);
    return id;
  }

  static int _size(LogRecord r) =>
      (r.message.length + r.details.length + 256) * 2;
  void _recordLocalLoss() {
    if (_lost == 0) return;
    final record = LogRecord(
      id: 'flutter:$_startupSession:loss:${++_sequence}',
      session: session ?? _startupSession,
      timestampMs: DateTime.now().millisecondsSinceEpoch.toDouble(),
      level: 'WARN',
      source: 'flutter',
      target: 'diagnostics',
      message: 'Dropped $_lost pending log records: queue capacity exceeded',
      details: '',
    );
    _lost = 0;
    _add(record);
    _localStore?.append(record);
  }

  void _add(LogRecord record) {
    records.add(record);
    _recordIds.add(record.id);
    _recordsRevision++;
    _bytes += _size(record);
    while (records.length > 5000 || _bytes > 8 * 1024 * 1024) {
      final removed = records.removeFirst();
      _bytes -= _size(removed);
      _recordIds.remove(removed.id);
    }
    if (record.level == 'ERROR' && !visible.value) _unreadPending++;
    _dirty = true;
    void notify() {
      unread.value += _unreadPending;
      _unreadPending = 0;
      if (!_connected) revision.value++;
    }

    if (!_deferred) {
      _deferred = true;
      scheduleMicrotask(() {
        _deferred = false;
        notify();
      });
    }
  }

  Future<void> flush() async {
    if (!_connected || _flushing || _pending.isEmpty) return;
    _flushing = true;
    final batch = <LogRecord>[];
    if (_lost > 0) {
      final loss = LogRecord(
        id: 'flutter:$_startupSession:loss:${++_sequence}',
        session: session ?? _startupSession,
        timestampMs: DateTime.now().millisecondsSinceEpoch.toDouble(),
        level: 'WARN',
        source: 'flutter',
        target: 'diagnostics',
        message: 'Dropped $_lost pending log records: queue capacity exceeded',
        details: '',
      );
      _add(loss);
      batch.add(loss);
      _lost = 0;
    }
    while (_pending.isNotEmpty && batch.length < 200) {
      final record = _pending.removeFirst();
      _pendingBytes -= _size(record);
      batch.add(record);
    }
    try {
      await api.diagnosticsAppend(records: batch);
    } catch (_) {
      _connected = false;
      for (final record in batch.reversed) {
        _pending.addFirst(record);
        _pendingBytes += _size(record);
      }
      if (!_closing) {
        _reconnect ??= Timer(const Duration(seconds: 1), () {
          _reconnect = null;
          unawaited(connect());
        });
      }
    } finally {
      _flushing = false;
    }
  }

  /// Controllers call this after their own rollback. Logging alone never notifies.
  void report(
    Object error, {
    StackTrace? stack,
    String operation = 'operation',
    bool notify = true,
  }) {
    final typed = error is AppError ? error : null;
    if (typed?.category == ErrorCategory.cancelled) notify = false;
    if (typed?.operation == 'playback_event' &&
        typed?.category == ErrorCategory.unavailable &&
        (_outputOperations > 0 ||
            (_lastOutputOperation?.isAfter(
                  DateTime.now().subtract(const Duration(seconds: 2)),
                ) ??
                false))) {
      notify = false;
    }
    final fingerprint = typed == null
        ? '$operation:${error.runtimeType}:${error.toString()}'
        : '${typed.category}:${typed.fingerprint}:${typed.context}';
    final now = DateTime.now();
    final duplicate =
        _notifications[fingerprint]?.isAfter(
          now.subtract(const Duration(seconds: 2)),
        ) ??
        false;
    _notifications.removeWhere(
      (_, time) => time.isBefore(now.subtract(const Duration(seconds: 2))),
    );
    if (notify) _notifications[fingerprint] = now;
    final alreadyReported =
        error is! String &&
        error is! num &&
        error is! bool &&
        _reported[error] == true;
    if (notify && error is! String && error is! num && error is! bool) {
      _reported[error] = true;
    }
    final id =
        typed?.diagnosticId ??
        record(
          'ERROR',
          operation,
          'Operation failed',
          error: error,
          stack: stack,
        );
    if (typed != null && stack != null && !alreadyReported) {
      record(
        typed.category == ErrorCategory.cancelled ? 'DEBUG' : 'ERROR',
        operation,
        'Rust operation failed [${typed.diagnosticId}]',
        stack: stack,
      );
    }
    if (notify && !duplicate && !alreadyReported) {
      final value = ErrorNotice(messageFor(error, operation: operation), id);
      scheduleMicrotask(() => notice.value = value);
    }
  }

  void beginOutputOperation() {
    _outputOperations++;
  }

  void endOutputOperation() {
    if (_outputOperations > 0) _outputOperations--;
    _lastOutputOperation = DateTime.now();
  }

  String messageFor(Object? error, {String operation = 'operation'}) {
    if (error is AppError) operation = error.operation;
    if (operation.contains('output') ||
        operation.contains('device') ||
        (error is AppError && error.category == ErrorCategory.unavailable)) {
      return chinese ? '无法使用此输出设备' : 'This output device is unavailable';
    }
    if (operation.contains('plugin')) {
      return chinese ? '插件操作失败' : 'Plugin operation failed';
    }
    if (operation.contains('library') || operation.contains('scan')) {
      return chinese ? '音乐库操作失败' : 'Library operation failed';
    }
    if (operation.contains('lyrics')) {
      return chinese ? '歌词加载失败' : 'Unable to load lyrics';
    }
    if (operation.contains('dlna')) {
      return chinese ? '无法连接播放设备' : 'Unable to connect to the renderer';
    }
    if (operation.contains('transcode')) {
      return chinese ? '音频转换失败' : 'Audio conversion failed';
    }
    if (operation.contains('play') ||
        operation.contains('seek') ||
        operation.contains('queue')) {
      return chinese ? '播放操作失败' : 'Playback operation failed';
    }
    return chinese ? '操作未能完成，请查看日志' : 'Operation failed. See logs for details';
  }

  String failureMessage(
    Object error, {
    String operation = 'operation',
    bool notify = true,
  }) {
    report(error, operation: operation, notify: notify);
    return messageFor(error, operation: operation);
  }

  void open([String? id]) {
    focusedId = id;
    focusRevision++;
    unread.value = 0;
    visible.value = true;
    revision.value++;
  }

  void close() {
    visible.value = false;
  }

  Future<List<String>> sessions() async => _connected
      ? api.diagnosticsSessions()
      : await _localStore?.sessions() ?? [];
  Future<LogRecord?> detail(String id) async {
    final local = records.where((r) => r.id == id).firstOrNull;
    if (local != null && local.source == 'flutter') return local;
    if (_connected) {
      try {
        return await api.diagnosticsDetail(id: id);
      } catch (_) {}
    }
    if (local != null) return local;
    final store = _localStore;
    if (store != null) {
      await for (final record in store.read()) {
        if (record.id == id) return record;
      }
    }
    return null;
  }

  Future<LogPage> query(
    String selected,
    int offset,
    String level,
    String source,
    String search,
  ) async {
    if (_connected) {
      return api.diagnosticsQuery(
        session: selected,
        offset: offset,
        limit: 200,
        level: level,
        source: source,
        search: search,
      );
    }
    final result = <LogRecord>[];
    var matched = 0;
    final store = _localStore;
    if (store != null) {
      await for (final r in store.read(selected)) {
        if ((level.isNotEmpty && r.level != level) ||
            (source.isNotEmpty && r.source != source) ||
            !('${r.message} ${r.details} ${r.target}').toLowerCase().contains(
              search.toLowerCase(),
            )) {
          continue;
        }
        if (matched++ < offset) continue;
        if (result.length == 200) {
          return LogPage(records: result, nextOffset: offset + 200);
        }
        result.add(r);
      }
    }
    return LogPage(records: result);
  }

  Future<void> export(String selected, String destination) async {
    if (_connected && selected != 'startup') {
      await api.diagnosticsExport(session: selected, destination: destination);
    } else {
      final store = _localStore;
      if (store != null && selected != 'startup') {
        final sink = File(destination).openWrite();
        try {
          await for (final r in store.read(selected)) {
            sink.writeln(
              '${r.timestampMs} ${r.level} [${r.source}] ${r.target}\n${r.message}\n${r.details}',
            );
          }
        } finally {
          await sink.close();
        }
        return;
      }
      await File(destination).writeAsString(
        records
            .map(
              (r) => '${r.timestampMs} ${r.level} ${r.message}\n${r.details}',
            )
            .join('\n'),
      );
    }
  }

  Future<void> shutdown() async {
    _closing = true;
    _lossTimer?.cancel();
    _lossTimer = null;
    if (!_connected) _recordLocalLoss();
    _timer?.cancel();
    _timer = null;
    _reconnect?.cancel();
    _reconnect = null;
    while (_flushing) {
      await Future<void>.delayed(const Duration(milliseconds: 10));
    }
    while (_pending.isNotEmpty && _connected) {
      await flush();
    }
    if (_connected) {
      try {
        await api.diagnosticsFlush();
      } catch (_) {}
    }
    _connected = false;
    await _subscription?.cancel();
    await _localStore?.flush();
  }
}
