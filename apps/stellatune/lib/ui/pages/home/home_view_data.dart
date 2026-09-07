/// Presentation data shared by the live homepage and deterministic previews.
class HomeCardData {
  const HomeCardData({
    required this.title,
    required this.subtitle,
    required this.artwork,
    this.coverPath,
  });
  final String title;
  final String subtitle;
  final String artwork;
  final String? coverPath;
}

class HomeViewData {
  const HomeViewData({
    required this.continueListening,
    required this.recentlyAdded,
  });
  final List<HomeCardData> continueListening;
  final List<HomeCardData> recentlyAdded;
}
