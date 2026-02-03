import 'package:flutter/foundation.dart';
import 'package:stellatune/bridge/bridge.dart';

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
    required this.selectedFolder,
    required this.includeSubfolders,
    required this.query,
    required this.results,
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
      selectedFolder = '',
      includeSubfolders = false,
      query = '',
      results = const [],
      isScanning = false,
      progress = const LibraryScanProgress.zero(),
      lastFinishedMs = null,
      lastError = null,
      lastLog = '';

  final List<String> roots;
  final List<String> folders;
  final List<String> excludedFolders;

  /// Normalized folder path. Empty string means "All music".
  final String selectedFolder;
  final bool includeSubfolders;
  final String query;
  final List<TrackLite> results;
  final bool isScanning;
  final LibraryScanProgress progress;
  final int? lastFinishedMs;
  final String? lastError;
  final String lastLog;

  LibraryState copyWith({
    List<String>? roots,
    List<String>? folders,
    List<String>? excludedFolders,
    String? selectedFolder,
    bool? includeSubfolders,
    String? query,
    List<TrackLite>? results,
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
      selectedFolder: selectedFolder ?? this.selectedFolder,
      includeSubfolders: includeSubfolders ?? this.includeSubfolders,
      query: query ?? this.query,
      results: results ?? this.results,
      isScanning: isScanning ?? this.isScanning,
      progress: progress ?? this.progress,
      lastFinishedMs: lastFinishedMs ?? this.lastFinishedMs,
      lastError: lastError ?? this.lastError,
      lastLog: lastLog ?? this.lastLog,
    );
  }
}
