import 'package:stellatune/platform/directory_access_store.dart';
import 'package:stellatune/platform/directory_access_service_factory.dart';

class DirectoryAccessException implements Exception {
  const DirectoryAccessException(this.message);

  final String message;

  @override
  String toString() => message;
}

abstract class DirectoryAccessLease {
  Future<void> release();
}

abstract class DirectoryAccessService {
  static final DirectoryAccessService instance = createDirectoryAccessService();

  Future<String> registerDirectory({
    required String path,
    required DirectoryAccessStore store,
  });

  Future<void> syncStoredDirectories({
    required Iterable<String> paths,
    required DirectoryAccessStore store,
  });

  Future<void> ensureRootsAuthorized({
    required Iterable<String> roots,
    required DirectoryAccessStore store,
  });

  Future<DirectoryAccessLease?> acquireRoots({
    required Iterable<String> roots,
    required DirectoryAccessStore store,
  });

  Future<DirectoryAccessLease?> acquireLocalPath({
    required String path,
    required DirectoryAccessStore store,
  });

  Future<void> forgetDirectory({
    required String path,
    required DirectoryAccessStore store,
  });
}
