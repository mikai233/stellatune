import 'package:flutter/foundation.dart';
import 'package:stellatune/bridge/bridge.dart';

const _sentinel = Object();

@immutable
class LibraryScanProgress {
  const LibraryScanProgress({
    required this.scanned,
    required this.updated,
    required this.skipped,
    required this.errors,
  });

  const LibraryScanProgress.zero()
    : scanned = 0,
      updated = 0,
      skipped = 0,
      errors = 0;

  final int scanned;
  final int updated;
  final int skipped;
  final int errors;
}

@immutable
class LibraryState {
  const LibraryState({
    required this.roots,
    required this.folders,
    required this.excludedFolders,
    required this.playlists,
    required this.selectedFolder,
    required this.selectedPlaylistId,
    required this.includeSubfolders,
    required this.query,
    required this.results,
    required this.likedTrackIds,
    required this.isScanning,
    required this.progress,
    required this.lastFinishedMs,
    required this.lastError,
    required this.lastLog,
  });

  const LibraryState.initial()
    : roots = const [],
      folders = const [],
      excludedFolders = const [],
      playlists = const [],
      selectedFolder = '',
      selectedPlaylistId = null,
      includeSubfolders = true,
      query = '',
      results = const [],
      likedTrackIds = const <int>{},
      isScanning = false,
      progress = const LibraryScanProgress.zero(),
      lastFinishedMs = null,
      lastError = null,
      lastLog = '';

  final List<String> roots;
  final List<String> folders;
  final List<String> excludedFolders;
  final List<PlaylistLite> playlists;

  /// Normalized folder path. Empty string means "All music".
  final String selectedFolder;
  final int? selectedPlaylistId;
  final bool includeSubfolders;
  final String query;
  final List<TrackLite> results;
  final Set<int> likedTrackIds;
  final bool isScanning;
  final LibraryScanProgress progress;
  final int? lastFinishedMs;
  final String? lastError;
  final String lastLog;

  LibraryState copyWith({
    List<String>? roots,
    List<String>? folders,
    List<String>? excludedFolders,
    List<PlaylistLite>? playlists,
    String? selectedFolder,
    Object? selectedPlaylistId = _sentinel,
    bool? includeSubfolders,
    String? query,
    List<TrackLite>? results,
    Set<int>? likedTrackIds,
    bool? isScanning,
    LibraryScanProgress? progress,
    int? lastFinishedMs,
    String? lastError,
    String? lastLog,
  }) {
    return LibraryState(
      roots: roots ?? this.roots,
      folders: folders ?? this.folders,
      excludedFolders: excludedFolders ?? this.excludedFolders,
      playlists: playlists ?? this.playlists,
      selectedFolder: selectedFolder ?? this.selectedFolder,
      selectedPlaylistId: identical(selectedPlaylistId, _sentinel)
          ? this.selectedPlaylistId
          : selectedPlaylistId as int?,
      includeSubfolders: includeSubfolders ?? this.includeSubfolders,
      query: query ?? this.query,
      results: results ?? this.results,
      likedTrackIds: likedTrackIds ?? this.likedTrackIds,
      isScanning: isScanning ?? this.isScanning,
      progress: progress ?? this.progress,
      lastFinishedMs: lastFinishedMs ?? this.lastFinishedMs,
      lastError: lastError ?? this.lastError,
      lastLog: lastLog ?? this.lastLog,
    );
  }
}
