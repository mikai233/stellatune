import 'package:flutter/services.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/platform/directory_access_store.dart';

class _MacosDirectoryLease implements DirectoryAccessLease {
  _MacosDirectoryLease(this._service, this._paths);

  final MacosDirectoryAccessService _service;
  final List<String> _paths;
  bool _released = false;

  @override
  Future<void> release() async {
    if (_released) return;
    _released = true;
    await _service.releasePaths(_paths);
  }
}

class MacosDirectoryAccessService implements DirectoryAccessService {
  static const MethodChannel _channel = MethodChannel(
    'stellatune/macos_directory_access',
  );

  const MacosDirectoryAccessService();

  @override
  Future<String> registerDirectory({
    required String path,
    required DirectoryAccessStore store,
  }) async {
    final trimmed = path.trim();
    if (trimmed.isEmpty) {
      return trimmed;
    }
    final normalized = _normalizePath(trimmed);

    final response = await _invoke('createDirectoryBookmark', {
      'path': normalized,
    });
    final resolvedPath = _pathField(response, 'path') ?? normalized;
    final bookmark = _bookmarkField(response, 'bookmark');
    if (bookmark != null) {
      await store.setMacosDirectoryBookmark(
        path: resolvedPath,
        bookmark: bookmark,
      );
      if (resolvedPath != normalized) {
        await store.removeMacosDirectoryBookmark(normalized);
      }
    }
    return resolvedPath;
  }

  @override
  Future<void> syncStoredDirectories({
    required Iterable<String> paths,
    required DirectoryAccessStore store,
  }) async {
    for (final rawPath in paths) {
      final path = _normalizePath(rawPath);
      if (path.isEmpty) continue;
      final bookmark = store.macosDirectoryBookmarkForPath(path);
      if (bookmark == null || bookmark.isEmpty) {
        logger.w('missing macos directory bookmark for root: $path');
        continue;
      }
      try {
        final response = await _invoke('resolveDirectoryBookmark', {
          'bookmark': bookmark,
        });
        await _updateBookmarkFromResponse(
          store: store,
          fallbackPath: path,
          response: response,
        );
      } catch (e, s) {
        logger.w(
          'failed to resolve macos directory bookmark for root: $path',
          error: e,
          stackTrace: s,
        );
      }
    }
  }

  @override
  Future<void> ensureRootsAuthorized({
    required Iterable<String> roots,
    required DirectoryAccessStore store,
  }) async {
    final missing = <String>[];
    for (final rawRoot in roots) {
      final root = _normalizePath(rawRoot);
      if (root.isEmpty) continue;
      final bookmark = store.macosDirectoryBookmarkForPath(root);
      if (bookmark == null || bookmark.isEmpty) {
        missing.add(root);
      }
    }
    if (missing.isEmpty) return;
    throw DirectoryAccessException(
      'Some library folders need to be reauthorized in macOS: ${missing.join(', ')}',
    );
  }

  @override
  Future<DirectoryAccessLease?> acquireRoots({
    required Iterable<String> roots,
    required DirectoryAccessStore store,
  }) async {
    final resolvedPaths = <String>[];
    final seen = <String>{};
    for (final rawRoot in roots) {
      final root = _normalizePath(rawRoot);
      if (root.isEmpty || !seen.add(root)) continue;
      final bookmark = store.macosDirectoryBookmarkForPath(root);
      if (bookmark == null || bookmark.isEmpty) {
        throw DirectoryAccessException(
          'Library folder needs to be reauthorized in macOS: $root',
        );
      }
      final response = await _invoke('startAccessingDirectory', {
        'bookmark': bookmark,
      });
      final resolvedPath = await _updateBookmarkFromResponse(
        store: store,
        fallbackPath: root,
        response: response,
      );
      resolvedPaths.add(resolvedPath);
    }
    if (resolvedPaths.isEmpty) return null;
    return _MacosDirectoryLease(this, resolvedPaths);
  }

  @override
  Future<DirectoryAccessLease?> acquireLocalPath({
    required String path,
    required DirectoryAccessStore store,
  }) async {
    final normalizedPath = _normalizePath(path);
    if (normalizedPath.isEmpty) return null;
    final root = _bestMatchingAuthorizedRoot(normalizedPath, store);
    if (root == null) {
      throw DirectoryAccessException(
        'This local file is outside authorized library folders on macOS: $normalizedPath',
      );
    }
    final bookmark = store.macosDirectoryBookmarkForPath(root);
    if (bookmark == null || bookmark.isEmpty) {
      throw DirectoryAccessException(
        'Library folder needs to be reauthorized in macOS: $root',
      );
    }
    final response = await _invoke('startAccessingDirectory', {
      'bookmark': bookmark,
    });
    final resolvedPath = await _updateBookmarkFromResponse(
      store: store,
      fallbackPath: root,
      response: response,
    );
    return _MacosDirectoryLease(this, [resolvedPath]);
  }

  @override
  Future<void> forgetDirectory({
    required String path,
    required DirectoryAccessStore store,
  }) async {
    final normalized = _normalizePath(path);
    if (normalized.isEmpty) return;
    await store.removeMacosDirectoryBookmark(normalized);
    await releasePaths([normalized]);
  }

  Future<void> releasePaths(List<String> paths) async {
    for (final path in paths) {
      try {
        await _channel.invokeMethod<void>('stopAccessingDirectory', {
          'path': path,
        });
      } catch (e, s) {
        logger.w(
          'failed to stop macos directory access for root: $path',
          error: e,
          stackTrace: s,
        );
      }
    }
  }

  Future<String> _updateBookmarkFromResponse({
    required DirectoryAccessStore store,
    required String fallbackPath,
    required Map<Object?, Object?>? response,
  }) async {
    final resolvedPath = _pathField(response, 'path') ?? fallbackPath;
    final bookmark = _bookmarkField(response, 'bookmark');
    if (bookmark != null) {
      await store.setMacosDirectoryBookmark(
        path: resolvedPath,
        bookmark: bookmark,
      );
      if (resolvedPath != fallbackPath) {
        await store.removeMacosDirectoryBookmark(fallbackPath);
      }
    }
    return resolvedPath;
  }

  Future<Map<Object?, Object?>?> _invoke(
    String method,
    Map<String, Object?> arguments,
  ) async {
    final response = await _channel.invokeMethod<Object?>(method, arguments);
    if (response == null) return null;
    if (response is Map<Object?, Object?>) return response;
    throw PlatformException(
      code: 'invalid_response',
      message: 'Expected map response for $method',
    );
  }

  String? _bestMatchingAuthorizedRoot(String path, DirectoryAccessStore store) {
    String? bestMatch;
    for (final root in store.macosDirectoryBookmarks.keys) {
      if (!_isSameOrChildPath(path, root)) continue;
      if (bestMatch == null || root.length > bestMatch.length) {
        bestMatch = root;
      }
    }
    return bestMatch;
  }

  bool _isSameOrChildPath(String path, String root) {
    if (path == root) return true;
    if (!path.startsWith(root)) return false;
    return path.length > root.length && path[root.length] == '/';
  }

  String? _pathField(Map<Object?, Object?>? response, String key) {
    final value = response?[key];
    if (value is String && value.trim().isNotEmpty) {
      return _normalizePath(value);
    }
    return null;
  }

  String? _bookmarkField(Map<Object?, Object?>? response, String key) {
    final value = response?[key];
    if (value is String && value.trim().isNotEmpty) {
      return value.trim();
    }
    return null;
  }

  String _normalizePath(String path) {
    var value = path.trim().replaceAll('\\', '/');
    while (value.length > 1 && value.endsWith('/')) {
      value = value.substring(0, value.length - 1);
    }
    return value;
  }
}
