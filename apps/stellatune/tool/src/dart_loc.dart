import 'dart:io';

import 'package:analyzer/dart/analysis/utilities.dart';

const maxEffectiveDartLines = 1200;

/// Explicit paths, so newly added handwritten bridge modules are checked too.
const generatedDartFiles = {
  'lib/bridge/frb_generated.dart',
  'lib/bridge/frb_generated.io.dart',
  'lib/bridge/frb_generated.web.dart',
  'lib/bridge/api/player/types.freezed.dart',
  'lib/bridge/api/player/types.dart',
  'lib/bridge/api/player.dart',
  'lib/bridge/api/library.dart',
  'lib/bridge/api/dlna.dart',
  'lib/bridge/api/dlna/types.dart',
  'lib/bridge/api/player/queue.dart',
  'lib/bridge/api/player/transcode.dart',
  'lib/bridge/api/runtime.dart',
  'lib/bridge/third_party/stellatune_backend_api/lyrics_types.freezed.dart',
  'lib/bridge/third_party/stellatune_backend_api/lyrics_types.dart',
  'lib/bridge/third_party/stellatune_backend_api/player_service/metadata.dart',
  'lib/bridge/third_party/stellatune_library.freezed.dart',
  'lib/bridge/third_party/stellatune_library.dart',
  'lib/bridge/third_party/stellatune_library/service.dart',
  'lib/l10n/app_localizations.dart',
  'lib/l10n/app_localizations_en.dart',
  'lib/l10n/app_localizations_zh.dart',
};

/// Nonblank lines intersecting a non-comment Dart token. Braces count as code.
int effectiveDartLines(String source) {
  final parsed = parseString(content: source, throwIfDiagnostics: false);
  final lines = source.split('\n');
  final counted = <int>{};
  var token = parsed.unit.beginToken;
  while (!token.isEof) {
    if (token.length > 0) {
      final first = parsed.lineInfo.getLocation(token.offset).lineNumber;
      final last = parsed.lineInfo.getLocation(token.end - 1).lineNumber;
      for (var number = first; number <= last; number++) {
        if (lines[number - 1].trim().isNotEmpty) counted.add(number);
      }
    }
    token = token.next!;
  }
  return counted.length;
}

List<({String path, int lines})> dartFileSizes(Directory root) {
  final absoluteRoot = root.absolute;
  final result = <({String path, int lines})>[];
  for (final name in ['lib', 'test', 'tool']) {
    final directory = Directory('${absoluteRoot.path}/$name');
    if (!directory.existsSync()) continue;
    for (final file
        in directory
            .listSync(recursive: true, followLinks: false)
            .whereType<File>()) {
      if (!file.path.endsWith('.dart')) continue;
      final path = file.path
          .substring(absoluteRoot.path.length + 1)
          .replaceAll('\\', '/');
      if (generatedDartFiles.contains(path)) continue;
      result.add((
        path: path,
        lines: effectiveDartLines(file.readAsStringSync()),
      ));
    }
  }
  result.sort((a, b) => b.lines.compareTo(a.lines));
  return result;
}
