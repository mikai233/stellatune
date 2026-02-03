import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logger/logger.dart';

class RustLikePrinter extends LogPrinter {
  RustLikePrinter({this.target = 'stellatune-flutter', this.useUtc = false});

  final String target;
  final bool useUtc;

  @override
  List<String> log(LogEvent event) {
    final ts = (useUtc ? event.time.toUtc() : event.time).toIso8601String();
    final level = _levelLabel(event.level).padLeft(5);

    var msg = _oneLine(event.message);
    if (event.error != null) {
      msg = '$msg err=${_oneLine(event.error)}';
    }
    if (event.stackTrace != null) {
      msg = '$msg st=${_oneLine(event.stackTrace)}';
    }

    return ['$ts $level $target $msg'];
  }

  static String _levelLabel(Level level) {
    return switch (level) {
      Level.trace => 'TRACE',
      Level.debug => 'DEBUG',
      Level.info => 'INFO',
      Level.warning => 'WARN',
      Level.error => 'ERROR',
      Level.fatal => 'FATAL',
      Level.all => 'ALL',
      _ => level.name.toUpperCase(),
    };
  }

  static String _oneLine(Object? value) {
    return (value ?? '').toString().replaceAll(RegExp(r'\s+'), ' ').trim();
  }
}

final loggerProvider = Provider<Logger>((ref) {
  return Logger(printer: RustLikePrinter());
});
