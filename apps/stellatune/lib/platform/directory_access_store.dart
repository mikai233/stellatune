abstract interface class DirectoryAccessStore {
  Map<String, String> get macosDirectoryBookmarks;

  String? macosDirectoryBookmarkForPath(String path);

  Future<void> setMacosDirectoryBookmark({
    required String path,
    required String bookmark,
  });

  Future<void> removeMacosDirectoryBookmark(String path);
}
