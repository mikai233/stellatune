import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/ui/theme/detail_artwork_palette.dart';

export 'detail_artwork_palette.dart';

final currentArtworkRequestProvider = Provider<ArtworkRequest?>((ref) {
  final source = ref.watch(
    queueControllerProvider.select((queue) {
      final item = queue.currentItem;
      return item == null
          ? null
          : (
              key: item.stableTrackKey,
              id: item.coverId,
              kind: item.cover?.kind,
              value: item.cover?.value,
            );
    }),
  );
  if (source == null) return null;
  final dir = ref.watch(coverDirProvider);
  return (
    trackKey: source.key,
    localPath: source.id == null || dir.isEmpty ? null : '$dir/${source.id}',
    kind: source.kind,
    value: source.value,
  );
});

final artworkPaletteLoaderProvider =
    Provider<Future<DetailArtworkPalette> Function(ArtworkRequest)>(
      (_) => loadDetailArtworkPalette,
    );

final detailArtworkPaletteCacheProvider = Provider<DetailArtworkPaletteCache>(
  (ref) => DetailArtworkPaletteCache(ref.watch(artworkPaletteLoaderProvider)),
);

/// Both detail pages consume this provider. Riverpod discards superseded
/// requests; cached successes can be reused on reopening.
final currentArtworkPaletteProvider =
    FutureProvider.autoDispose<DetailArtworkPalette>((ref) {
      final request = ref.watch(currentArtworkRequestProvider);
      if (request == null) return DetailArtworkPalette.neutral;
      return ref.watch(detailArtworkPaletteCacheProvider).load(request);
    }, retry: (_, _) => null);
