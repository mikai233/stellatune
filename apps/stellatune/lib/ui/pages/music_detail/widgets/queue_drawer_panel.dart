import 'dart:io';
import 'dart:ui';
import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/widgets/marquee_text.dart';

class QueueDrawerPanel extends ConsumerStatefulWidget {
  const QueueDrawerPanel({super.key});

  @override
  ConsumerState<QueueDrawerPanel> createState() => _QueueDrawerPanelState();
}

class _QueueDrawerPanelState extends ConsumerState<QueueDrawerPanel> {
  final _listKey = GlobalKey<_QueueListState>();

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final queue = ref.watch(queueControllerProvider);
    final coverDir = ref.watch(coverDirProvider);
    final sourceLabel = (queue.sourceLabel ?? '').trim();
    final displaySourceLabel = sourceLabel.isEmpty
        ? l10n.queueSourceUnset
        : sourceLabel;

    return SafeArea(
      left: false,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10, 12, 12, 12),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(18),
          child: BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: theme.colorScheme.surface.withValues(alpha: 0.78),
                border: Border.all(
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.14),
                ),
              ),
              child: Column(
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(12, 10, 8, 6),
                    child: Row(
                      children: [
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                l10n.queueTitle,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: theme.textTheme.titleMedium?.copyWith(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              const SizedBox(height: 2),
                              Text(
                                displaySourceLabel,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: theme.textTheme.bodySmall?.copyWith(
                                  color: theme.colorScheme.onSurfaceVariant,
                                ),
                              ),
                            ],
                          ),
                        ),
                        IconButton(
                          onPressed: () => _listKey.currentState
                              ?.scrollToCurrent(animated: true),
                          icon: const Icon(Icons.my_location_rounded),
                        ),
                      ],
                    ),
                  ),
                  Divider(
                    height: 1,
                    thickness: 0.8,
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.12),
                  ),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(8, 8, 8, 10),
                      child: _QueueList(
                        key: _listKey,
                        coverDir: coverDir,
                        items: queue.items,
                        currentIndex: queue.currentIndex,
                        onActivate: (i) => ref
                            .read(playbackControllerProvider.notifier)
                            .playIndex(i),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _QueueList extends StatefulWidget {
  const _QueueList({
    super.key,
    required this.coverDir,
    required this.items,
    required this.currentIndex,
    required this.onActivate,
  });

  final String coverDir;
  final List<QueueItem> items;
  final int? currentIndex;
  final void Function(int index) onActivate;

  @override
  State<_QueueList> createState() => _QueueListState();
}

class _QueueListState extends State<_QueueList> {
  static const _itemExtent = 76.0;
  late final ScrollController _scrollController;

  @override
  void initState() {
    super.initState();
    final current = widget.currentIndex ?? 0;
    final initialOffset = math.max(0.0, (current - 3) * _itemExtent);
    _scrollController = ScrollController(initialScrollOffset: initialOffset);
  }

  @override
  void didUpdateWidget(covariant _QueueList oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.currentIndex != oldWidget.currentIndex) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        scrollToCurrent(animated: true);
      });
    }
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  void scrollToCurrent({required bool animated}) {
    final current = widget.currentIndex;
    if (current == null || !_scrollController.hasClients) {
      return;
    }
    final position = _scrollController.position;
    final centeredOffset =
        current * _itemExtent - (position.viewportDimension - _itemExtent) / 2;
    final target = centeredOffset.clamp(0.0, position.maxScrollExtent);
    if (animated) {
      _scrollController.animateTo(
        target,
        duration: const Duration(milliseconds: 260),
        curve: Curves.easeOutCubic,
      );
    } else {
      _scrollController.jumpTo(target);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    if (widget.items.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              Icons.queue_music,
              size: 28,
              color: theme.colorScheme.onSurfaceVariant,
            ),
            const SizedBox(height: 8),
            Text(l10n.queueEmpty),
          ],
        ),
      );
    }

    return ListView.builder(
      controller: _scrollController,
      itemExtent: _itemExtent,
      cacheExtent: _itemExtent * 8,
      itemCount: widget.items.length,
      itemBuilder: (context, i) {
        final item = widget.items[i];
        final selected = widget.currentIndex == i;
        final album = (item.album ?? '').trim();
        final artist = (item.artist ?? '').trim();
        final subtitle = album.isNotEmpty
            ? (artist.isNotEmpty ? '$album • $artist' : album)
            : artist;
        final durationText = _formatDuration(item.durationMs);

        return Padding(
          padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 4),
          child: Material(
            color: selected
                ? theme.colorScheme.secondaryContainer.withValues(alpha: 0.92)
                : theme.colorScheme.surfaceContainerHighest.withValues(
                    alpha: 0.28,
                  ),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
              side: selected
                  ? BorderSide(
                      color: theme.colorScheme.secondary.withValues(alpha: 0.7),
                    )
                  : BorderSide.none,
            ),
            child: InkWell(
              borderRadius: BorderRadius.circular(14),
              onTap: () => widget.onActivate(i),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(8, 8, 8, 8),
                child: Row(
                  children: [
                    _QueueCover(
                      coverDir: widget.coverDir,
                      trackId: item.id,
                      highPriority: selected,
                      deferMs: selected ? 0 : 40 + (i % 7) * 18,
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          SizedBox(
                            height: 22,
                            child: selected
                                ? MarqueeText(
                                    text: item.displayTitle,
                                    style: theme.textTheme.titleSmall?.copyWith(
                                      fontWeight: FontWeight.w700,
                                    ),
                                    pixelsPerSecond: 34,
                                    pauseDuration: const Duration(
                                      milliseconds: 1300,
                                    ),
                                    gap: 28,
                                  )
                                : Text(
                                    item.displayTitle,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: theme.textTheme.titleSmall?.copyWith(
                                      fontWeight: FontWeight.w600,
                                    ),
                                  ),
                          ),
                          const SizedBox(height: 4),
                          SizedBox(
                            height: 18,
                            child: selected
                                ? MarqueeText(
                                    text: subtitle.isEmpty ? ' ' : subtitle,
                                    style: theme.textTheme.bodySmall?.copyWith(
                                      color: theme.colorScheme.onSurfaceVariant,
                                    ),
                                    pixelsPerSecond: 24,
                                    pauseDuration: const Duration(
                                      milliseconds: 1500,
                                    ),
                                    gap: 24,
                                  )
                                : Text(
                                    subtitle,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: theme.textTheme.bodySmall?.copyWith(
                                      color: theme.colorScheme.onSurfaceVariant,
                                    ),
                                  ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 8),
                    SizedBox(
                      width: 52,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.end,
                        children: [
                          if (durationText != null)
                            Text(
                              durationText,
                              style: theme.textTheme.labelMedium?.copyWith(
                                color: theme.colorScheme.onSurfaceVariant,
                              ),
                            ),
                          const SizedBox(height: 6),
                          Icon(
                            selected
                                ? Icons.play_circle_fill
                                : Icons.play_circle_outline,
                            size: 18,
                            color: selected
                                ? theme.colorScheme.primary
                                : theme.colorScheme.onSurfaceVariant,
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }

  static String? _formatDuration(int? ms) {
    final v = ms;
    if (v == null || v <= 0) return null;
    final totalSeconds = (v / 1000).floor();
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }
}

class _QueueCover extends StatefulWidget {
  const _QueueCover({
    required this.coverDir,
    required this.trackId,
    required this.highPriority,
    required this.deferMs,
  });

  final String coverDir;
  final int? trackId;
  final bool highPriority;
  final int deferMs;

  @override
  State<_QueueCover> createState() => _QueueCoverState();
}

class _QueueCoverState extends State<_QueueCover> {
  Timer? _loadTimer;
  bool _ready = false;

  @override
  void initState() {
    super.initState();
    _scheduleLoad();
  }

  @override
  void didUpdateWidget(covariant _QueueCover oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.trackId != widget.trackId ||
        oldWidget.highPriority != widget.highPriority ||
        oldWidget.deferMs != widget.deferMs) {
      _scheduleLoad();
    }
  }

  @override
  void dispose() {
    _loadTimer?.cancel();
    super.dispose();
  }

  void _scheduleLoad() {
    _loadTimer?.cancel();
    final id = widget.trackId;
    if (id == null) {
      _ready = false;
      return;
    }
    if (widget.highPriority || widget.deferMs <= 0) {
      _ready = true;
      return;
    }
    _ready = false;
    _loadTimer = Timer(Duration(milliseconds: widget.deferMs), () {
      if (!mounted) return;
      setState(() => _ready = true);
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final placeholder = Container(
      width: 52,
      height: 52,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(10),
        color: theme.colorScheme.primary.withValues(alpha: 0.14),
        border: Border.all(
          color: theme.colorScheme.primary.withValues(alpha: 0.22),
        ),
      ),
      child: Icon(Icons.music_note, color: theme.colorScheme.primary),
    );

    if (widget.trackId == null) {
      return placeholder;
    }
    if (!_ready) {
      return placeholder;
    }
    final path = '${widget.coverDir}${Platform.pathSeparator}${widget.trackId}';
    final provider = ResizeImage(
      FileImage(File(path)),
      width: 96,
      height: 96,
      allowUpscaling: false,
    );
    return ClipRRect(
      borderRadius: BorderRadius.circular(10),
      child: Image(
        image: provider,
        width: 52,
        height: 52,
        fit: BoxFit.cover,
        filterQuality: FilterQuality.low,
        gaplessPlayback: true,
        errorBuilder: (context, error, stackTrace) => placeholder,
      ),
    );
  }
}
