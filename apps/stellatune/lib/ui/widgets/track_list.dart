import 'dart:io';
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/audio_format_badge.dart';

class TrackList extends StatefulWidget {
  const TrackList({
    super.key,
    required this.coverDir,
    required this.items,
    required this.onActivate,
    required this.onEnqueue,
  });

  final String coverDir;
  final List<TrackLite> items;
  final Future<void> Function(int index, List<TrackLite> items) onActivate;
  final Future<void> Function(TrackLite track) onEnqueue;

  @override
  State<TrackList> createState() => _TrackListState();
}

class _TrackListState extends State<TrackList> {
  final ScrollController _controller = ScrollController();
  Timer? _settleTimer;

  bool _deferHeavy = false;
  double _lastPixels = 0.0;
  int _lastMicros = 0;

  @override
  void dispose() {
    _settleTimer?.cancel();
    _controller.dispose();
    super.dispose();
  }

  bool _onScrollNotification(ScrollNotification n) {
    final nowMicros = DateTime.now().microsecondsSinceEpoch;
    final pixels = n.metrics.pixels;

    final dtMicros = _lastMicros == 0 ? 0 : (nowMicros - _lastMicros);
    final deltaPx = (pixels - _lastPixels).abs();
    final dtMs = dtMicros / 1000.0;
    final speed = (dtMs <= 0) ? 0.0 : (deltaPx / dtMs); // px/ms

    _lastMicros = nowMicros;
    _lastPixels = pixels;

    // Treat big jumps (typical when dragging the scrollbar thumb) as "fast scrolling"
    // and temporarily render lighter rows. This keeps scroll thumb tracking snappy,
    // while content fills in shortly after the user stops.
    final viewport = n.metrics.viewportDimension;
    final isFast = deltaPx > viewport * 0.6 || speed > 5.0; // ~5000 px/s

    if (isFast && !_deferHeavy) {
      setState(() => _deferHeavy = true);
    }

    _settleTimer?.cancel();
    _settleTimer = Timer(const Duration(milliseconds: 160), () {
      if (!mounted) return;
      if (_deferHeavy) setState(() => _deferHeavy = false);
    });

    return false;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    if (widget.items.isEmpty) {
      return Center(child: Text(l10n.noResultsHint));
    }

    return NotificationListener<ScrollNotification>(
      onNotification: _onScrollNotification,
      child: CustomScrollView(
        controller: _controller,
        // Smaller cache while scrubbing a long list keeps rebuild work low; once settled, allow
        // more cache for normal wheel/trackpad scrolling.
        cacheExtent: _deferHeavy ? 200 : 800,
        slivers: [
          SliverFixedExtentList(
            itemExtent: 72,
            delegate: SliverChildBuilderDelegate((context, i) {
              final t = widget.items[i];
              final title = (t.title ?? '').trim();
              final artist = (t.artist ?? '').trim();
              final album = (t.album ?? '').trim();

              final line1 = title.isNotEmpty ? title : _basename(t.path);
              final line2 = [
                artist,
                album,
              ].where((s) => s.isNotEmpty).join(' • ');
              final coverPath =
                  '${widget.coverDir}${Platform.pathSeparator}${t.id}';

              return DecoratedBox(
                decoration: BoxDecoration(
                  border: Border(
                    bottom: BorderSide(
                      color: Theme.of(context).dividerColor,
                      width: 0.5,
                    ),
                  ),
                ),
                child: ListTile(
                  dense: true,
                  leading: _deferHeavy
                      ? const _CoverPlaceholder()
                      : _CoverThumb(path: coverPath),
                  title: Text(
                    line1,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  subtitle: _deferHeavy
                      ? null
                      : Row(
                          children: [
                            AudioFormatBadge(path: t.path),
                            Expanded(
                              child: Text(
                                line2.isNotEmpty ? line2 : t.path,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                          ],
                        ),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      _DurationText(ms: t.durationMs?.toInt()),
                      const SizedBox(width: 8),
                      if (!_deferHeavy)
                        PopupMenuButton<_TrackAction>(
                          onSelected: (action) async {
                            if (action == _TrackAction.enqueue) {
                              await widget.onEnqueue(t);
                            } else if (action == _TrackAction.play) {
                              await widget.onActivate(i, widget.items);
                            }
                          },
                          itemBuilder: (context) => [
                            PopupMenuItem(
                              value: _TrackAction.play,
                              child: Text(l10n.menuPlay),
                            ),
                            PopupMenuItem(
                              value: _TrackAction.enqueue,
                              child: Text(l10n.menuEnqueue),
                            ),
                          ],
                        ),
                    ],
                  ),
                  onTap: () => widget.onActivate(i, widget.items),
                ),
              );
            }, childCount: widget.items.length),
          ),
        ],
      ),
    );
  }

  static String _basename(String path) {
    final parts = path.split(RegExp(r'[\\/]+'));
    return parts.isEmpty ? path : parts.last;
  }
}

enum _TrackAction { play, enqueue }

class _CoverPlaceholder extends StatelessWidget {
  const _CoverPlaceholder();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        color: theme.colorScheme.surfaceContainerHighest,
      ),
      child: Icon(Icons.music_note, color: theme.colorScheme.onSurfaceVariant),
    );
  }
}

class _CoverThumb extends StatelessWidget {
  const _CoverThumb({required this.path});

  final String path;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final placeholder = Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        color: theme.colorScheme.primary.withValues(alpha: 0.10),
        border: Border.all(
          color: theme.colorScheme.primary.withValues(alpha: 0.15),
        ),
      ),
      child: Icon(Icons.music_note, color: theme.colorScheme.primary),
    );

    final provider = ResizeImage(
      FileImage(File(path)),
      width: 80,
      height: 80,
      allowUpscaling: false,
    );

    return ClipRRect(
      borderRadius: BorderRadius.circular(8),
      child: Image(
        image: provider,
        width: 40,
        height: 40,
        fit: BoxFit.cover,
        filterQuality: FilterQuality.low,
        gaplessPlayback: true,
        errorBuilder: (context, error, stackTrace) => placeholder,
      ),
    );
  }
}

class _DurationText extends StatelessWidget {
  const _DurationText({required this.ms});

  final int? ms;

  @override
  Widget build(BuildContext context) {
    final v = ms;
    if (v == null || v <= 0) return const SizedBox.shrink();
    final totalSeconds = (v / 1000).floor();
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    return Text(
      '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}',
      style: Theme.of(context).textTheme.bodySmall,
    );
  }
}
