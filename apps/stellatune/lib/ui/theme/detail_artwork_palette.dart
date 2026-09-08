import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:palette_generator/palette_generator.dart';
import 'package:stellatune/player/queue_models.dart';

typedef ArtworkRequest = ({
  String trackKey,
  String? localPath,
  QueueCoverKind? kind,
  String? value,
});

/// The original detail-page palette: four most populous colors, without HSL
/// filtering or desaturation. Fixed home themes do not use this palette.
class DetailArtworkPalette {
  const DetailArtworkPalette({required this.colors, required this.foreground});

  final List<Color> colors;
  final Color foreground;

  static const neutral = DetailArtworkPalette(
    colors: [
      Color(0xFF212121),
      Color(0xFF424242),
      Colors.black,
      Color(0xFF263238),
    ],
    foreground: Colors.white,
  );

  factory DetailArtworkPalette.fromSwatches(
    Iterable<PaletteColor> swatches, {
    Color? dominant,
  }) {
    final sorted = [...swatches]
      ..sort((a, b) => b.population.compareTo(a.population));
    final dominantColor = dominant ?? Colors.blueGrey;
    return DetailArtworkPalette(
      colors: List<Color>.unmodifiable([
        for (var index = 0; index < 4; index++)
          index < sorted.length
              ? sorted[index].color
              : index == 0
              ? dominantColor
              : Colors.black,
      ]),
      foreground: dominantColor.computeLuminance() > .5
          ? Colors.black
          : Colors.white,
    );
  }
}

Future<DetailArtworkPalette> detailPaletteFromImage(ImageProvider image) async {
  final palette = await PaletteGenerator.fromImageProvider(
    ResizeImage(image, width: 100, height: 100),
    maximumColorCount: 24,
  ).timeout(const Duration(seconds: 8));
  return DetailArtworkPalette.fromSwatches(
    palette.paletteColors,
    dominant: palette.dominantColor?.color,
  );
}

Future<DetailArtworkPalette> loadDetailArtworkPalette(
  ArtworkRequest request,
) async {
  Object? localError;
  if (request.localPath != null) {
    try {
      final file = File(request.localPath!);
      if (!await file.exists()) {
        throw FileSystemException('Cover file not found', request.localPath);
      }
      return await detailPaletteFromImage(FileImage(file));
    } catch (error) {
      localError = error;
    }
  }

  final value = request.value;
  final ImageProvider? image = value == null || value.isEmpty
      ? null
      : switch (request.kind) {
          QueueCoverKind.file => FileImage(File(value)),
          QueueCoverKind.url => NetworkImage(value),
          QueueCoverKind.data => MemoryImage(
            value.startsWith('data:')
                ? Uri.parse(value).data!.contentAsBytes()
                : base64Decode(value),
          ),
          null => null,
        };
  if (image != null) return detailPaletteFromImage(image);
  // Missing/corrupt files are retryable, unlike a track with no artwork at all.
  if (localError != null) throw localError;
  return DetailArtworkPalette.neutral;
}

/// Keeps only a small set of successful/in-flight extractions. Failures are
/// evicted, so reopening details or invalidating the provider retries that cover.
class DetailArtworkPaletteCache {
  DetailArtworkPaletteCache(this.loader);
  final Future<DetailArtworkPalette> Function(ArtworkRequest) loader;
  final _entries = <ArtworkRequest, Future<DetailArtworkPalette>>{};
  static const _capacity = 32;

  Future<DetailArtworkPalette> load(ArtworkRequest request) {
    final cached = _entries.remove(request);
    if (cached != null) {
      _entries[request] = cached;
      return cached;
    }
    late final Future<DetailArtworkPalette> pending;
    pending = Future<DetailArtworkPalette>.sync(() => loader(request))
        .catchError((Object error, StackTrace stack) {
          if (identical(_entries[request], pending)) _entries.remove(request);
          Error.throwWithStackTrace(error, stack);
        });
    _entries[request] = pending;
    if (_entries.length > _capacity) _entries.remove(_entries.keys.first);
    return pending;
  }
}
