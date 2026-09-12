import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_track_sort.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/l10n/app_localizations_en.dart';

import 'catalog_column_layout.dart';

import 'package:stellatune/ui/widgets/track_cover_provider.dart';

extension CatalogLocalization on BuildContext {
  AppLocalizations get catalogL10n =>
      AppLocalizations.of(this) ?? AppLocalizationsEn();
}

String catalogKindLabel(BuildContext context, MediaKind kind) {
  final l = context.catalogL10n;
  return switch (kind) {
    MediaKind.track => l.catalogSongs,
    MediaKind.album => l.catalogAlbums,
    MediaKind.artist => l.catalogArtists,
    MediaKind.folder => l.catalogFolders,
    MediaKind.playlist => l.catalogPlaylists,
  };
}

String catalogTitle(BuildContext context, CatalogItem item) =>
    item.title.isEmpty
    ? context.catalogL10n.catalogUnknown(
        catalogKindLabel(context, item.reference.kind),
      )
    : item.title;

IconData catalogKindIcon(MediaKind kind) => switch (kind) {
  MediaKind.track => Icons.music_note_outlined,
  MediaKind.album => Icons.album_outlined,
  MediaKind.artist => Icons.person_outline,
  MediaKind.folder => Icons.folder_outlined,
  MediaKind.playlist => Icons.queue_music,
};

class CatalogArtwork extends ConsumerWidget {
  const CatalogArtwork({super.key, required this.item, this.size = 36});
  final CatalogItem item;
  final double size;
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = Theme.of(context).colorScheme;
    final fallback = ColoredBox(
      color: scheme.primary.withValues(alpha: .07),
      child: Center(
        child: Icon(
          catalogKindIcon(item.reference.kind),
          size: size * .42,
          color: scheme.primary.withValues(alpha: .55),
        ),
      ),
    );
    final Widget image;
    if (_CatalogScrollActivity.maybeOf(context)?.value == true) {
      image = fallback;
    } else if (item.artworkUrl?.isNotEmpty == true) {
      image = Image.network(
        item.artworkUrl!,
        fit: BoxFit.cover,
        width: size,
        height: size,
        frameBuilder: (_, child, frame, _) => frame == null ? fallback : child,
        errorBuilder: (_, _, _) => fallback,
      );
    } else if (item.localTrackId != null) {
      final coverDir = ref.watch(coverDirProvider);
      image = Image(
        image: size <= 48
            ? localTrackCoverProvider(coverDir, item.localTrackId!.toInt())
            : FileImage(
                File('$coverDir${Platform.pathSeparator}${item.localTrackId}'),
              ),
        fit: BoxFit.cover,
        width: size,
        height: size,
        frameBuilder: (_, child, frame, _) => frame == null ? fallback : child,
        errorBuilder: (_, _, _) => fallback,
      );
    } else {
      image = fallback;
    }
    return SizedBox.square(
      dimension: size,
      child: ClipRRect(
        borderRadius: BorderRadius.circular(
          item.reference.kind == MediaKind.artist ? size / 2 : 7,
        ),
        child: image,
      ),
    );
  }
}

/// Measure once for the whole table, using the row font and accessibility scale.
double catalogTrackNumberWidth(BuildContext context, int trackCount) {
  final digits = trackCount.toString().length;
  final painter = TextPainter(
    textDirection: Directionality.of(context),
    textScaler: MediaQuery.textScalerOf(context),
    maxLines: 1,
  );
  final style = TextStyle(
    fontFamily: Theme.of(context).textTheme.bodyMedium?.fontFamily,
    fontSize: 13,
  );
  var width = 18.0;
  for (var digit = 0; digit <= 9; digit++) {
    painter.text = TextSpan(text: '$digit' * digits, style: style);
    painter.layout();
    if (painter.width > width) width = painter.width;
  }
  painter.dispose();
  return width.ceilToDouble();
}

/// The same responsive column geometry is used by the header and every row.
class CatalogTrackColumns extends ConsumerWidget {
  const CatalogTrackColumns({
    super.key,
    required this.number,
    required this.title,
    required this.artist,
    required this.album,
    required this.duration,
    required this.actions,
    this.numberWidth = 30,
    this.resizable = false,
  });
  final bool resizable;
  final double numberWidth;
  final Widget number, title, artist, album, duration, actions;
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final saved = ref.watch(catalogColumnWidthsProvider);
    return LayoutBuilder(
      builder: (context, constraints) {
        final layout = CatalogColumnLayout(
          constraints.maxWidth,
          numberWidth,
          saved,
        );
        final cells = {
          CatalogTrackColumn.original: number,
          CatalogTrackColumn.title: title,
          CatalogTrackColumn.artist: artist,
          CatalogTrackColumn.album: album,
          CatalogTrackColumn.duration: duration,
        };
        final body = Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12),
          child: Row(
            children: [
              for (final column in layout.columns) ...[
                SizedBox(
                  key: ValueKey('catalog-column-${column.name}'),
                  width: layout.widths[column],
                  child: layout.visibility[column] == 1
                      ? cells[column]
                      : ClipRect(
                          child: OverflowBox(
                            alignment: Alignment.centerLeft,
                            minWidth:
                                layout.widths[column]! /
                                layout.visibility[column]!,
                            maxWidth:
                                layout.widths[column]! /
                                layout.visibility[column]!,
                            child: IgnorePointer(
                              ignoring: layout.visibility[column]! < .5,
                              child: ExcludeFocus(
                                excluding: layout.visibility[column]! < .5,
                                child: ExcludeSemantics(
                                  excluding: layout.visibility[column]! < .5,
                                  child: Opacity(
                                    opacity: layout.visibility[column]!,
                                    child: cells[column],
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
                ),
                SizedBox(
                  key: ValueKey('catalog-gap-${column.name}'),
                  width: layout.gaps[column],
                ),
              ],
              SizedBox(width: layout.actionsWidth, child: actions),
            ],
          ),
        );
        if (!resizable) return body;
        return Stack(
          children: [
            body,
            for (var i = 0; i < layout.columns.length - 1; i++)
              if (layout.visibility[layout.columns[i]]! >= .5 &&
                  layout.visibility[layout.columns[i + 1]]! >= .5)
                Positioned(
                  left: layout.boundaryOffset(i) - 6,
                  top: 0,
                  bottom: 0,
                  width: 12,
                  child: _CatalogColumnHandle(
                    key: ValueKey('catalog-resize-${layout.columns[i].name}'),
                    layout: layout,
                    boundary: i,
                  ),
                ),
          ],
        );
      },
    );
  }
}

class _CatalogColumnHandle extends ConsumerStatefulWidget {
  const _CatalogColumnHandle({
    super.key,
    required this.layout,
    required this.boundary,
  });
  final CatalogColumnLayout layout;
  final int boundary;
  @override
  ConsumerState<_CatalogColumnHandle> createState() =>
      _CatalogColumnHandleState();
}

class _CatalogColumnHandleState extends ConsumerState<_CatalogColumnHandle> {
  bool hovered = false, focused = false, dragging = false;
  double startX = 0;
  CatalogColumnLayout? startLayout;

  CatalogColumnWidthsController get controller =>
      ref.read(catalogColumnWidthsProvider.notifier);

  void step(double delta) {
    controller.resize(widget.layout, widget.boundary, delta);
    unawaited(controller.save());
  }

  @override
  Widget build(BuildContext context) => Tooltip(
    message: context.catalogL10n.catalogResizeColumns,
    child: Semantics(
      label: context.catalogL10n.catalogResizeColumns,
      onIncrease: () => step(8),
      onDecrease: () => step(-8),
      child: Focus(
        onFocusChange: (value) => setState(() => focused = value),
        onKeyEvent: (_, event) {
          if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
            return KeyEventResult.ignored;
          }
          if (event.logicalKey == LogicalKeyboardKey.arrowLeft) {
            step(-8);
          } else if (event.logicalKey == LogicalKeyboardKey.arrowRight) {
            step(8);
          } else if (event.logicalKey == LogicalKeyboardKey.home) {
            unawaited(controller.reset());
          } else {
            return KeyEventResult.ignored;
          }
          return KeyEventResult.handled;
        },
        child: MouseRegion(
          cursor: SystemMouseCursors.resizeColumn,
          onEnter: (_) => setState(() => hovered = true),
          onExit: (_) => setState(() => hovered = false),
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onDoubleTap: () => unawaited(controller.reset()),
            onHorizontalDragStart: (details) {
              startX = details.globalPosition.dx;
              startLayout = widget.layout;
              setState(() => dragging = true);
            },
            onHorizontalDragUpdate: (details) => controller.resize(
              startLayout!,
              widget.boundary,
              details.globalPosition.dx - startX,
            ),
            onHorizontalDragEnd: (_) {
              setState(() => dragging = false);
              unawaited(controller.save());
            },
            onHorizontalDragCancel: () {
              setState(() => dragging = false);
              unawaited(controller.save());
            },
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 8),
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 100),
                color: Theme.of(context).colorScheme.primary
                    .withValues(alpha: hovered || focused || dragging ? .5 : 0),
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

class CatalogTrackHeader extends StatelessWidget {
  const CatalogTrackHeader({
    super.key,
    this.sort = const CatalogTrackSort(),
    this.onSort,
    this.numberWidth = 30,
  });
  final double numberWidth;
  final CatalogTrackSort sort;
  final ValueChanged<CatalogTrackColumn>? onSort;
  @override
  Widget build(BuildContext context) {
    final l = context.catalogL10n;
    Widget heading(
      String label,
      CatalogTrackColumn column, {
      Alignment alignment = Alignment.centerLeft,
    }) {
      final active = sort.column == column;
      final direction = sort.descending
          ? l.catalogSortDescending
          : l.catalogSortAscending;
      final button = Tooltip(
        message: column == CatalogTrackColumn.original
            ? l.catalogDefaultOrder
            : '$label · ${active && !sort.descending ? l.catalogSortDescending : l.catalogSortAscending}',
        child: Semantics(
          selected: active,
          value: active
              ? column == CatalogTrackColumn.original
                    ? l.catalogDefaultOrder
                    : direction
              : null,
          child: InkWell(
            key: ValueKey('catalog-sort-${column.name}'),
            onTap: onSort == null ? null : () => onSort!(column),
            borderRadius: BorderRadius.circular(6),
            hoverColor: Theme.of(context).colorScheme.primary
                .withValues(alpha: .05),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Flexible(child: Text(label)),
                  if (active && column != CatalogTrackColumn.original) ...[
                    const SizedBox(width: 2),
                    Icon(
                      sort.descending
                          ? Icons.arrow_downward
                          : Icons.arrow_upward,
                      size: 12,
                      color: Theme.of(context).colorScheme.primary,
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      );
      // Extend feedback into the gutters while keeping labels aligned with rows.
      // Short headings get a compact target with room around their text.
      return LayoutBuilder(
        builder: (context, constraints) => Center(
          child: OverflowBox(
            minWidth: constraints.maxWidth + 16,
            maxWidth: constraints.maxWidth + 16,
            child: Align(alignment: alignment, child: button),
          ),
        ),
      );
    }

    return SizedBox(
      height: 34,
      child: DefaultTextStyle(
        style: TextStyle(
          fontFamily: Theme.of(context).textTheme.bodySmall?.fontFamily,
          fontSize: 12,
          color: Theme.of(context).colorScheme.onSurfaceVariant,
        ),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        child: CatalogTrackColumns(
          resizable: onSort != null,
          numberWidth: numberWidth,
          number: heading(
            '#',
            CatalogTrackColumn.original,
            alignment: Alignment.center,
          ),
          title: Padding(
            padding: const EdgeInsets.only(left: 46),
            child: heading(l.catalogTitle, CatalogTrackColumn.title),
          ),
          artist: heading(l.catalogArtists, CatalogTrackColumn.artist),
          album: heading(l.catalogAlbums, CatalogTrackColumn.album),
          duration: heading(l.catalogDuration, CatalogTrackColumn.duration),
          actions: const SizedBox(),
        ),
      ),
    );
  }
}

class CatalogTrackRow extends StatefulWidget {
  const CatalogTrackRow({
    super.key,
    required this.item,
    required this.index,
    required this.onPlay,
    required this.actions,
    this.current = false,
    this.numberWidth = 30,
    this.onOpenArtist,
    this.onOpenAlbum,
    this.localArtists = false,
  });
  final void Function(MediaRef reference, String title)? onOpenArtist;
  final VoidCallback? onOpenAlbum;
  final bool localArtists;
  final double numberWidth;
  final CatalogItem item;
  final int index;
  final bool current;
  final VoidCallback? onPlay;
  final Widget actions;
  @override
  State<CatalogTrackRow> createState() => _CatalogTrackRowState();
}

class _CatalogTrackRowState extends State<CatalogTrackRow> {
  bool hovered = false, focused = false;
  @override
  Widget build(BuildContext context) {
    final item = widget.item;
    final scheme = Theme.of(context).colorScheme;
    final seconds = (item.durationMs?.toInt() ?? 0) ~/ 1000;
    final duration = item.durationMs == null
        ? '—'
        : '${seconds ~/ 60}:${(seconds % 60).toString().padLeft(2, '0')}';
    return MouseRegion(
      onEnter: (_) => setState(() => hovered = true),
      onExit: (_) => setState(() => hovered = false),
      child: Material(
        color: widget.current
            ? scheme.primary.withValues(alpha: .10)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(7),
        child: InkWell(
          onTap: widget.onPlay,
          onFocusChange: (v) => setState(() => focused = v),
          borderRadius: BorderRadius.circular(7),
          child: SizedBox(
            height: 48,
            child: DefaultTextStyle(
              style: TextStyle(
                fontFamily: Theme.of(context).textTheme.bodyMedium?.fontFamily,
                fontSize: 13,
                color: scheme.onSurfaceVariant,
              ),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              child: CatalogTrackColumns(
                numberWidth: widget.numberWidth,
                number: Center(
                  child: widget.current
                      ? Icon(Icons.equalizer, size: 17, color: scheme.primary)
                      : Text('${widget.index + 1}'),
                ),
                title: Row(
                  children: [
                    CatalogArtwork(item: item, size: 34),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Text(
                        catalogTitle(context, item),
                        style: TextStyle(color: scheme.onSurface),
                      ),
                    ),
                  ],
                ),
                artist: CatalogArtistLinks(
                  item: item,
                  local: widget.localArtists,
                  onOpen: widget.onOpenArtist,
                ),
                album: _CatalogMetadataLink(
                  text: item.album ?? '—',
                  onTap: widget.onOpenAlbum,
                ),
                duration: Text(duration),
                actions: LayoutBuilder(
                  builder: (context, constraints) => Opacity(
                    // Keep the menu accessible to keyboard and touch, and reveal hover actions.
                    opacity: hovered || focused || widget.current ? 1 : .35,
                    child: widget.actions,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

// Share in-flight remote lookups between visible rows. Dispose when no row
// uses the artist; local catalog names are already encoded in their IDs.
final _artistDetailProvider = FutureProvider.autoDispose
    .family<CatalogItem, MediaRef>((ref, artist) {
      return ref.watch(catalogBridgeProvider).detail(artist);
    });

class CatalogArtistLinks extends ConsumerWidget {
  const CatalogArtistLinks({
    super.key,
    required this.item,
    required this.local,
    this.onOpen,
  });

  final CatalogItem item;
  final bool local;
  final void Function(MediaRef reference, String title)? onOpen;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final artists = item.artistRefs
        .where(
          (r) =>
              r.kind == MediaKind.artist &&
              r.sourceInstanceId == item.reference.sourceInstanceId,
        )
        .toSet()
        .toList();
    if (artists.isEmpty) return Text(item.artist ?? '—');
    if (artists.length == 1) {
      return _CatalogMetadataLink(
        text: item.artist ?? '—',
        onTap: onOpen == null
            ? null
            : () => onOpen!(artists.single, item.artist ?? ''),
      );
    }

    // Only the local provider defines IDs as JSON-encoded artist names. Never
    // split display credits to guess which name belongs to a remote opaque ID.
    final names = <String>[];
    for (final artist in artists) {
      String? name;
      if (local) {
        try {
          final decoded = jsonDecode(artist.id);
          if (decoded is String) name = decoded;
        } on FormatException {
          // Older/custom IDs can still resolve through the catalog provider.
        }
      }
      name ??= ref.watch(_artistDetailProvider(artist)).asData?.value.title;
      names.add(name ?? '');
    }
    // Retain the original credit while remote names load, without a spinner
    // or a guessed navigation target. Local names are available immediately.
    if (names.any((name) => name.isEmpty)) return Text(item.artist ?? '—');
    return Row(
      children: [
        for (var i = 0; i < artists.length; i++) ...[
          if (i > 0) const Flexible(child: Text(' / ')),
          Flexible(
            flex: 8,
            child: _CatalogMetadataLink(
              text: names[i],
              onTap: onOpen == null
                  ? null
                  : () => onOpen!(artists[i], names[i]),
            ),
          ),
        ],
      ],
    );
  }
}

class _CatalogMetadataLink extends StatefulWidget {
  const _CatalogMetadataLink({required this.text, this.onTap});
  final String text;
  final VoidCallback? onTap;
  @override
  State<_CatalogMetadataLink> createState() => _CatalogMetadataLinkState();
}

class _CatalogMetadataLinkState extends State<_CatalogMetadataLink> {
  bool hovered = false, focused = false;
  @override
  Widget build(BuildContext context) {
    if (widget.onTap == null) return Text(widget.text);
    return Align(
      alignment: Alignment.centerLeft,
      widthFactor: 1,
      child: Semantics(
        link: true,
        child: InkWell(
          onTap: widget.onTap,
          onHover: (value) => setState(() => hovered = value),
          onFocusChange: (value) => setState(() => focused = value),
          overlayColor: const WidgetStatePropertyAll(Colors.transparent),
          splashFactory: NoSplash.splashFactory,
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 8),
            child: Text(
              widget.text,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: hovered || focused
                  ? TextStyle(
                      color: Theme.of(context).colorScheme.primary,
                      decoration: TextDecoration.underline,
                    )
                  : null,
            ),
          ),
        ),
      ),
    );
  }
}

class CatalogCollectionCard extends StatefulWidget {
  const CatalogCollectionCard({
    super.key,
    required this.item,
    required this.onOpen,
  });
  final CatalogItem item;
  final VoidCallback? onOpen;
  @override
  State<CatalogCollectionCard> createState() => _CatalogCollectionCardState();
}

class _CatalogCollectionCardState extends State<CatalogCollectionCard> {
  bool hovered = false, focused = false, pressed = false;
  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final item = widget.item;
      final artist = item.reference.kind == MediaKind.artist;
      final size = constraints.maxWidth;
      final scheme = Theme.of(context).colorScheme;
      final active = widget.onOpen != null && (hovered || focused || pressed);
      final radius = BorderRadius.circular(artist ? size / 2 : 7);
      final subtitle = artist
          ? (item.trackCount == null
                ? ''
                : context.catalogL10n.catalogSongCount(
                    item.trackCount!.toInt(),
                  ))
          : item.artist ?? '';
      return Semantics(
        button: true,
        enabled: widget.onOpen != null,
        child: InkWell(
          onTap: widget.onOpen,
          onHover: (value) => setState(() => hovered = value),
          onFocusChange: (value) => setState(() => focused = value),
          onHighlightChanged: (value) => setState(() => pressed = value),
          // Feedback belongs to the artwork, not the card's text/empty space.
          overlayColor: const WidgetStatePropertyAll(Colors.transparent),
          splashFactory: NoSplash.splashFactory,
          child: Column(
            crossAxisAlignment: artist
                ? CrossAxisAlignment.center
                : CrossAxisAlignment.start,
            children: [
              Stack(
                children: [
                  CatalogArtwork(item: item, size: size),
                  Positioned.fill(
                    child: IgnorePointer(
                      child: AnimatedContainer(
                        key: const ValueKey('catalog-cover-feedback'),
                        duration: MediaQuery.disableAnimationsOf(context)
                            ? Duration.zero
                            : const Duration(milliseconds: 120),
                        decoration: BoxDecoration(
                          borderRadius: radius,
                          color: active
                              ? scheme.primary.withValues(
                                  alpha: pressed ? .12 : .06,
                                )
                              : Colors.transparent,
                          border: Border.all(
                            width: 2,
                            color: active
                                ? scheme.primary.withValues(
                                    alpha: focused ? .85 : .3,
                                  )
                                : Colors.transparent,
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 7),
              Text(
                catalogTitle(context, item),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w500,
                ),
              ),
              if (subtitle.isNotEmpty) ...[
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontSize: 12,
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
                ),
              ],
            ],
          ),
        ),
      );
    },
  );
}

final catalogScrollOffsetsProvider = Provider((ref) => <String, double>{});

/// Both layouts are lazy slivers and keep an independent scroll offset per location.
class CatalogItemsView extends ConsumerStatefulWidget {
  const CatalogItemsView({
    super.key,
    required this.locationKey,
    required this.items,
    required this.itemBuilder,
    required this.footer,
    this.grid = false,
    this.itemExtent,
  });
  final String locationKey;
  final List<CatalogItem> items;
  final Widget Function(CatalogItem, int) itemBuilder;
  final Widget footer;
  final bool grid;
  final double? itemExtent;
  @override
  ConsumerState<CatalogItemsView> createState() => _CatalogItemsViewState();
}

class _CatalogItemsViewState extends ConsumerState<CatalogItemsView> {
  late final ScrollController scroll;
  final _deferArtwork = ValueNotifier(false);
  final _scrollClock = Stopwatch()..start();
  Timer? _settleTimer;
  double _lastOffset = 0;
  int _lastScrollMicros = 0;
  @override
  void initState() {
    super.initState();
    final offsets = ref.read(catalogScrollOffsetsProvider);
    scroll = ScrollController(
      initialScrollOffset: offsets[widget.locationKey] ?? 0,
    );
    _lastOffset = scroll.initialScrollOffset;
    scroll.addListener(() {
      // Run before laying out the new viewport, including scrollbar jumps.
      final now = _scrollClock.elapsedMicroseconds;
      final delta = (scroll.offset - _lastOffset).abs();
      final elapsed = now - _lastScrollMicros;
      final fast =
          delta > scroll.position.viewportDimension * .45 ||
          (delta > 48 && elapsed > 0 && delta * 1000 / elapsed > 6.5);
      _lastOffset = scroll.offset;
      _lastScrollMicros = now;
      if (fast) _deferArtwork.value = true;
      if (_deferArtwork.value) {
        _settleTimer?.cancel();
        _settleTimer = Timer(const Duration(milliseconds: 240), () {
          _deferArtwork.value = false;
        });
      }
      offsets.remove(widget.locationKey);
      offsets[widget.locationKey] = scroll.offset;
      while (offsets.length > 128) {
        offsets.remove(offsets.keys.first);
      }
    });
  }

  @override
  void dispose() {
    _settleTimer?.cancel();
    scroll.dispose();
    _deferArtwork.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      // Fill each column. Window height must not shrink covers inside wide cells.
      const gap = 18.0;
      final available = (constraints.maxWidth - 32).clamp(1.0, double.infinity);
      final count = ((available + gap) / (160 + gap)).ceil();
      final width = (available - (count - 1) * gap) / count;
      final scale = MediaQuery.textScalerOf(context).scale(1);
      final delegate = SliverChildBuilderDelegate(
        (_, i) => widget.itemBuilder(widget.items[i], i),
        childCount: widget.items.length,
      );
      return _CatalogScrollActivity(
        notifier: _deferArtwork,
        child: Scrollbar(
          controller: scroll,
          child: CustomScrollView(
            key: const ValueKey('catalog-items-scroll'),
            controller: scroll,
            scrollCacheExtent: const ScrollCacheExtent.pixels(200),
            slivers: [
              if (widget.grid)
                SliverPadding(
                  padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
                  sliver: SliverGrid(
                    gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                      crossAxisCount: count,
                      crossAxisSpacing: gap,
                      mainAxisSpacing: 12,
                      mainAxisExtent: width + 50 * scale,
                    ),
                    delegate: delegate,
                  ),
                )
              else
                SliverPadding(
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  sliver: widget.itemExtent == null
                      ? SliverList(delegate: delegate)
                      : SliverFixedExtentList(
                          itemExtent: widget.itemExtent!,
                          delegate: delegate,
                        ),
                ),
              SliverToBoxAdapter(child: widget.footer),
            ],
          ),
        ),
      );
    },
  );
}

/// Only artwork depends on this signal; scrolling does not rebuild the library.
class _CatalogScrollActivity extends InheritedNotifier<ValueNotifier<bool>> {
  const _CatalogScrollActivity({required super.notifier, required super.child});

  static ValueNotifier<bool>? maybeOf(BuildContext context) => context
      .dependOnInheritedWidgetOfExactType<_CatalogScrollActivity>()
      ?.notifier;
}
