import 'api.dart' as api;
import 'third_party/stellatune_core.dart';

export 'frb_generated.dart' show StellatuneApi;
export 'third_party/stellatune_core.dart'
    show Event, PlayerState, LibraryEvent, TrackLite;

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
  Future<void> stop() => api.stop(player: player);
}

class LibraryBridge {
  LibraryBridge._(this.library);

  final api.Library library;

  static Future<LibraryBridge> create({required String dbPath}) async {
    final library = await api.createLibrary(dbPath: dbPath);
    return LibraryBridge._(library);
  }

  Stream<LibraryEvent> events() => api.libraryEvents(library_: library);

  Future<void> addRoot(String path) =>
      api.libraryAddRoot(library_: library, path: path);

  Future<void> removeRoot(String path) =>
      api.libraryRemoveRoot(library_: library, path: path);

  Future<void> scanAll() => api.libraryScanAll(library_: library);

  Future<void> listRoots() => api.libraryListRoots(library_: library);

  Future<void> search(String query, {int limit = 200, int offset = 0}) =>
      api.librarySearch(
        library_: library,
        query: query,
        limit: limit,
        offset: offset,
      );
}
