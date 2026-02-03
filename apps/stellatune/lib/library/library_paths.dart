import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

Future<String> defaultLibraryDbPath() async {
  // - Windows/macOS/Linux: application support dir is user-scoped and appropriate for databases.
  // - Android/iOS: maps to app sandbox data directories.
  final dir = await getApplicationSupportDirectory();
  return p.join(dir.path, 'stellatune.sqlite3');
}
