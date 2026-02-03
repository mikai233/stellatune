import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logger/logger.dart';

final loggerProvider = Provider<Logger>((ref) {
  return Logger(
    printer: PrettyPrinter(
      methodCount: 0,
      errorMethodCount: 8,
      lineLength: 120,
      colors: true,
      printEmojis: false,
      dateTimeFormat: DateTimeFormat.onlyTimeAndSinceStart,
    ),
  );
});
