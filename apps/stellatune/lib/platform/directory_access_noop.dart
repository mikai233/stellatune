import 'package:stellatune/bridge/api/player/types.dart';
import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/platform/directory_access_store.dart';

class NoopDirectoryAccessService implements DirectoryAccessService {
  const NoopDirectoryAccessService();

  @override
  Future<String> registerDirectory({
    required String path,
    required DirectoryAccessStore store,
  }) async => path.trim();

  @override
  Future<void> syncStoredDirectories({
    required Iterable<String> paths,
    required DirectoryAccessStore store,
  }) async {}

  @override
  Future<void> ensureRootsAuthorized({
    required Iterable<String> roots,
    required DirectoryAccessStore store,
  }) async {}

  @override
  Future<DirectoryAccessLease?> acquireRoots({
    required Iterable<String> roots,
    required DirectoryAccessStore store,
  }) async => null;

  @override
  Future<DirectoryAccessLease?> acquireLocalPath({
    required String path,
    required DirectoryAccessStore store,
  }) async => null;

  @override
  Future<DirectoryAccessLease?> acquireTrackRef({
    required TrackRef track,
    required DirectoryAccessStore store,
  }) async => null;

  @override
  Future<void> forgetDirectory({
    required String path,
    required DirectoryAccessStore store,
  }) async {}
}
