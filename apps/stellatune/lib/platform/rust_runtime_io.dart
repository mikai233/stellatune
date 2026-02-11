import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:stellatune/bridge/api/runtime.dart' as runtime_api;
import 'package:stellatune/bridge/frb_generated.dart';

String _libFileName() {
  const stem = 'stellatune_ffi';
  if (Platform.isWindows) return '$stem.dll';
  if (Platform.isMacOS) return 'lib$stem.dylib';
  if (Platform.isLinux) return 'lib$stem.so';
  if (Platform.isAndroid) return 'lib$stem.so';
  if (Platform.isIOS) return 'lib$stem.a';
  throw UnsupportedError('Unsupported platform');
}

ExternalLibrary _openRustLibrary() {
  final name = _libFileName();

  // Android ships the .so as a JNI library; open by name.
  if (Platform.isAndroid) {
    return ExternalLibrary.open(name);
  }

  // On desktop and iOS, the runner links/embeds the Rust library, so it is
  // already loaded into the current process.
  return ExternalLibrary.process(iKnowHowToUseIt: true);
}

Future<void> initRustRuntime() async {
  await StellatuneApi.init(externalLibrary: _openRustLibrary());
  await runtime_api.prepareHotRestart();
}
