import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:stellatune/bridge/third_party/stellatune_backend_api/diagnostics/model.dart';

/// Loader-failure fallback. Uses the same JSONL format as the Rust store.
class LocalLogStore {
  LocalLogStore(this.directory, this.session);
  final Directory directory;
  final String session;
  Future<void> _writes = Future.value();
  int _queued = 0, _written = 0, _part = 0;
  int dropped = 0;
  static const fileLimit = 10 * 1024 * 1024;

  Future<List<File>> _files() async {
    if (!await directory.exists()) return [];
    final files = await directory
        .list()
        .where(
          (f) =>
              f is File &&
              f.uri.pathSegments.last.startsWith('session-') &&
              f.path.endsWith('.jsonl'),
        )
        .cast<File>()
        .toList();
    files.sort((a, b) => a.path.compareTo(b.path));
    return files;
  }

  Future<void> initialize() async {
    await directory.create(recursive: true);
    await _cleanup();
  }

  Future<void> _cleanup() async {
    final retained = <(File, int)>[];
    for (final file in await _files()) {
      final stat = await file.stat();
      if (DateTime.now().difference(stat.modified) > const Duration(days: 7)) {
        await file.delete();
      } else {
        retained.add((file, stat.size));
      }
    }
    var total = retained.fold(0, (sum, item) => sum + item.$2);
    for (final item in retained) {
      if (total <= 90 * 1024 * 1024) break;
      await item.$1.delete();
      total -= item.$2;
    }
  }

  bool append(LogRecord record) {
    final bytes = utf8.encode(
      '${jsonEncode({'id': record.id, 'session': session, 'timestamp_ms': record.timestampMs, 'level': record.level, 'source': record.source, 'target': record.target, 'message': record.message, 'details': record.details})}\n',
    );
    if (_queued + bytes.length > 8 * 1024 * 1024 || bytes.length > fileLimit) {
      dropped++;
      return false;
    }
    _queued += bytes.length;
    _writes = _writes
        .then((_) async {
          if (_written + bytes.length > fileLimit) {
            _part++;
            _written = 0;
            await _cleanup();
          }
          final file = File(
            '${directory.path}/$session-${_part.toString().padLeft(5, '0')}.jsonl',
          );
          await file.writeAsBytes(bytes, mode: FileMode.append, flush: true);
          _written += bytes.length;
        })
        .catchError((Object _) {
          dropped++;
        })
        .whenComplete(() => _queued -= bytes.length);
    return true;
  }

  Future<void> flush() => _writes;
  Future<List<String>> sessions() async => (await _files())
      .map((f) {
        final name = f.uri.pathSegments.last;
        return name.substring(0, name.lastIndexOf('-'));
      })
      .toSet()
      .toList()
      .reversed
      .toList();

  Stream<LogRecord> read([String? selected]) async* {
    await flush();
    for (final file in await _files()) {
      if (selected != null &&
          !file.uri.pathSegments.last.startsWith('$selected-')) {
        continue;
      }
      try {
        await for (final line
            in file
                .openRead()
                .transform(utf8.decoder)
                .transform(const LineSplitter())) {
          try {
            final r = jsonDecode(line) as Map<String, dynamic>;
            yield LogRecord(
              id: r['id'],
              session: r['session'],
              timestampMs: (r['timestamp_ms'] as num).toDouble(),
              level: r['level'],
              source: r['source'],
              target: r['target'],
              message: r['message'],
              details: r['details'],
              pluginId: r['plugin_id'],
              generation: r['generation'],
              fingerprint: r['fingerprint'],
            );
          } on FormatException {
            /* Partial last line after a crash. */
          }
        }
      } on FileSystemException {
        /* Retention may have removed a segment. */
      }
    }
  }
}
