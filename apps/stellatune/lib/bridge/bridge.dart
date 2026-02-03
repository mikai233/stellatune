import 'api.dart' as api;
import 'third_party/stellatune_core.dart';

export 'frb_generated.dart' show StellatuneApi;
export 'third_party/stellatune_core.dart' show Event, PlayerState;

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
