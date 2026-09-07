import 'home_view_data.dart';

/// Temporary editorial content for the visual design pass. Never register these
/// entries as playable tracks or persist their counts in the media library.
abstract final class HomePlaceholders {
  static const root = 'assets/images/home/';
  static const hero = '${root}hero.png';
  static const flowers = '${root}flowers.png';
  static const artwork = [
    '${root}portrait.png',
    '${root}moon.png',
    '${root}anime.png',
    '${root}palms.png',
    '${root}forest.png',
    hero,
  ];

  static const listening = [
    HomeCardData(
      title: '开始懂了',
      subtitle: '孙燕姿',
      artwork: '${root}portrait.png',
    ),
    HomeCardData(
      title: 'The Moment',
      subtitle: 'Novo Amor',
      artwork: '${root}moon.png',
    ),
    HomeCardData(
      title: '夜に駆ける',
      subtitle: 'YOASOBI',
      artwork: '${root}anime.png',
    ),
    HomeCardData(
      title: 'Happiness',
      subtitle: 'Various Artists',
      artwork: '${root}palms.png',
    ),
    HomeCardData(
      title: 'For A Better Day',
      subtitle: 'Kodaline',
      artwork: '${root}forest.png',
    ),
    HomeCardData(title: '像风一样', subtitle: '薛之谦', artwork: hero),
  ];
  static const recent = [
    HomeCardData(
      title: 'Sketches',
      subtitle: 'Nujabes',
      artwork: '${root}forest.png',
    ),
    HomeCardData(
      title: 'The Deep End',
      subtitle: 'AURORA',
      artwork: '${root}moon.png',
    ),
    HomeCardData(
      title: '与你',
      subtitle: 'rubur',
      artwork: '${root}portrait.png',
    ),
    HomeCardData(
      title: '四季',
      subtitle: 'bad snacks',
      artwork: '${root}palms.png',
    ),
    HomeCardData(
      title: 'Flow',
      subtitle: 'Cigarettes After Sex',
      artwork: hero,
    ),
    HomeCardData(title: '夢の続き', subtitle: 'eill', artwork: '${root}anime.png'),
  ];
  static const artists = [
    HomeCardData(
      title: '孙燕姿',
      subtitle: '328 首',
      artwork: '${root}portrait.png',
    ),
    HomeCardData(
      title: 'YOASOBI',
      subtitle: '156 首',
      artwork: '${root}anime.png',
    ),
    HomeCardData(
      title: 'AURORA',
      subtitle: '112 首',
      artwork: '${root}flowers.png',
    ),
    HomeCardData(title: '陈绮贞', subtitle: '98 首', artwork: hero),
    HomeCardData(title: '周杰伦', subtitle: '320 首', artwork: '${root}moon.png'),
  ];
  static const data = HomeViewData(
    continueListening: listening,
    recentlyAdded: recent,
  );
}
