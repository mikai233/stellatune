import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:flutter/material.dart';

import '../home_placeholders.dart';
import 'home_artwork.dart';
import 'home_track_section.dart';

class HomeShortcuts extends StatelessWidget {
  const HomeShortcuts({super.key, required this.onOpen});
  final ValueChanged<int> onOpen;
  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
    decoration: BoxDecoration(
      color: ArtworkPalette.of(context).onBackdrop.withValues(alpha: .065),
      border: Border.all(
        color: ArtworkPalette.of(context).onBackdrop.withValues(alpha: .14),
      ),
      borderRadius: BorderRadius.circular(16),
    ),
    child: Column(
      children: [
        for (var i = 0; i < 4; i++)
          Material(
            type: MaterialType.transparency,
            child: InkWell(
              onTap: () => onOpen(i),
              borderRadius: BorderRadius.circular(10),
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 6),
                child: Row(
                  children: [
                    Container(
                      width: 38,
                      height: 38,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: ArtworkPalette.of(context).onBackdrop
                            .withValues(alpha: .09),
                      ),
                      child: Icon(
                        [
                          Icons.favorite_border_rounded,
                          Icons.history_rounded,
                          Icons.add_rounded,
                          Icons.star_border_rounded,
                        ][i],
                        color: ArtworkPalette.of(context).onBackdrop,
                        size: 22,
                      ),
                    ),
                    SizedBox(width: 13),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            ['我喜欢的音乐', '最近播放', '最近添加', '高评分'][i],
                            style: TextStyle(
                              color: ArtworkPalette.of(context).onBackdrop,
                              fontSize: 13,
                            ),
                          ),
                          Text(
                            ['327 首', '50 首', '168 首', '86 首'][i],
                            style: TextStyle(
                              color: ArtworkPalette.of(context).onBackdrop
                                  .withValues(alpha: .60),
                              fontSize: 11,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
      ],
    ),
  );
}

class HomeArtists extends StatelessWidget {
  const HomeArtists({super.key, required this.onOpen});
  final VoidCallback onOpen;
  @override
  Widget build(BuildContext context) => Column(
    children: [
      HomeSectionTitle(title: '常听的艺术家', onMore: onOpen),
      SizedBox(height: 6),
      LayoutBuilder(
        builder: (context, constraints) {
          final count = (constraints.maxWidth / 96).floor().clamp(2, 5);
          return Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (var i = 0; i < count; i++) ...[
                if (i > 0) SizedBox(width: 16),
                Expanded(
                  child: Material(
                    type: MaterialType.transparency,
                    child: InkWell(
                      onTap: onOpen,
                      borderRadius: BorderRadius.circular(12),
                      child: Column(
                        children: [
                          SizedBox(
                            width: 92,
                            height: 92,
                            child: ClipOval(
                              child: HomeArtwork(
                                asset: HomePlaceholders.artists[i].artwork,
                              ),
                            ),
                          ),
                          SizedBox(height: 8),
                          Text(
                            HomePlaceholders.artists[i].title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              fontSize: 12,
                              color: ArtworkPalette.of(context).onBackdrop,
                            ),
                          ),
                          Text(
                            HomePlaceholders.artists[i].subtitle,
                            style: TextStyle(
                              fontSize: 10,
                              color: ArtworkPalette.of(context).onBackdrop
                                  .withValues(alpha: .60),
                            ),
                          ),
                        ],
                      ),
                    ),
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

class HomeMoods extends StatelessWidget {
  const HomeMoods({super.key, required this.onOpen});
  final VoidCallback onOpen;
  @override
  Widget build(BuildContext context) => Column(
    children: [
      const HomeSectionTitle(title: '心情歌单', showMore: false),
      const SizedBox(height: 6),
      SizedBox(
        height: 91,
        child: Row(
          children: [
            for (var i = 0; i < 6; i++) ...[
              if (i > 0) const SizedBox(width: 10),
              Expanded(
                child: _DiscoveryCard(
                  title: ['专注', '放松', '夜晚', '治愈', '通勤', '清晨'][i],
                  subtitle: [
                    'Focus',
                    'Relax',
                    'Night',
                    'Heal',
                    'Commute',
                    'Morning',
                  ][i],
                  artwork: [
                    HomePlaceholders.artwork[4],
                    HomePlaceholders.artwork[1],
                    HomePlaceholders.artwork[1],
                    HomePlaceholders.artwork[4],
                    HomePlaceholders.hero,
                    HomePlaceholders.flowers,
                  ][i],
                  tint: [
                    const Color(0x802C4835),
                    const Color(0x8048738B),
                    const Color(0x8023315F),
                    const Color(0x807C9678),
                    const Color(0x805A5148),
                    const Color(0x809D784F),
                  ][i],
                  onTap: onOpen,
                  icon: [
                    Icons.headphones,
                    Icons.air,
                    Icons.nightlight_round,
                    Icons.spa_outlined,
                    Icons.train_outlined,
                    Icons.wb_sunny_outlined,
                  ][i],
                ),
              ),
            ],
          ],
        ),
      ),
    ],
  );
}

class HomeMoments extends StatelessWidget {
  const HomeMoments({super.key, required this.onOpen});
  final VoidCallback onOpen;
  @override
  Widget build(BuildContext context) => Column(
    children: [
      const HomeSectionTitle(title: '音乐时刻', showMore: false),
      const SizedBox(height: 6),
      SizedBox(
        height: 91,
        child: Row(
          children: [
            for (var i = 0; i < 4; i++) ...[
              if (i > 0) const SizedBox(width: 10),
              Expanded(
                child: _DiscoveryCard(
                  title: ['深夜电台', '雨天', '工作', '周末'][i],
                  subtitle: [
                    '在安静的夜里，听见自己',
                    '让雨声和音乐融在一起',
                    '保持专注，进入心流',
                    '把生活调成喜欢的节奏',
                  ][i],
                  artwork: [
                    HomePlaceholders.hero,
                    HomePlaceholders.flowers,
                    HomePlaceholders.artwork[2],
                    HomePlaceholders.artwork[4],
                  ][i],
                  onTap: onOpen,
                ),
              ),
            ],
          ],
        ),
      ),
    ],
  );
}

class _DiscoveryCard extends StatelessWidget {
  const _DiscoveryCard({
    required this.title,
    required this.subtitle,
    required this.artwork,
    required this.onTap,
    this.tint = Colors.transparent,
    this.icon,
  });
  final String title, subtitle, artwork;
  final Color tint;
  final IconData? icon;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => ClipRRect(
    borderRadius: BorderRadius.circular(8),
    child: Stack(
      fit: StackFit.expand,
      children: [
        HomeArtwork(asset: artwork),
        ColoredBox(color: tint),
        const DecoratedBox(
          decoration: BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment.topCenter,
              end: Alignment.bottomCenter,
              colors: [Color(0x05000000), Color(0xA6000000)],
            ),
          ),
        ),
        if (icon != null)
          Align(
            alignment: const Alignment(0, -.55),
            child: Icon(icon, color: Colors.white54, size: 20),
          ),
        Positioned(
          left: 10,
          right: 6,
          bottom: 9,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  color: Colors.white,
                  fontSize: 14,
                  fontWeight: FontWeight.w500,
                ),
              ),
              Text(
                subtitle,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(color: Colors.white70, fontSize: 10),
              ),
            ],
          ),
        ),
        Material(
          type: MaterialType.transparency,
          child: InkWell(onTap: onTap),
        ),
      ],
    ),
  );
}
