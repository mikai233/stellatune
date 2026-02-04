import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

Future<String> defaultPluginDir() async {
  final dir = await getApplicationSupportDirectory();
  return p.join(dir.path, 'plugins');
}

String disabledPluginsFilePath(String pluginDir) {
  return p.join(pluginDir, 'disabled_plugins.json');
}

Future<void> writeDisabledPluginsFile({
  required String pluginDir,
  required Iterable<String> disabledIds,
}) async {
  final ids =
      disabledIds
          .map((s) => s.trim())
          .where((s) => s.isNotEmpty)
          .toSet()
          .toList(growable: false)
        ..sort();

  await Directory(pluginDir).create(recursive: true);
  final file = File(disabledPluginsFilePath(pluginDir));
  await file.writeAsString(jsonEncode(ids));
}
