import 'package:stellatune/ui/preview/settings_preview.dart';
import 'package:stellatune/ui/theme/desktop_theme.dart';
import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart' show PlayerState;
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/pages/home/home_placeholders.dart';
import 'package:stellatune/ui/pages/home/home_view.dart';
import 'package:stellatune/ui/pages/home/home_view_data.dart';
import 'package:stellatune/ui/pages/home/widgets/home_artwork.dart';
import 'package:stellatune/ui/pages/shell/desktop_frame.dart';
import 'package:stellatune/ui/widgets/now_playing_bar/desktop_player_bar.dart';
import 'package:stellatune/ui/widgets/now_playing_common.dart';

/// Deterministic presentation harness. Uses production layout and local assets,
/// with no database, plugin, audio device or network initialization.
class HomeVisualPreview extends StatefulWidget {
  const HomeVisualPreview({
    super.key,
    this.data = HomePlaceholders.data,
    this.textScale = 1,
    this.desktopTheme = DesktopThemePreset.daylight,
    this.brightness = Brightness.light,
    this.onMinimize,
    this.onMaximize,
    this.onClose,
    this.onDrag,
    this.contentBuilder,
    this.initialDestination = 0,
  });
  final HomeViewData data;
  final double textScale;
  final DesktopThemePreset desktopTheme;
  final Brightness brightness;
  final WidgetBuilder? contentBuilder;
  final int initialDestination;
  final VoidCallback? onMinimize, onMaximize, onClose, onDrag;
  @override
  State<HomeVisualPreview> createState() => _HomeVisualPreviewState();
}

class _HomeVisualPreviewState extends State<HomeVisualPreview> {
  bool playing = true;
  late DesktopThemePreset desktopTheme = widget.desktopTheme;
  late int selected = widget.initialDestination;
  double progress = .42;
  double volume = .65;
  String title = '开始懂了';
  String artwork = HomePlaceholders.artwork[0];

  void choose(HomeCardData item) => setState(() {
    title = item.title;
    artwork = item.artwork;
    playing = true;
  });

  @override
  Widget build(BuildContext context) => MaterialApp(
    debugShowCheckedModeBanner: false,
    locale: const Locale('zh'),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    theme: ThemeData(
      useMaterial3: true,
      fontFamily: 'NotoSansSC',
      colorScheme: ColorScheme.fromSeed(
        seedColor: const Color(0xFF4F629A),
        brightness: widget.brightness,
      ),
      visualDensity: VisualDensity.standard,
    ),
    builder: (context, child) => MediaQuery(
      data: MediaQuery.of(context)
          .copyWith(textScaler: TextScaler.linear(widget.textScale)),
      child: child!,
    ),
    home: ArtworkTheme(
      palette: desktopTheme.palette,
      child: Builder(
        builder: (context) {
          void placeholder() => ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(
              content: Text('这里暂时展示设计占位内容'),
              duration: Duration(seconds: 2),
            ),
          );
          Widget icon(IconData icon, String label) => SizedBox(
            width: 37,
            height: 34,
            child: IconButton(
              onPressed: placeholder,
              tooltip: label,
              icon: Icon(icon, size: 18),
              padding: EdgeInsets.zero,
            ),
          );
          return Scaffold(
            body: DesktopFrame(
              selectedIndex: selected,
              onDestinationSelected: (value) =>
                  setState(() => selected = value),
              onMinimize: widget.onMinimize,
              onMaximize: widget.onMaximize,
              onClose: widget.onClose,
              onDrag: widget.onDrag,
              onSearch: (_) => placeholder(),
              searchHint: selected == 1
                  ? AppLocalizations.of(context)!.catalogSearchHint
                  : null,
              playerBar: DesktopPlayerBar(
                title: title,
                subtitle: '孙燕姿 · 我要的幸福',
                cover: HomeArtwork(asset: artwork),
                isPlaying: playing,
                position: NowPlayingCommon.formatMs(
                  (progress * 268000).round(),
                ),
                duration: '04:28',
                onPlayPause: () => setState(() => playing = !playing),
                onPrevious: () => choose(HomePlaceholders.listening[1]),
                onNext: () => choose(HomePlaceholders.listening[2]),
                onShuffle: placeholder,
                onRepeat: placeholder,
                progress: NowPlayingProgressBar(
                  minHitHeight: 24,
                  durationMs: 268000,
                  positionMs: (progress * 268000).round(),
                  enabled: true,
                  // Freeze time for deterministic screenshots; seeking remains live.
                  audioStarted: false,
                  playerState: PlayerState.paused,
                  foregroundColor: ArtworkPalette.of(context).accent,
                  onSeekMs: (ms) => setState(() => progress = ms / 268000),
                ),
                trailing: Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    icon(Icons.volume_up_rounded, '音量'),
                    if (MediaQuery.sizeOf(context).width >= 1200)
                      SizedBox(
                        width: 50,
                        child: SliderTheme(
                          data: SliderTheme.of(context).copyWith(
                            trackHeight: 2,
                            thumbShape: const RoundSliderThumbShape(
                              enabledThumbRadius: 2,
                            ),
                            overlayShape: SliderComponentShape.noOverlay,
                          ),
                          child: Slider(
                            value: volume,
                            onChanged: (v) => setState(() => volume = v),
                          ),
                        ),
                      ),
                    icon(Icons.cast_rounded, '投放'),
                    icon(Icons.lyrics_outlined, '歌词'),
                    if (MediaQuery.sizeOf(context).width >= 1250) ...[
                      const SizedBox(width: 10),
                      const _FormatTag('FLAC'),
                      const SizedBox(width: 6),
                      const _FormatTag('44.1 kHz / 16 bit'),
                      const SizedBox(width: 10),
                    ],
                    icon(Icons.queue_music_rounded, '队列'),
                  ],
                ),
              ),
              child:
                  widget.contentBuilder?.call(context) ??
                  (selected == 3
                      ? SettingsVisualPreview(
                          desktopTheme: desktopTheme,
                          onDesktopThemeChanged: (value) =>
                              setState(() => desktopTheme = value),
                        )
                      : HomeView(
                          data: widget.data,
                          onResume: () => setState(() => playing = true),
                          onContinue: (index) =>
                              choose(widget.data.continueListening[index]),
                          onRecent: (index) =>
                              choose(widget.data.recentlyAdded[index]),
                          onOpenLibrary: placeholder,
                          onOpenPlaceholder: placeholder,
                        )),
            ),
          );
        },
      ),
    ),
  );
}

class _FormatTag extends StatelessWidget {
  const _FormatTag(this.text);
  final String text;
  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
    decoration: BoxDecoration(
      borderRadius: BorderRadius.circular(8),
      border: Border.all(color: ArtworkPalette.of(context).outline),
    ),
    child: Text(
      text,
      style: TextStyle(
        fontSize: 9,
        color: ArtworkPalette.of(context).onSurfaceVariant,
      ),
    ),
  );
}
