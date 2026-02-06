import 'api.dart' as api;
import 'third_party/stellatune_core.dart';

export 'frb_generated.dart' show StellatuneApi;
export 'third_party/stellatune_core.dart'
    show
        Event,
        AudioBackend,
        AudioDevice,
        DspChainItem,
        DspTypeDescriptor,
        PluginDescriptor,
        PlayerState,
        LibraryEvent,
        TrackDecodeInfo,
        TrackLite,
        DlnaSsdpDevice,
        DlnaRenderer,
        DlnaHttpServerInfo;

/// Thin Dart-side facade over the generated FRB bindings.
///
/// Keeps UI code clean and hides generated `api.dart` / `third_party/*` details.
class PlayerBridge {
  PlayerBridge._(this.player);

  final api.Player player;

  static Future<PlayerBridge> create() async {
    final player = await api.createPlayer();
    return PlayerBridge._(player);
  }

  Stream<Event> events() => api.events(player: player);

  Future<void> load(String path) => api.load(player: player, path: path);

  Future<void> play() => api.play(player: player);
  Future<void> pause() => api.pause(player: player);
  Future<void> seekMs(int positionMs) =>
      api.seekMs(player: player, positionMs: BigInt.from(positionMs));
  Future<void> setVolume(double volume) =>
      api.setVolume(player: player, volume: volume);
  Future<void> stop() => api.stop(player: player);

  Future<List<PluginDescriptor>> pluginsList() =>
      api.pluginsList(player: player);

  Future<List<DspTypeDescriptor>> dspListTypes() =>
      api.dspListTypes(player: player);

  Future<void> dspSetChain(List<DspChainItem> chain) =>
      api.dspSetChain(player: player, chain: chain);

  Future<TrackDecodeInfo?> currentTrackInfo() =>
      api.currentTrackInfo(player: player);

  Future<void> pluginsReload(String dir) =>
      api.pluginsReload(player: player, dir: dir);

  Future<void> pluginsReloadWithDisabled({
    required String dir,
    required List<String> disabledIds,
  }) => api.pluginsReloadWithDisabled(
    player: player,
    dir: dir,
    disabledIds: disabledIds,
  );

  Future<void> refreshDevices() => api.refreshDevices(player: player);

  Future<void> setOutputDevice({
    required AudioBackend backend,
    String? deviceId,
  }) =>
      api.setOutputDevice(player: player, backend: backend, deviceId: deviceId);

  Future<void> setOutputOptions({required bool matchTrackSampleRate}) =>
      api.setOutputOptions(
        player: player,
        matchTrackSampleRate: matchTrackSampleRate,
      );
}

class LibraryBridge {
  LibraryBridge._(this.library);

  final api.Library library;

  static Future<LibraryBridge> create({
    required String dbPath,
    List<String> disabledPluginIds = const [],
  }) async {
    final library = await api.createLibrary(
      dbPath: dbPath,
      disabledPluginIds: disabledPluginIds,
    );
    return LibraryBridge._(library);
  }

  Stream<LibraryEvent> events() => api.libraryEvents(library_: library);

  Future<void> addRoot(String path) =>
      api.libraryAddRoot(library_: library, path: path);

  Future<void> removeRoot(String path) =>
      api.libraryRemoveRoot(library_: library, path: path);

  Future<void> deleteFolder(String path) =>
      api.libraryDeleteFolder(library_: library, path: path);

  Future<void> restoreFolder(String path) =>
      api.libraryRestoreFolder(library_: library, path: path);

  Future<void> listExcludedFolders() =>
      api.libraryListExcludedFolders(library_: library);

  Future<void> scanAll() => api.libraryScanAll(library_: library);
  Future<void> scanAllForce() => api.libraryScanAllForce(library_: library);

  Future<void> listRoots() => api.libraryListRoots(library_: library);

  Future<void> listFolders() => api.libraryListFolders(library_: library);

  Future<void> listTracks({
    required String folder,
    required bool recursive,
    required String query,
    int limit = 5000,
    int offset = 0,
  }) => api.libraryListTracks(
    library_: library,
    folder: folder,
    recursive: recursive,
    query: query,
    limit: limit,
    offset: offset,
  );

  Future<void> search(String query, {int limit = 200, int offset = 0}) =>
      api.librarySearch(
        library_: library,
        query: query,
        limit: limit,
        offset: offset,
      );

  Future<void> pluginsReloadWithDisabled({
    required String dir,
    required List<String> disabledIds,
  }) => api.libraryPluginsReloadWithDisabled(
    library_: library,
    dir: dir,
    disabledIds: disabledIds,
  );
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
