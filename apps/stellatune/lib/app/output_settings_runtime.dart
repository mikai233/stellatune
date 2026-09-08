import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/output_settings_values.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/bridge/bridge.dart';

/// Shared by bootstrap and settings: the backend acknowledges before we save.
Future<void> applyOutputSelection(
  PlayerBridge bridge, {
  required AudioBackend backend,
  required String? deviceId,
  required OutputSinkRoute? route,
}) => route == null
    // setOutputDevice switches away from native output transactionally in Rust.
    ? bridge.setOutputDevice(backend: backend, deviceId: deviceId)
    : bridge.setOutputSinkRoute(route);

Future<void> applyOutputOptions(PlayerBridge bridge, SettingsState settings) =>
    bridge.setOutputOptions(
      matchTrackSampleRate: settings.matchTrackSampleRate,
      gaplessPlayback: settings.gaplessPlayback,
      seekTrackFade: settings.seekTrackFade,
      resampleQuality: settings.resampleQuality,
    );

Future<void> restorePersistedOutputSettings({
  required PlayerBridge bridge,
  required SettingsStore settings,
}) async {
  final persisted = settings.readState();
  await bridge.setPlaybackLatency(persisted.playbackLatency);
  var deviceId = persisted.selectedDeviceId;
  try {
    await applyOutputSelection(
      bridge,
      backend: persisted.selectedBackend,
      deviceId: deviceId,
      route: null,
    );
  } catch (error, stack) {
    logger.w(
      'persisted output unavailable; using default device',
      error: error,
      stackTrace: stack,
    );
    deviceId = null;
    await applyOutputSelection(
      bridge,
      backend: persisted.selectedBackend,
      deviceId: deviceId,
      route: null,
    );
  }
  await applyOutputOptions(bridge, persisted);
  OutputSinkRoute? restoredRoute;
  final route = persisted.outputSinkRoute;
  if (route != null) {
    try {
      final types = await bridge.outputSinkListTypes();
      if (types.any(
        (t) => t.pluginId == route.pluginId && t.typeId == route.typeId,
      )) {
        final targets = OutputSettingsValues.parseOutputSinkTargetsJson(
          await bridge.outputSinkListTargetsJson(
            pluginId: route.pluginId,
            typeId: route.typeId,
            configJson: route.configJson,
          ),
        );
        final target = targets
            .where(
              (target) => OutputSettingsValues.jsonTextsEquivalent(
                OutputSettingsValues.targetValueOf(target),
                route.targetJson,
              ),
            )
            .firstOrNull;
        final resolved = target ?? targets.firstOrNull;
        restoredRoute = OutputSinkRoute(
          pluginId: route.pluginId,
          typeId: route.typeId,
          configJson: route.configJson,
          targetJson: resolved == null
              ? route.targetJson
              : OutputSettingsValues.targetValueOf(resolved),
        );
        await applyOutputSelection(
          bridge,
          backend: persisted.selectedBackend,
          deviceId: deviceId,
          route: restoredRoute,
        );
      }
    } catch (error, stack) {
      logger.w(
        'persisted plugin output unavailable; using local output',
        error: error,
        stackTrace: stack,
      );
      restoredRoute = null;
      await applyOutputSelection(
        bridge,
        backend: persisted.selectedBackend,
        deviceId: deviceId,
        route: null,
      );
    }
  }
  await settings.saveOutputSelection(
    backend: persisted.selectedBackend,
    deviceId: deviceId,
    route: restoredRoute,
  );
}
