import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/l10n/app_localizations_en.dart';
import 'package:stellatune/player/queue_models.dart' show PlayMode;

/// Stateless layout; playback actions and the progress widget stay connected to
/// the existing controller in NowPlayingBar.
class DesktopPlayerBar extends StatelessWidget {
  const DesktopPlayerBar({
    super.key,
    required this.title,
    required this.subtitle,
    required this.cover,
    required this.isPlaying,
    required this.position,
    required this.duration,
    required this.progress,
    required this.trailing,
    this.onPlayPause,
    this.onPrevious,
    this.onNext,
    this.onPlayMode,
    this.playMode = PlayMode.sequential,
  });
  final String title, subtitle, position, duration;
  final Widget cover, progress, trailing;
  final bool isPlaying;
  final PlayMode playMode;
  final VoidCallback? onPlayPause, onPrevious, onNext, onPlayMode;

  @override
  Widget build(BuildContext context) => Theme(
    data: ArtworkPalette.of(context).applyTo(Theme.of(context)),
    child: Container(
      height: 78,
      decoration: BoxDecoration(
        color: ArtworkPalette.of(context).playerSurface,
      ),
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          Positioned.fill(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 8, 20, 0),
              child: LayoutBuilder(
                builder: (context, constraints) => Row(
                  children: [
                    Expanded(
                      flex: 3,
                      child: Row(
                        children: [
                          SizedBox(
                            width: 50,
                            height: 50,
                            child: ClipRRect(
                              borderRadius: BorderRadius.circular(6),
                              child: cover,
                            ),
                          ),
                          const SizedBox(width: 14),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                Text(
                                  title,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    fontSize: 14,
                                    fontWeight: FontWeight.w500,
                                    color: ArtworkPalette.of(context).onSurface,
                                  ),
                                ),
                                const SizedBox(height: 4),
                                Text(
                                  subtitle,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    fontSize: 12,
                                    color: ArtworkPalette.of(context)
                                        .onSurfaceVariant,
                                  ),
                                ),
                              ],
                            ),
                          ),
                          const SizedBox(width: 20),
                        ],
                      ),
                    ),
                    SizedBox(
                      width: constraints.maxWidth < 1100 ? 220 : 280,
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          SizedBox(
                            height: 43,
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                _Control(
                                  icon: switch (playMode) {
                                    PlayMode.sequential =>
                                      Icons.arrow_forward_rounded,
                                    PlayMode.shuffle => Icons.shuffle_rounded,
                                    PlayMode.repeatAll => Icons.repeat_rounded,
                                    PlayMode.repeatOne =>
                                      Icons.repeat_one_rounded,
                                  },
                                  label: _playModeLabel(context),
                                  onTap: onPlayMode,
                                  selected: playMode != PlayMode.sequential,
                                ),
                                SizedBox(
                                  width: constraints.maxWidth < 1100 ? 8 : 24,
                                ),
                                _Control(
                                  icon: Icons.skip_previous_rounded,
                                  label: '上一首',
                                  onTap: onPrevious,
                                ),
                                const SizedBox(width: 12),
                                SizedBox(
                                  width: 38,
                                  height: 38,
                                  child: IconButton.filled(
                                    key: const ValueKey('player-play-pause'),
                                    style: IconButton.styleFrom(
                                      backgroundColor: ArtworkPalette.of(
                                        context,
                                      ).accent,
                                      foregroundColor: ArtworkPalette.of(
                                        context,
                                      ).onAccent,
                                      padding: EdgeInsets.zero,
                                    ),
                                    onPressed: onPlayPause,
                                    tooltip: isPlaying ? '暂停' : '播放',
                                    icon: Icon(
                                      isPlaying
                                          ? Icons.pause_rounded
                                          : Icons.play_arrow_rounded,
                                      size: 25,
                                    ),
                                  ),
                                ),
                                const SizedBox(width: 12),
                                _Control(
                                  icon: Icons.skip_next_rounded,
                                  label: '下一首',
                                  onTap: onNext,
                                ),
                                SizedBox(
                                  width: constraints.maxWidth < 1100 ? 8 : 24,
                                ),
                                // Balance the mode button so the transport stays centered.
                                const SizedBox(width: 30),
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 24),
                    Expanded(
                      flex: 3,
                      child: Row(
                        children: [
                          Text(
                            '$position / $duration',
                            style: TextStyle(
                              fontSize: 13,
                              fontFeatures: const [
                                FontFeature.tabularFigures(),
                              ],
                              color: ArtworkPalette.of(context)
                                  .onSurfaceVariant,
                            ),
                          ),
                          const SizedBox(width: 16),
                          Expanded(child: trailing),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
          Positioned(
            key: const ValueKey('player-top-progress'),
            // The progress track is centered 3 px into its resting 6 px box.
            // Align that center with the player's top edge, including on hover.
            top: -3,
            left: 0,
            right: 0,
            child: RepaintBoundary(child: progress),
          ),
        ],
      ),
    ),
  );

  String _playModeLabel(BuildContext context) {
    final l = AppLocalizations.of(context) ?? AppLocalizationsEn();
    return switch (playMode) {
      PlayMode.sequential => l.playModeSequential,
      PlayMode.shuffle => l.playModeShuffle,
      PlayMode.repeatAll => l.playModeRepeatAll,
      PlayMode.repeatOne => l.playModeRepeatOne,
    };
  }
}

class _Control extends StatelessWidget {
  const _Control({
    required this.icon,
    required this.label,
    this.onTap,
    this.selected = false,
  });
  final IconData icon;
  final String label;
  final bool selected;
  final VoidCallback? onTap;
  @override
  Widget build(BuildContext context) => SizedBox(
    width: 30,
    height: 30,
    child: IconButton(
      onPressed: onTap,
      tooltip: label,
      padding: EdgeInsets.zero,
      icon: Icon(
        icon,
        size: 19,
        color: selected
            ? ArtworkPalette.of(context).accent
            : ArtworkPalette.of(context).onSurfaceVariant,
      ),
    ),
  );
}
