import 'dart:ui' show ImageFilter, TileMode;

import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:flutter/material.dart';

import '../home_view_data.dart';
import 'home_artwork.dart';

class HomeSectionTitle extends StatelessWidget {
  const HomeSectionTitle({
    super.key,
    required this.title,
    this.onMore,
    this.showMore = true,
  });
  final String title;
  final VoidCallback? onMore;
  final bool showMore;
  @override
  Widget build(BuildContext context) => SizedBox(
    height: 32,
    child: Row(
      children: [
        Text(
          title,
          style: TextStyle(
            fontSize: 17,
            fontWeight: FontWeight.w600,
            color: ArtworkPalette.of(context).onBackdrop,
            letterSpacing: .2,
          ),
        ),
        SizedBox(width: 4),
        Icon(
          Icons.chevron_right,
          size: 17,
          color: ArtworkPalette.of(context).onBackdrop.withValues(alpha: .70),
        ),
        const Spacer(),
        if (showMore)
          TextButton(
            onPressed: onMore,
            style: TextButton.styleFrom(
              foregroundColor: ArtworkPalette.of(context).onBackdrop
                  .withValues(alpha: .70),
              textStyle: TextStyle(fontSize: 11, fontFamily: 'NotoSansSC'),
              padding: EdgeInsets.zero,
              minimumSize: const Size(66, 28),
            ),
            child: Row(
              children: [
                Text('查看全部'),
                SizedBox(width: 3),
                Icon(Icons.chevron_right, size: 14),
              ],
            ),
          ),
      ],
    ),
  );
}

class HomeTrackSection extends StatelessWidget {
  const HomeTrackSection({
    super.key,
    required this.title,
    required this.items,
    required this.onTap,
    required this.onMore,
    this.compact = false,
  });
  final String title;
  final List<HomeCardData> items;
  final ValueChanged<int> onTap;
  final VoidCallback onMore;
  final bool compact;
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      HomeSectionTitle(title: title, onMore: onMore),
      SizedBox(height: 6),
      LayoutBuilder(
        builder: (context, constraints) {
          final count = (constraints.maxWidth / (compact ? 95 : 135))
              .floor()
              .clamp(1, 6);
          final visible = items.take(count).toList();
          if (visible.isEmpty) {
            return SizedBox(
              height: 130,
              child: Center(
                child: Text(
                  '还没有音乐，去发现喜欢的声音吧',
                  style: TextStyle(
                    color: ArtworkPalette.of(context).onBackdrop
                        .withValues(alpha: .70),
                  ),
                ),
              ),
            );
          }
          return Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (var i = 0; i < visible.length; i++) ...[
                if (i > 0) SizedBox(width: 12),
                Expanded(
                  child: HomeMusicCard(
                    key: ValueKey('$title-$i'),
                    data: visible[i],
                    compact: compact,
                    onTap: () => onTap(i),
                  ),
                ),
              ],
            ],
          );
        },
      ),
    ],
  );
}

class HomeMusicCard extends StatefulWidget {
  const HomeMusicCard({
    super.key,
    required this.data,
    required this.onTap,
    this.compact = false,
  });
  final HomeCardData data;
  final VoidCallback onTap;
  final bool compact;
  @override
  State<HomeMusicCard> createState() => _HomeMusicCardState();
}

class _HomeMusicCardState extends State<HomeMusicCard> {
  bool hovered = false;

  @override
  Widget build(BuildContext context) {
    final radius = BorderRadius.circular(12);
    return MergeSemantics(
      child: RepaintBoundary(
        child: MouseRegion(
          onEnter: (_) => setState(() => hovered = true),
          onExit: (_) => setState(() => hovered = false),
          cursor: SystemMouseCursors.click,
          child: AspectRatio(
            aspectRatio: 1,
            child: ClipRRect(
              borderRadius: radius,
              child: Stack(
                fit: StackFit.expand,
                children: [
                  HomeArtwork(
                    asset: widget.data.artwork,
                    filePath: widget.data.coverPath,
                  ),
                  // Blend a lightly blurred copy of the same full-size cover.
                  // The mask feathers the blur itself, not just its tint.
                  ExcludeSemantics(
                    child: ShaderMask(
                      blendMode: BlendMode.dstIn,
                      shaderCallback: (bounds) => const LinearGradient(
                        begin: Alignment.topCenter,
                        end: Alignment.bottomCenter,
                        stops: [0, .45, .82, 1],
                        colors: [
                          Colors.transparent,
                          Colors.transparent,
                          Colors.white,
                          Colors.white,
                        ],
                      ).createShader(bounds),
                      child: ImageFiltered(
                        imageFilter: ImageFilter.blur(
                          sigmaX: 3,
                          sigmaY: 3,
                          tileMode: TileMode.clamp,
                        ),
                        child: HomeArtwork(
                          asset: widget.data.artwork,
                          filePath: widget.data.coverPath,
                        ),
                      ),
                    ),
                  ),
                  const IgnorePointer(
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        gradient: LinearGradient(
                          begin: Alignment.topCenter,
                          end: Alignment.bottomCenter,
                          stops: [0, .45, 1],
                          colors: [
                            Colors.transparent,
                            Colors.transparent,
                            Color(0x38000000),
                          ],
                        ),
                      ),
                    ),
                  ),
                  Align(
                    alignment: Alignment.bottomCenter,
                    child: Padding(
                      padding: EdgeInsets.symmetric(
                        horizontal: widget.compact ? 9 : 12,
                        vertical: widget.compact ? 8 : 10,
                      ),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Text(
                            widget.data.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              fontSize: widget.compact ? 11 : 13,
                              height: 1.4,
                              shadows: const [
                                Shadow(
                                  color: Color(0xCC000000),
                                  blurRadius: 4,
                                  offset: Offset(0, 1),
                                ),
                                Shadow(color: Color(0x99000000), blurRadius: 1),
                              ],
                              color: Colors.white,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                          const SizedBox(height: 3),
                          Text(
                            widget.data.subtitle,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              fontSize: widget.compact ? 10 : 11,
                              height: 1.4,
                              shadows: const [
                                Shadow(
                                  color: Color(0xCC000000),
                                  blurRadius: 4,
                                  offset: Offset(0, 1),
                                ),
                                Shadow(color: Color(0x99000000), blurRadius: 1),
                              ],
                              color: const Color(0xE6FFFFFF),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                  IgnorePointer(
                    child: AnimatedContainer(
                      duration: const Duration(milliseconds: 150),
                      decoration: BoxDecoration(
                        borderRadius: radius,
                        border: Border.all(
                          color: Colors.white.withValues(
                            alpha: hovered ? .50 : .12,
                          ),
                        ),
                      ),
                    ),
                  ),
                  Material(
                    type: MaterialType.transparency,
                    child: InkWell(
                      onTap: widget.onTap,
                      borderRadius: radius,
                      hoverColor: Colors.white.withValues(alpha: .04),
                      focusColor: Colors.white.withValues(alpha: .14),
                      splashColor: Colors.white.withValues(alpha: .12),
                      child: const SizedBox.expand(),
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
