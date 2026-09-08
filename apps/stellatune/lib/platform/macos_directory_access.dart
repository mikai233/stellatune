import 'package:flutter/services.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/platform/directory_access_store.dart';

class _MacosDirectoryLease implements DirectoryAccessLease {
  _MacosDirectoryLease(this._service, this._paths);

  final MacosDirectoryAccessService _service;
  final List<String> _paths;
  Future<void>? _release;

  @override
  Future<void> release() => _release ??= _service.releasePaths(_paths);
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
    try {
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
        // Own the access as soon as native acquisition succeeds. Persisting a
        // refreshed bookmark may fail too, including when a folder has moved.
        resolvedPaths.add(_pathField(response, 'path') ?? root);
        await _updateBookmarkFromResponse(
          store: store,
          fallbackPath: root,
          response: response,
        );
      }
    } catch (_) {
      await releasePaths(resolvedPaths.reversed.toList());
      rethrow;
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
    return acquireRoots(roots: [root], store: store);
  }

  @override
  Future<void> forgetDirectory({
    required String path,
    required DirectoryAccessStore store,
  }) async {
    final normalized = _normalizePath(path);
    if (normalized.isEmpty) return;
    await store.removeMacosDirectoryBookmark(normalized);
    // Existing leases own their start/stop pairs until their consumers finish.
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
