import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    as frb;
import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/platform/directory_access_store.dart';

import 'api.dart' as api;
import 'api/dlna/types.dart';
import 'api/player/queue.dart' as queue_api;
import 'api/player/transcode.dart' as transcode_api;
import 'api/player/types.dart';
import 'third_party/stellatune_backend_api/lyrics_types.dart';
import 'third_party/stellatune_library.dart';

export 'api/player/queue.dart' show PlaybackQueue, QueueEntry, QueueRepeatMode;
export 'frb_generated.dart' show StellatuneApi;
export 'api/player/types.dart'
    show
        Event,
        EventPatterns,
        AudioBackend,
        AudioDevice,
        SourceCatalogTypeDescriptor,
        LyricsProviderTypeDescriptor,
        EncoderTypeDescriptor,
        OutputSinkTypeDescriptor,
        OutputSinkRoute,
        PluginDescriptor,
        PlaybackSnapshot,
        PlayerState,
        TrackDecodeInfo,
        TranscodeProgressEvent,
        ResampleQuality;
export 'third_party/stellatune_library.dart'
    show LibraryEvent, LibraryEventPatterns, PlaylistLite, TrackLite;
export 'third_party/stellatune_backend_api/lyrics_types.dart'
    show
        LyricsQuery,
        LyricsEvent,
        LyricsEventPatterns,
        LyricsDoc,
        LyricLine,
        LyricsSearchCandidate;
export 'api/dlna/types.dart'
    show
        DlnaSsdpDevice,
        DlnaRenderer,
        DlnaHttpServerInfo,
        DlnaPositionInfo,
        DlnaTransportInfo;

/// Thin Dart-side facade over the generated FRB bindings.
///
/// Keeps UI code clean and hides generated `api.dart` / `third_party/*` details.
class PlayerBridge {
  PlayerBridge._();

  Stream<Event>? _eventBroadcast;
  Stream<LyricsEvent>? _lyricsEventBroadcast;
  DirectoryAccessStore? _directoryAccessStore;
  final Map<String, DirectoryAccessLease?> _queueLeases = {};

  static Future<PlayerBridge> create() async => PlayerBridge._();

  void bindDirectoryAccessStore(DirectoryAccessStore store) {
    _directoryAccessStore = store;
  }

  Future<void> dispose() async {
    for (final lease in _queueLeases.values) {
      await lease?.release();
    }
    _queueLeases.clear();
  }

  Stream<Event> events() =>
      _eventBroadcast ??= api.events().asBroadcastStream();

  Stream<LyricsEvent> lyricsEvents() =>
      _lyricsEventBroadcast ??= api.lyricsEvents().asBroadcastStream();

  Future<List<BigInt>> ensureLocalTracks(List<int> libraryTrackIds) async =>
      (await api.ensureLocalTracks(
        libraryTrackIds: frb.Int64List.fromList(libraryTrackIds),
      )).toList();

  Future<BigInt> ensureLocalTrack(int libraryTrackId) =>
      api.ensureLocalTrack(libraryTrackId: libraryTrackId);

  Future<BigInt> ensureProviderTrack({
    required String providerId,
    required String providerKey,
    required String pluginId,
    required String typeId,
    required String configJson,
  }) => api.ensureProviderTrack(
    providerId: providerId,
    providerKey: providerKey,
    pluginId: pluginId,
    typeId: typeId,
    configJson: configJson,
  );

  Future<queue_api.PlaybackQueue> playbackQueue() => queue_api.playbackQueue();

  Future<void> retainQueuePaths(Iterable<String> paths) async {
    for (final path in paths.where((path) => path.isNotEmpty).toSet()) {
      if (!_queueLeases.containsKey(path)) {
        _queueLeases[path] = await _acquireLocalPathLease(path);
      }
    }
  }

  Future<void> releaseRemovedQueuePaths(Iterable<String> paths) async {
    final retained = paths.toSet();
    for (final path in _queueLeases.keys.toList()) {
      if (!retained.contains(path)) {
        await _queueLeases.remove(path)?.release();
      }
    }
  }

  Future<queue_api.PlaybackQueue> replaceQueue(List<BigInt> ids) =>
      queue_api.replaceQueue(trackIds: frb.Uint64List.fromList(ids));
  Future<queue_api.PlaybackQueue> appendQueue(List<BigInt> ids) =>
      queue_api.appendQueue(trackIds: frb.Uint64List.fromList(ids));
  Future<queue_api.PlaybackQueue> removeQueueItems(List<BigInt> ids) =>
      queue_api.removeQueueItems(itemIds: frb.Uint64List.fromList(ids));
  Future<queue_api.PlaybackQueue> setQueueMode(
    queue_api.QueueRepeatMode repeat,
    bool shuffle,
  ) => queue_api.setQueueMode(repeat: repeat, shuffle: shuffle);
  Future<bool> selectQueueItem(BigInt itemId, {bool autoplay = true}) =>
      queue_api.selectQueueItem(itemId: itemId, autoplay: autoplay);
  Future<bool> nextQueueItem() => queue_api.nextQueueItem();
  Future<bool> previousQueueItem() => queue_api.previousQueueItem();

  Future<void> play() => api.play();
  Future<void> pause() => api.pause();
  Future<void> seekMs(int positionMs) =>
      api.seekMs(positionMs: BigInt.from(positionMs));
  Future<void> setVolume(
    double volume, {
    required int seq,
    required int rampMs,
  }) => api.setVolume(volume: volume, seq: BigInt.from(seq), rampMs: rampMs);
  Future<void> stop() => api.stop();

  Future<PlaybackSnapshot> playbackSnapshot() => api.playbackSnapshot();

  Future<void> lyricsPrepare(LyricsQuery query) =>
      api.lyricsPrepare(query: query);

  Future<void> lyricsPrefetch(LyricsQuery query) =>
      api.lyricsPrefetch(query: query);

  Future<List<LyricsSearchCandidate>> lyricsSearchCandidates(
    LyricsQuery query,
  ) => api.lyricsSearchCandidates(query: query);

  Future<void> lyricsApplyCandidate({
    required String trackKey,
    required LyricsDoc doc,
  }) => api.lyricsApplyCandidate(trackKey: trackKey, doc: doc);

  Future<void> lyricsSetCacheDbPath(String dbPath) =>
      api.lyricsSetCacheDbPath(dbPath: dbPath);

  Future<void> lyricsClearCache() => api.lyricsClearCache();

  Future<void> lyricsRefreshCurrent() => api.lyricsRefreshCurrent();

  Future<void> lyricsSetPositionMs(int positionMs) =>
      api.lyricsSetPositionMs(positionMs: BigInt.from(positionMs));

  Future<List<PluginDescriptor>> pluginsList() => api.pluginsList();

  Future<List<SourceCatalogTypeDescriptor>> sourceListTypes() =>
      api.sourceListTypes();

  Future<List<LyricsProviderTypeDescriptor>> lyricsProviderListTypes() =>
      api.lyricsProviderListTypes();

  Future<List<OutputSinkTypeDescriptor>> outputSinkListTypes() =>
      api.outputSinkListTypes();

  Future<List<EncoderTypeDescriptor>> encoderListTypes() =>
      api.encoderListTypes();

  Future<String> sourceListItemsJson({
    required String pluginId,
    required String typeId,
    required String configJson,
    required String requestJson,
  }) => api.sourceListItemsJson(
    pluginId: pluginId,
    typeId: typeId,
    configJson: configJson,
    requestJson: requestJson,
  );

  Future<String> lyricsProviderSearchJson({
    required String pluginId,
    required String typeId,
    required String queryJson,
  }) => api.lyricsProviderSearchJson(
    pluginId: pluginId,
    typeId: typeId,
    queryJson: queryJson,
  );

  Future<String> lyricsProviderFetchJson({
    required String pluginId,
    required String typeId,
    required String trackJson,
  }) => api.lyricsProviderFetchJson(
    pluginId: pluginId,
    typeId: typeId,
    trackJson: trackJson,
  );

  Future<String> outputSinkListTargetsJson({
    required String pluginId,
    required String typeId,
    required String configJson,
  }) => api.outputSinkListTargetsJson(
    pluginId: pluginId,
    typeId: typeId,
    configJson: configJson,
  );

  Future<void> setOutputSinkRoute(OutputSinkRoute route) =>
      api.setOutputSinkRoute(route: route);

  Future<void> clearOutputSinkRoute() => api.clearOutputSinkRoute();

  Future<TrackDecodeInfo?> currentTrackInfo() => api.currentTrackInfo();

  Future<String> pluginsInstallFromFile({
    required String dir,
    required String artifactPath,
  }) => api.pluginsInstallFromFile(pluginsDir: dir, artifactPath: artifactPath);

  Future<String> pluginsListInstalledJson({required String dir}) =>
      api.pluginsListInstalledJson(pluginsDir: dir);

  Future<void> pluginsUninstallById({
    required String dir,
    required String pluginId,
  }) => api.pluginsUninstallById(pluginsDir: dir, pluginId: pluginId);

  Future<List<AudioDevice>> refreshDevices() => api.refreshDevices();

  Future<void> setOutputDevice({
    required AudioBackend backend,
    String? deviceId,
  }) => api.setOutputDevice(backend: backend, deviceId: deviceId);

  Future<void> setOutputOptions({
    required bool matchTrackSampleRate,
    required bool gaplessPlayback,
    required bool seekTrackFade,
    required ResampleQuality resampleQuality,
  }) => api.setOutputOptions(
    matchTrackSampleRate: matchTrackSampleRate,
    gaplessPlayback: gaplessPlayback,
    seekTrackFade: seekTrackFade,
    resampleQuality: resampleQuality,
  );

  Stream<TranscodeProgressEvent> transcodeTrackLocal({
    required String taskId,
    required String sourcePath,
    required String outputPath,
    required String encoderPluginId,
    required String encoderTypeId,
    required String encoderConfigJson,
    String? encoderOptionsJson,
  }) => transcode_api.transcodeTrackLocal(
    request: transcode_api.TranscodeTrackLocalRequest(
      taskId: taskId,
      sourcePath: sourcePath,
      outputPath: outputPath,
      encoderPluginId: encoderPluginId,
      encoderTypeId: encoderTypeId,
      encoderConfigJson: encoderConfigJson,
      encoderOptionsJson: encoderOptionsJson,
    ),
  );

  Future<void> transcodeCancel({required String taskId}) =>
      transcode_api.transcodeCancel(taskId: taskId);

  Future<List<String>> decoderSupportedExtensions() =>
      api.decoderSupportedExtensions();

  Future<DirectoryAccessLease?> _acquireLocalPathLease(String path) {
    final store = _directoryAccessStore;
    if (store == null) {
      return Future.value(null);
    }
    return DirectoryAccessService.instance.acquireLocalPath(
      path: path,
      store: store,
    );
  }
}

class LibraryBridge {
  LibraryBridge._();
  Stream<LibraryEvent>? _eventBroadcast;

  static Future<LibraryBridge> create({required String dbPath}) async {
    await api.createLibrary(dbPath: dbPath);
    return LibraryBridge._();
  }

  Stream<LibraryEvent> events() =>
      _eventBroadcast ??= api.libraryEvents().asBroadcastStream();

  Future<void> addRoot(String path) => api.libraryAddRoot(path: path);

  Future<void> removeRoot(String path) => api.libraryRemoveRoot(path: path);

  Future<void> deleteFolder(String path) => api.libraryDeleteFolder(path: path);

  Future<void> restoreFolder(String path) =>
      api.libraryRestoreFolder(path: path);

  Future<List<String>> listExcludedFolders() =>
      api.libraryListExcludedFolders();

  Future<void> scanAll() => api.libraryScanAll();
  Future<void> scanAllForce() => api.libraryScanAllForce();

  Future<List<String>> listRoots() => api.libraryListRoots();

  Future<List<String>> listFolders() => api.libraryListFolders();

  Future<List<TrackLite>> listTracks({
    required String folder,
    required bool recursive,
    required String query,
    int limit = 5000,
    int offset = 0,
  }) => api.libraryListTracks(
    folder: folder,
    recursive: recursive,
    query: query,
    limit: limit,
    offset: offset,
  );

  Future<List<TrackLite>> search(
    String query, {
    int limit = 200,
    int offset = 0,
  }) => api.librarySearch(query: query, limit: limit, offset: offset);

  Future<List<PlaylistLite>> listPlaylists() => api.libraryListPlaylists();

  Future<void> createPlaylist(String name) =>
      api.libraryCreatePlaylist(name: name);

  Future<void> renamePlaylist({required int id, required String name}) =>
      api.libraryRenamePlaylist(id: id, name: name);

  Future<void> deletePlaylist({required int id}) =>
      api.libraryDeletePlaylist(id: id);

  Future<List<TrackLite>> listPlaylistTracks({
    required int playlistId,
    required String query,
    int limit = 5000,
    int offset = 0,
  }) => api.libraryListPlaylistTracks(
    playlistId: playlistId,
    query: query,
    limit: limit,
    offset: offset,
  );

  Future<void> addTrackToPlaylist({
    required int playlistId,
    required int trackId,
  }) => api.libraryAddTrackToPlaylist(playlistId: playlistId, trackId: trackId);

  Future<void> addTracksToPlaylist({
    required int playlistId,
    required List<int> trackIds,
  }) => api.libraryAddTracksToPlaylist(
    playlistId: playlistId,
    trackIds: frb.Int64List.fromList(trackIds),
  );

  Future<void> removeTrackFromPlaylist({
    required int playlistId,
    required int trackId,
  }) => api.libraryRemoveTrackFromPlaylist(
    playlistId: playlistId,
    trackId: trackId,
  );

  Future<void> removeTracksFromPlaylist({
    required int playlistId,
    required List<int> trackIds,
  }) => api.libraryRemoveTracksFromPlaylist(
    playlistId: playlistId,
    trackIds: frb.Int64List.fromList(trackIds),
  );

  Future<void> moveTrackInPlaylist({
    required int playlistId,
    required int trackId,
    required int newIndex,
  }) => api.libraryMoveTrackInPlaylist(
    playlistId: playlistId,
    trackId: trackId,
    newIndex: newIndex,
  );

  Future<List<int>> listLikedTrackIds() async =>
      (await api.libraryListLikedTrackIds()).map((v) => v.toInt()).toList();

  Future<void> setTrackLiked({required int trackId, required bool liked}) =>
      api.librarySetTrackLiked(trackId: trackId, liked: liked);

  Future<void> pluginDisable({required String pluginId}) =>
      api.libraryPluginDisable(pluginId: pluginId);

  Future<void> pluginEnable({required String pluginId}) =>
      api.libraryPluginEnable(pluginId: pluginId);

  Future<void> pluginApplyState() => api.libraryPluginApplyState();

  Future<String> pluginApplyStateStatusJson() =>
      api.libraryPluginApplyStateStatusJson();

  Future<List<String>> listDisabledPluginIds() =>
      api.libraryListDisabledPluginIds();
}

class DlnaBridge {
  const DlnaBridge();

  Future<List<DlnaSsdpDevice>> discoverMediaRenderers({
    Duration timeout = const Duration(milliseconds: 1200),
  }) => api.dlnaDiscoverMediaRenderers(timeoutMs: timeout.inMilliseconds);

  Future<List<DlnaRenderer>> discoverRenderers({
    Duration timeout = const Duration(milliseconds: 1200),
  }) => api.dlnaDiscoverRenderers(timeoutMs: timeout.inMilliseconds);

  Future<DlnaHttpServerInfo> httpStart({String? advertiseIp, int? port}) =>
      api.dlnaHttpStart(advertiseIp: advertiseIp, port: port);

  Future<String> httpPublishTrack({required String path}) =>
      api.dlnaHttpPublishTrack(path: path);

  Future<void> httpUnpublishAll() => api.dlnaHttpUnpublishAll();

  Future<void> avTransportSetUri({
    required String controlUrl,
    required String uri,
    String? metadata,
    String? serviceType,
  }) => api.dlnaAvTransportSetUri(
    controlUrl: controlUrl,
    serviceType: serviceType,
    uri: uri,
    metadata: metadata,
  );

  Future<void> avTransportPlay({
    required String controlUrl,
    String? serviceType,
  }) =>
      api.dlnaAvTransportPlay(controlUrl: controlUrl, serviceType: serviceType);

  Future<void> avTransportPause({
    required String controlUrl,
    String? serviceType,
  }) => api.dlnaAvTransportPause(
    controlUrl: controlUrl,
    serviceType: serviceType,
  );

  Future<void> avTransportStop({
    required String controlUrl,
    String? serviceType,
  }) =>
      api.dlnaAvTransportStop(controlUrl: controlUrl, serviceType: serviceType);

  Future<void> avTransportSeekMs({
    required String controlUrl,
    required int positionMs,
    String? serviceType,
  }) => api.dlnaAvTransportSeekMs(
    controlUrl: controlUrl,
    serviceType: serviceType,
    positionMs: BigInt.from(positionMs),
  );

  Future<DlnaTransportInfo> avTransportGetTransportInfo({
    required String controlUrl,
    String? serviceType,
  }) => api.dlnaAvTransportGetTransportInfo(
    controlUrl: controlUrl,
    serviceType: serviceType,
  );

  Future<DlnaPositionInfo> avTransportGetPositionInfo({
    required String controlUrl,
    String? serviceType,
  }) => api.dlnaAvTransportGetPositionInfo(
    controlUrl: controlUrl,
    serviceType: serviceType,
  );

  Future<void> renderingControlSetVolume({
    required String controlUrl,
    required int volume0To100,
    String? serviceType,
  }) => api.dlnaRenderingControlSetVolume(
    controlUrl: controlUrl,
    serviceType: serviceType,
    volume0100: volume0To100,
  );

  Future<void> renderingControlSetMute({
    required String controlUrl,
    required bool mute,
    String? serviceType,
  }) => api.dlnaRenderingControlSetMute(
    controlUrl: controlUrl,
    serviceType: serviceType,
    mute: mute,
  );

  Future<int> renderingControlGetVolume({
    required String controlUrl,
    String? serviceType,
  }) async {
    final v = await api.dlnaRenderingControlGetVolume(
      controlUrl: controlUrl,
      serviceType: serviceType,
    );
    return v.toInt();
  }

  Future<String> playLocalPath({
    required DlnaRenderer renderer,
    required String path,
  }) => api.dlnaPlayLocalPath(renderer: renderer, path: path);

  Future<String> playLocalTrack({
    required DlnaRenderer renderer,
    required String path,
    String? title,
    String? artist,
    String? album,
    String? coverPath,
  }) => api.dlnaPlayLocalTrack(
    renderer: renderer,
    path: path,
    title: title,
    artist: artist,
    album: album,
    coverPath: coverPath,
  );
}
