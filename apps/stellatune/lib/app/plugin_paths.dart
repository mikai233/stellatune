import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

Future<String> defaultPluginDir() async {
  final dir = await getApplicationSupportDirectory();
  return p.join(dir.path, 'plugins');
}
