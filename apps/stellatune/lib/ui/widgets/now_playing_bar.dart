import 'dart:async';

import 'package:animations/animations.dart';
import 'package:flutter/material.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/playability_messages.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/music_detail_page.dart';
import 'package:stellatune/ui/widgets/audio_format_badge.dart';
import 'package:stellatune/ui/widgets/marquee_text.dart';
import 'package:stellatune/ui/widgets/now_playing_common.dart';

class NowPlayingBar extends ConsumerWidget {
  const NowPlayingBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final playback = ref.watch(playbackControllerProvider);
    final queue = ref.watch(queueControllerProvider);
    final selectedRenderer = ref.watch(dlnaSelectedRendererProvider);

    final currentTitle = queue.currentItem?.displayTitle ?? l10n.nowPlayingNone;
    final String currentSubtitle;
    if (queue.currentItem != null) {
      final artist = (queue.currentItem?.artist ?? '').trim();
      final album = (queue.currentItem?.album ?? '').trim();
      currentSubtitle = [artist, album].where((s) => s.isNotEmpty).join(' • ');
    } else {
      currentSubtitle = playback.currentPath ?? '';
    }
    final playModeLabel = switch (queue.playMode) {
      PlayMode.sequential => l10n.playModeSequential,
      PlayMode.shuffle => l10n.playModeShuffle,
      PlayMode.repeatAll => l10n.playModeRepeatAll,
      PlayMode.repeatOne => l10n.playModeRepeatOne,
    };

    final isPlaying =
        playback.playerState == PlayerState.playing ||
        playback.playerState == PlayerState.buffering;
    final localizedPlaybackError = playback.lastError == null
        ? null
        : localizePlaybackError(l10n, playback.lastError!);
    final totalDurationMs =
        queue.currentItem?.durationMs ??
        playback.trackInfo?.durationMs?.toInt();
    final timeLabel = (totalDurationMs != null && totalDurationMs > 0)
        ? '${NowPlayingCommon.formatMs(playback.positionMs)} / ${NowPlayingCommon.formatMs(totalDurationMs)}'
        : NowPlayingCommon.formatMs(playback.positionMs);

    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [
            theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.9),
            theme.colorScheme.surfaceContainer.withValues(alpha: 0.94),
          ],
        ),
        border: Border(
          top: BorderSide(
            color: theme.colorScheme.onSurface.withValues(alpha: 0.08),
          ),
        ),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.08),
            blurRadius: 12,
            offset: const Offset(0, -2),
          ),
        ],
      ),
      child: SizedBox(
        height: 76,
        child: LayoutBuilder(
          builder: (context, constraints) {
            return Stack(
              children: [
                Row(
                  children: [
                    const SizedBox(width: 12),
                    OpenContainer(
                      closedElevation: 0,
                      openElevation: 0,
                      closedColor: Colors.black,
                      openColor: Colors.black,
                      closedShape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(8),
                      ),
                      openShape: const RoundedRectangleBorder(
                        borderRadius: BorderRadius.zero,
                      ),
                      transitionDuration: const Duration(milliseconds: 400),
                      transitionType: ContainerTransitionType.fade,
                      openBuilder: (context, close) => const MusicDetailPage(),
                      closedBuilder: (context, open) => Consumer(
                        builder: (context, ref, child) {
                          final innerQueue = ref.watch(queueControllerProvider);
                          final innerCoverDir = ref.watch(coverDirProvider);
                          final innerTrackId = innerQueue.currentItem?.id;
                          final cover = innerQueue.currentItem?.cover;
                          return NowPlayingCover(
                            coverDir: innerCoverDir,
                            trackId: innerTrackId,
                            cover: cover,
                            primaryColor: theme.colorScheme.primary,
                            onTap: innerQueue.currentItem != null ? open : null,
                          );
                        },
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          MarqueeText(
                            text: currentTitle,
                            style: theme.textTheme.bodyMedium,
                          ),
                          if (currentSubtitle.isNotEmpty)
                            Row(
                              children: [
                                if (playback.currentPath != null) ...[
                                  AudioFormatBadge(
                                    path: playback.currentPath!,
                                    sampleRate: playback.trackInfo?.sampleRate,
                                  ),
                                  const SizedBox(width: 4),
                                ],
                                Expanded(
                                  child: MarqueeText(
                                    text: currentSubtitle,
                                    style: theme.textTheme.bodySmall,
                                  ),
                                ),
                              ],
                            ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 14),
                    Text(timeLabel, style: theme.textTheme.titleMedium),
                    if (localizedPlaybackError != null) ...[
                      const SizedBox(width: 8),
                      IconButton(
                        tooltip: localizedPlaybackError,
                        onPressed: () {
                          ScaffoldMessenger.of(context).showSnackBar(
                            SnackBar(content: Text(localizedPlaybackError)),
                          );
                        },
                        icon: Icon(
                          Icons.error_outline,
                          color: theme.colorScheme.error,
                        ),
                      ),
                    ],
                    const SizedBox(width: 14),
                    Container(
                      height: 40,
                      decoration: BoxDecoration(
                        color: theme.colorScheme.surface.withValues(
                          alpha: 0.54,
                        ),
                        borderRadius: BorderRadius.circular(14),
                        border: Border.all(
                          color: theme.colorScheme.onSurface.withValues(
                            alpha: 0.08,
                          ),
                        ),
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            tooltip: l10n.tooltipPrevious,
                            onPressed: () => ref
                                .read(playbackControllerProvider.notifier)
                                .previous(),
                            icon: const Icon(Icons.skip_previous),
                          ),
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            tooltip: isPlaying ? l10n.pause : l10n.play,
                            onPressed: () => isPlaying
                                ? ref
                                      .read(playbackControllerProvider.notifier)
                                      .pause()
                                : ref
                                      .read(playbackControllerProvider.notifier)
                                      .play(),
                            icon: Icon(
                              isPlaying ? Icons.pause : Icons.play_arrow,
                            ),
                          ),
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            tooltip: l10n.stop,
                            onPressed: () => ref
                                .read(playbackControllerProvider.notifier)
                                .stop(),
                            icon: const Icon(Icons.stop),
                          ),
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            tooltip: l10n.tooltipNext,
                            onPressed: () => ref
                                .read(playbackControllerProvider.notifier)
                                .next(),
                            icon: const Icon(Icons.skip_next),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 10),
                    Container(
                      height: 40,
                      padding: const EdgeInsets.symmetric(horizontal: 2),
                      decoration: BoxDecoration(
                        color: theme.colorScheme.surface.withValues(
                          alpha: 0.54,
                        ),
                        borderRadius: BorderRadius.circular(14),
                        border: Border.all(
                          color: theme.colorScheme.onSurface.withValues(
                            alpha: 0.08,
                          ),
                        ),
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          VolumePopupButton(
                            volume: playback.desiredVolume,
                            enableHover: true, // Desktop behavior
                            onChanged: (v) => ref
                                .read(playbackControllerProvider.notifier)
                                .setVolume(v),
                            onToggleMute: () => ref
                                .read(playbackControllerProvider.notifier)
                                .toggleMute(),
                          ),
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            tooltip: playModeLabel,
                            onPressed: () => ref
                                .read(queueControllerProvider.notifier)
                                .cyclePlayMode(),
                            icon: Icon(
                              switch (queue.playMode) {
                                PlayMode.sequential => Icons.playlist_play,
                                PlayMode.shuffle => Icons.shuffle,
                                PlayMode.repeatAll => Icons.repeat,
                                PlayMode.repeatOne => Icons.repeat_one,
                              },
                              color: queue.playMode == PlayMode.sequential
                                  ? null
                                  : theme.colorScheme.primary,
                            ),
                          ),
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            tooltip: selectedRenderer == null
                                ? 'DLNA'
                                : 'DLNA: ${selectedRenderer.friendlyName}',
                            onPressed: () async {
                              final chosen =
                                  await showDialog<_DlnaActionResult>(
                                    context: context,
                                    builder: (context) =>
                                        _DlnaDialog(selected: selectedRenderer),
                                  );
                              if (chosen == null) return;

                              if (chosen.applySelection) {
                                ref
                                    .read(dlnaSelectedRendererProvider.notifier)
                                    .set(chosen.selected);
                              }

                              final message = chosen.message;
                              if (message != null && context.mounted) {
                                ScaffoldMessenger.of(context).showSnackBar(
                                  SnackBar(content: Text(message)),
                                );
                              }
                            },
                            icon: Icon(
                              Icons.cast,
                              color: selectedRenderer == null
                                  ? null
                                  : theme.colorScheme.primary,
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 12),
                  ],
                ),
                Positioned(
                  left: 0,
                  right: 0,
                  top: 0,
                  child: NowPlayingProgressBar(
                    durationMs: totalDurationMs,
                    positionMs: playback.positionMs,
                    enabled:
                        queue.currentItem != null &&
                        playback.currentPath != null &&
                        playback.currentPath!.isNotEmpty,
                    audioStarted: playback.audioStarted,
                    playerState: playback.playerState,
                    onSeekMs: (ms) => ref
                        .read(playbackControllerProvider.notifier)
                        .seekMs(ms),
                  ),
                ),
              ],
            );
          },
        ),
      ),
    );
  }
}

class _DlnaActionResult {
  const _DlnaActionResult({
    this.applySelection = false,
    this.selected,
    this.message,
  });

  final bool applySelection;
  final DlnaRenderer? selected;
  final String? message;
}

class _DlnaDialog extends StatefulWidget {
  const _DlnaDialog({required this.selected});

  final DlnaRenderer? selected;

  @override
  State<_DlnaDialog> createState() => _DlnaDialogState();
}

class _DlnaDialogState extends State<_DlnaDialog> {
  late Future<List<DlnaRenderer>> _future;
  DlnaRenderer? _selected;

  @override
  void initState() {
    super.initState();
    _selected = widget.selected;
    _future = const DlnaBridge().discoverRenderers(
      timeout: const Duration(milliseconds: 1200),
    );
  }

  void _refresh() {
    setState(() {
      _future = const DlnaBridge().discoverRenderers(
        timeout: const Duration(milliseconds: 1200),
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);

    final okLabel = MaterialLocalizations.of(context).okButtonLabel;
    final cancelLabel = MaterialLocalizations.of(context).cancelButtonLabel;
    final screenH = MediaQuery.sizeOf(context).height;
    final listHeight = (screenH * 0.45).clamp(260.0, 420.0);

    return Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(24, 20, 24, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Row(
                children: [
                  Icon(Icons.cast, color: theme.colorScheme.primary),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(l10n.dlna, style: theme.textTheme.titleLarge),
                  ),
                  IconButton(
                    tooltip: l10n.refresh,
                    onPressed: _refresh,
                    icon: const Icon(Icons.refresh),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  l10n.settingsOutputTitle,
                  style: theme.textTheme.labelMedium?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
              const SizedBox(height: 12),
              SizedBox(
                height: listHeight,
                child: FutureBuilder<List<DlnaRenderer>>(
                  future: _future,
                  builder: (context, snapshot) {
                    final data = snapshot.data;
                    if (snapshot.connectionState != ConnectionState.done) {
                      return const Center(child: CircularProgressIndicator());
                    }
                    if (snapshot.hasError) {
                      return _DlnaEmptyState(
                        icon: Icons.error_outline,
                        title: l10n.dlnaSearchFailed(snapshot.error.toString()),
                        subtitle: '${snapshot.error}',
                        onRetry: _refresh,
                      );
                    }

                    final devices = data ?? const [];
                    if (devices.isEmpty) {
                      return _DlnaEmptyState(
                        icon: Icons.wifi_off,
                        title: l10n.dlnaNoDevices,
                        subtitle: l10n.dlnaNoDevicesSubtitle,
                        onRetry: _refresh,
                      );
                    }

                    return Container(
                      decoration: BoxDecoration(
                        color: theme.colorScheme.surfaceContainerLow,
                        borderRadius: BorderRadius.circular(16),
                        border: Border.all(
                          color: theme.colorScheme.outlineVariant,
                        ),
                      ),
                      clipBehavior: Clip.antiAlias,
                      child: ListView.separated(
                        shrinkWrap: true,
                        itemCount: devices.length + 1,
                        separatorBuilder: (context, index) =>
                            const Divider(height: 1),
                        itemBuilder: (context, i) {
                          if (i == 0) {
                            final selected = _selected == null;
                            return ListTile(
                              dense: true,
                              leading: const Icon(Icons.computer),
                              title: Text(l10n.deviceLocal),
                              subtitle: Text(l10n.deviceLocalSubtitle),
                              trailing: selected
                                  ? Icon(
                                      Icons.check_circle,
                                      color: theme.colorScheme.primary,
                                    )
                                  : null,
                              selected: selected,
                              onTap: () => setState(() => _selected = null),
                            );
                          }

                          final d = devices[i - 1];
                          final ok = d.avTransportControlUrl != null;
                          final selected = _selected?.usn == d.usn;
                          final volOk = d.renderingControlUrl != null;
                          final subtitle = ok
                              ? (volOk ? null : l10n.dlnaNoVolumeSupport)
                              : l10n.dlnaNoAvTransportSupport;
                          return ListTile(
                            dense: true,
                            enabled: ok,
                            leading: const Icon(Icons.speaker),
                            title: Text(d.friendlyName),
                            subtitle: subtitle == null ? null : Text(subtitle),
                            trailing: selected
                                ? Icon(
                                    Icons.check_circle,
                                    color: theme.colorScheme.primary,
                                  )
                                : null,
                            selected: selected,
                            onTap: ok
                                ? () => setState(() => _selected = d)
                                : null,
                          );
                        },
                      ),
                    );
                  },
                ),
              ),
              const SizedBox(height: 16),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: Text(cancelLabel),
                  ),
                  const SizedBox(width: 8),
                  FilledButton(
                    onPressed: () {
                      final d = _selected;
                      if (d == null) {
                        Navigator.of(context).pop(
                          _DlnaActionResult(
                            applySelection: true,
                            selected: null,
                            message: l10n.dlnaSwitchedToLocal,
                          ),
                        );
                        return;
                      }
                      if (d.avTransportControlUrl == null) {
                        Navigator.of(context).pop(
                          _DlnaActionResult(
                            applySelection: false,
                            message: l10n.dlnaNoAvTransportSupport,
                          ),
                        );
                        return;
                      }
                      Navigator.of(context).pop(
                        _DlnaActionResult(
                          applySelection: true,
                          selected: d,
                          message: l10n.dlnaSelected(d.friendlyName),
                        ),
                      );
                    },
                    child: Text(okLabel),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _DlnaEmptyState extends StatelessWidget {
  const _DlnaEmptyState({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.onRetry,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 12),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 36, color: theme.colorScheme.onSurfaceVariant),
            const SizedBox(height: 12),
            Text(title, style: theme.textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              subtitle,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            FilledButton.tonalIcon(
              onPressed: onRetry,
              icon: const Icon(Icons.refresh),
              label: Text(l10n.refresh),
            ),
          ],
        ),
      ),
    );
  }
}
