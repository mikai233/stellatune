import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:flutter/material.dart';

import 'home_view_data.dart';
import 'widgets/home_hero.dart';
import 'widgets/home_track_section.dart';
import 'widgets/home_discovery_sections.dart';

class HomeView extends StatelessWidget {
  const HomeView({
    super.key,
    required this.data,
    required this.onResume,
    required this.onContinue,
    required this.onRecent,
    required this.onOpenLibrary,
    required this.onOpenPlaceholder,
    this.greeting = '晚上好',
    this.subtitle = '愿音乐，陪你度过每一个平凡夜晚。',
  });
  final HomeViewData data;
  final VoidCallback onResume, onOpenLibrary, onOpenPlaceholder;
  final ValueChanged<int> onContinue, onRecent;
  final String greeting;
  final String subtitle;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final wide = constraints.maxWidth >= 1050;
      final listening = HomeTrackSection(
        title: '继续聆听',
        items: data.continueListening,
        onTap: onContinue,
        onMore: onOpenLibrary,
      );
      final recent = HomeTrackSection(
        title: '最近添加',
        items: data.recentlyAdded,
        onTap: onRecent,
        onMore: onOpenLibrary,
        compact: true,
      );
      return SingleChildScrollView(
        key: const ValueKey('home-scroll'),
        padding: const EdgeInsets.fromLTRB(20, 12, 18, 18),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.only(left: 30),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    greeting,
                    style: TextStyle(
                      fontSize: 36,
                      height: 1.25,
                      fontWeight: FontWeight.w600,
                      color: ArtworkPalette.of(context).onBackdrop,
                      letterSpacing: 1.5,
                    ),
                  ),
                  SizedBox(height: 7),
                  Text(
                    subtitle,
                    style: TextStyle(
                      fontSize: 14,
                      color: ArtworkPalette.of(context).onBackdrop
                          .withValues(alpha: .85),
                      letterSpacing: 1,
                    ),
                  ),
                ],
              ),
            ),
            SizedBox(height: 25),
            HomeHero(onResume: onResume),
            SizedBox(height: 17),
            if (wide)
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(flex: 4, child: listening),
                  SizedBox(width: 24),
                  Expanded(
                    child: HomeShortcuts(onOpen: (_) => onOpenPlaceholder()),
                  ),
                ],
              )
            else
              listening,
            SizedBox(height: 17),
            if (wide)
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(flex: 11, child: recent),
                  SizedBox(width: 30),
                  Expanded(
                    flex: 10,
                    child: HomeArtists(onOpen: onOpenPlaceholder),
                  ),
                ],
              )
            else ...[
              recent,
              SizedBox(height: 20),
              HomeArtists(onOpen: onOpenPlaceholder),
            ],
            SizedBox(height: 16),
            if (constraints.maxWidth >= 800)
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(child: HomeMoods(onOpen: onOpenPlaceholder)),
                  SizedBox(width: 34),
                  Expanded(child: HomeMoments(onOpen: onOpenPlaceholder)),
                ],
              )
            else ...[
              HomeMoods(onOpen: onOpenPlaceholder),
              SizedBox(height: 18),
              HomeMoments(onOpen: onOpenPlaceholder),
            ],
          ],
        ),
      );
    },
  );
}
