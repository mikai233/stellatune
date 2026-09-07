import 'dart:convert';
import 'dart:io';

import 'package:flutter/painting.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:palette_generator/palette_generator.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

import 'artwork_palette.dart';

typedef ArtworkRequest = ({
  String trackKey,
  String? localPath,
  QueueCoverKind? kind,
  String? value,
});

final currentArtworkRequestProvider = Provider<ArtworkRequest?>((ref) {
  final source = ref.watch(
    queueControllerProvider.select((queue) {
      final item = queue.currentItem;
      return item == null
          ? null
          : (
              key: item.stableTrackKey,
              id: item.id,
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
    Provider<Future<ArtworkPalette> Function(ArtworkRequest)>(
      (_) => loadArtworkPalette,
    );

/// Used by playback details only. Riverpod discards results from superseded
/// dependencies, so a slow cover for track A cannot overwrite track B's palette.
final currentArtworkPaletteProvider =
    FutureProvider.autoDispose<ArtworkPalette>((ref) async {
      final request = ref.watch(currentArtworkRequestProvider);
      if (request == null) return ArtworkPalette.neutral;
      return ref.watch(artworkPaletteLoaderProvider)(request);
    });

Future<ArtworkPalette> paletteFromImage(ImageProvider image) async {
  final result = await PaletteGenerator.fromImageProvider(
    ResizeImage(image, width: 96, height: 96),
    maximumColorCount: 16,
  ).timeout(const Duration(seconds: 8));
  final colors = [...result.paletteColors]
    ..sort((a, b) => b.population.compareTo(a.population));
  final useful = colors.where((entry) {
    final color = HSLColor.fromColor(entry.color);
    return color.saturation > .08 &&
        color.lightness > .08 &&
        color.lightness < .92;
  });
  return ArtworkPalette.fromSeed(
    useful.isEmpty ? result.dominantColor?.color : useful.first.color,
  );
}

Future<ArtworkPalette> loadArtworkPalette(ArtworkRequest request) async {
  if (request.localPath != null) {
    try {
      final file = File(request.localPath!);
      if (await file.exists()) return await paletteFromImage(FileImage(file));
    } catch (_) {
      // A provider cover can still be used if the local cache is absent/corrupt.
    }
  }
  final value = request.value;
  if (value == null || value.isEmpty) return ArtworkPalette.neutral;
  try {
    final ImageProvider? image = switch (request.kind) {
      QueueCoverKind.file => FileImage(File(value)),
      QueueCoverKind.url => NetworkImage(value),
      QueueCoverKind.data => MemoryImage(
        value.startsWith('data:')
            ? Uri.parse(value).data!.contentAsBytes()
            : base64Decode(value),
      ),
      null => null,
    };
    return image == null
        ? ArtworkPalette.neutral
        : await paletteFromImage(image);
  } catch (_) {
    return ArtworkPalette.neutral;
  }
}
