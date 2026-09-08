import 'dart:io';

import 'src/dart_loc.dart';

void main(List<String> arguments) {
  final root = arguments.isEmpty
      ? Directory.current
      : Directory(arguments.single);
  if (!File('${root.path}/pubspec.yaml').existsSync()) {
    stderr.writeln('Run from apps/stellatune or pass its directory.');
    exitCode = 2;
    return;
  }
  final files = dartFileSizes(root);
  final violations = files
      .where((file) => file.lines > maxEffectiveDartLines)
      .toList();
  stdout.writeln(
    'Checked ${files.length} handwritten Dart files, '
    '${files.fold<int>(0, (sum, file) => sum + file.lines)} effective lines; '
    'limit $maxEffectiveDartLines per file.',
  );
  for (final file in violations) {
    stderr.writeln('${file.lines}\t${file.path}');
  }
  if (violations.isNotEmpty) exitCode = 1;
}
