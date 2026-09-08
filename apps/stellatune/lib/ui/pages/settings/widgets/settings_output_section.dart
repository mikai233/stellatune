import 'dart:io';

import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/bridge/api/error.dart';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/output_settings_controller.dart';
import 'package:stellatune/app/output_settings_values.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_form_row.dart';
import 'package:stellatune/ui/pages/settings/widgets/settings_section_card.dart';

class SettingsOutputSection extends ConsumerWidget {
  const SettingsOutputSection({super.key, this.includePlayback = false});
  final bool includePlayback;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final state = ref.watch(outputSettingsControllerProvider);
    final confirmed = ref.watch(settingsStoreProvider);
    final controller = ref.read(outputSettingsControllerProvider.notifier);
    final selection =
        state.draft ?? OutputSettingsDraft.fromSettings(confirmed);
    final local = selection.localBackend;
    final backends = <DropdownMenuItem<String>>[
      for (final backend in OutputSettingsValues.availableLocalBackends())
        DropdownMenuItem(
          value: OutputSettingsValues.localBackendKey(backend),
          child: Text(switch (backend) {
            AudioBackend.shared =>
              Platform.isWindows
                  ? l10n.settingsBackendShared
                  : l10n.settingsBackendSharedGeneric,
            AudioBackend.wasapiExclusive => l10n.settingsBackendWasapiExclusive,
          }),
        ),
      for (final type in state.types)
        DropdownMenuItem(
          value: OutputSettingsValues.pluginBackendKey(
            type.pluginId,
            type.typeId,
          ),
          child: Text('Plugin: ${type.displayName} (${type.pluginName})'),
        ),
    ];
    if (!backends.any((item) => item.value == selection.backendKey)) {
      backends.add(
        DropdownMenuItem(
          value: selection.backendKey,
          child: Text(selection.backendKey),
        ),
      );
    }
    final deviceValue = local == null
        ? selection.targetJson
        : selection.deviceId;
    DropdownMenuItem<String?> deviceItem(String value, String label) {
      final error = state.unavailableTargets[(selection.backendKey, value)];
      return DropdownMenuItem(
        value: value,
        enabled: error == null,
        child: Tooltip(
          message: error == null
              ? label
              : DiagnosticsService.instance.messageFor(
                  error,
                  operation: 'output',
                ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Flexible(
                child: Text(
                  label,
                  overflow: TextOverflow.ellipsis,
                  style: error == null
                      ? null
                      : TextStyle(color: Theme.of(context).disabledColor),
                ),
              ),
              if (error != null)
                IconButton(
                  tooltip: DiagnosticsService.instance.chinese
                      ? '查看日志'
                      : 'View logs',
                  icon: const Icon(Icons.receipt_long_outlined, size: 16),
                  onPressed: () => DiagnosticsService.instance.open(
                    error is AppError ? error.diagnosticId : null,
                  ),
                ),
            ],
          ),
        ),
      );
    }

    final devices = <DropdownMenuItem<String?>>[
      if (local != null)
        DropdownMenuItem(value: null, child: Text(l10n.settingsDeviceDefault)),
      if (local != null)
        for (final device in state.devices.where((d) => d.backend == local))
          deviceItem(device.id, device.name),
      if (local == null)
        for (final target in state.targets)
          deviceItem(
            OutputSettingsValues.targetValueOf(target),
            OutputSettingsValues.targetLabelOf(target),
          ),
    ];
    if (deviceValue != null &&
        !devices.any((item) => item.value == deviceValue)) {
      devices.add(
        DropdownMenuItem(
          value: deviceValue,
          child: Text(
            local == null
                ? OutputSettingsValues.targetLabelOf(deviceValue)
                : deviceValue,
          ),
        ),
      );
    }
    return SettingsSectionCard(
      title: l10n.settingsOutputTitle,
      icon: Icons.graphic_eq,
      subtitle: '配置音频输出与音质相关选项',
      headerBottomSpacing: 8,
      trailing: IconButton(
        tooltip: l10n.refresh,
        onPressed: state.applying || state.loading ? null : controller.refresh,
        icon: const Icon(Icons.refresh),
      ),
      children: [
        if (state.loading || state.loadingTargets)
          const LinearProgressIndicator(),
        SettingsSelectField<String>(
          key: ValueKey(('backend', state.revision)),
          decoration: InputDecoration(labelText: l10n.settingsBackend),
          initialValue: selection.backendKey,
          items: backends,
          onChanged: state.applying
              ? null
              : (value) {
                  if (value != null) controller.selectBackend(value);
                },
        ),
        const SizedBox(height: 12),
        SettingsSelectField<String?>(
          key: ValueKey(('device', state.revision)),
          decoration: InputDecoration(labelText: l10n.settingsDevice),
          initialValue: deviceValue,
          items: devices,
          onChanged: state.applying || state.loadingTargets
              ? null
              : controller.selectDevice,
        ),
        const SizedBox(height: 12),
        if (local == AudioBackend.wasapiExclusive || local == null)
          SettingsToggleRow(
            title: Text(l10n.settingsMatchTrackSampleRate),
            value: confirmed.matchTrackSampleRate,
            onChanged: state.applying
                ? null
                : (value) => controller.setOptions(matchTrackSampleRate: value),
          ),
        if (local == AudioBackend.wasapiExclusive) ...[
          SettingsToggleRow(
            title: Text(l10n.settingsGaplessPlayback),
            value: confirmed.gaplessPlayback,
            onChanged: state.applying
                ? null
                : (value) => controller.setOptions(gaplessPlayback: value),
          ),
        ],
        if (includePlayback) const SettingsPlaybackControls(),
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: SettingsSelectField<ResampleQuality>(
            key: ValueKey(('quality', state.revision)),
            decoration: InputDecoration(
              labelText: l10n.settingsResampleQuality,
            ),
            initialValue: confirmed.resampleQuality,
            items: [
              DropdownMenuItem(
                value: ResampleQuality.fast,
                child: Text(l10n.settingsResampleQualityFast),
              ),
              DropdownMenuItem(
                value: ResampleQuality.balanced,
                child: Text(l10n.settingsResampleQualityBalanced),
              ),
              DropdownMenuItem(
                value: ResampleQuality.high,
                child: Text(l10n.settingsResampleQualityHigh),
              ),
              DropdownMenuItem(
                value: ResampleQuality.ultra,
                child: Text(l10n.settingsResampleQualityUltra),
              ),
            ],
            onChanged: state.applying
                ? null
                : (value) {
                    if (value != null) {
                      controller.setOptions(resampleQuality: value);
                    }
                  },
          ),
        ),
      ],
    );
  }
}

class SettingsPlaybackSection extends StatelessWidget {
  const SettingsPlaybackSection({super.key});
  @override
  Widget build(BuildContext context) => const SettingsSectionCard(
    title: '播放',
    icon: Icons.play_circle_outline,
    subtitle: '调整播放行为与体验细节',
    children: [SettingsPlaybackControls()],
  );
}

class SettingsPlaybackControls extends ConsumerWidget {
  const SettingsPlaybackControls({super.key});
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final settings = ref.watch(settingsStoreProvider);
    final output = ref.watch(outputSettingsControllerProvider);
    final controller = ref.read(outputSettingsControllerProvider.notifier);
    return Column(
      children: [
        SettingsToggleRow(
          title: Text(l10n.settingsSeekTrackFade),
          subtitle: const Text('让跳转与切歌时的声音过渡更自然'),
          value: settings.seekTrackFade,
          onChanged: output.applying
              ? null
              : (value) => controller.setOptions(seekTrackFade: value),
        ),
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: SettingsSelectField<PlaybackLatency>(
            key: ValueKey(('latency', output.revision)),
            decoration: InputDecoration(
              labelText: l10n.settingsPlaybackLatency,
              helperText: l10n.settingsPlaybackLatencyHint,
            ),
            initialValue: settings.playbackLatency,
            items: [
              DropdownMenuItem(
                value: PlaybackLatency.low,
                child: Text(l10n.settingsPlaybackLatencyLow),
              ),
              DropdownMenuItem(
                value: PlaybackLatency.medium,
                child: Text(l10n.settingsPlaybackLatencyMedium),
              ),
              DropdownMenuItem(
                value: PlaybackLatency.high,
                child: Text(l10n.settingsPlaybackLatencyHigh),
              ),
            ],
            onChanged: output.applying
                ? null
                : (value) {
                    if (value != null) controller.setLatency(value);
                  },
          ),
        ),
      ],
    );
  }
}
