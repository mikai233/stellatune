import 'api.dart' as api;
import 'third_party/stellatune_core.dart';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show PlatformInt64, PlatformInt64Util;

export 'frb_generated.dart' show StellatuneApi;
export 'third_party/stellatune_core.dart' show Command, Event, PlayerState;

/// Thin Dart-side facade over the generated FRB bindings.
///
/// Keeps UI code clean and hides generated `api.dart` / `third_party/*` details.
class CoreBridge {
  CoreBridge._(this.service);

  final api.CoreService service;

  static Future<CoreBridge> create() async {
    final service = await api.createCoreService();
    return CoreBridge._(service);
  }

  Stream<Event> events() => api.eventsStream(service: service);

  Future<void> send(Command cmd) => api.sendCommand(service: service, cmd: cmd);

  Future<void> play() => send(const Command.play());
  Future<void> pause() => send(const Command.pause());
  Future<void> stop() => send(const Command.stop());

  Future<void> seek(int ms) =>
      send(Command.seek(ms: PlatformInt64Util.from(ms)));

  Future<void> next() => send(const Command.next());
  Future<void> previous() => send(const Command.previous());

  static String formatPlatformInt64(PlatformInt64 value) => value.toString();
}
