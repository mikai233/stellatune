import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import '../tool/src/dart_loc.dart';

void main() {
  test('counts tokens, not comments or whitespace, including braces', () {
    const source =
        '// heading\n\n/* block\n * comment */\nvoid main() {\n'
        '  final url = "https://example.test/*text*/"; // trailing\n}\n';
    expect(effectiveDartLines(source), 3);
    expect(effectiveDartLines(source.replaceAll('\n', '\r\n')), 3);
  });
  test(
    'multiline strings count nonblank content and nested comments do not',
    () {
      const source =
          "/* outer /* inner */ comment */\nconst value = '''\n"
          "// string content\n\n/* also string content */\n''';\n";
      expect(effectiveDartLines(source), 4);
    },
  );
  test(
    'generated exclusion does not hide handwritten bridge or test modules',
    () async {
      final directory = await Directory.systemTemp.createTemp('dart_loc_');
      addTearDown(() => directory.delete(recursive: true));
      for (final path in [
        'lib/bridge/frb_generated.dart',
        'lib/bridge/lease.dart',
        'test/sample.dart',
      ]) {
        final file = File('${directory.path}/$path');
        await file.parent.create(recursive: true);
        await file.writeAsString(
          List.generate(1201, (i) => 'const value$i = $i;').join('\n'),
        );
      }
      final sizes = dartFileSizes(directory);
      expect(sizes.map((file) => file.path).toSet(), {
        'lib/bridge/lease.dart',
        'test/sample.dart',
      });
      expect(sizes.every((file) => file.lines == 1201), isTrue);
    },
  );
}
