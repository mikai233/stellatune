import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'catalog_bridge.dart';

String trackFormat(CatalogItem item) {
  final format = item.audio?.format.trim().toUpperCase();
  final path = item.localPath?.replaceAll('\\', '/');
  final name = path?.split('/').last ?? '';
  final fallback = name.contains('.') ? name.split('.').last.toUpperCase() : '';
  final value = format?.isNotEmpty == true ? format! : fallback;
  return [if (value.isNotEmpty) value, if (item.isSegment) 'CUE'].join(' · ');
}

bool isLossyAudio(CatalogAudioInfo audio) {
  final codec = audio.codec?.toLowerCase();
  if (codec != null && codec.isNotEmpty) {
    return {
      'mp1',
      'mp2',
      'mp3',
      'aac',
      'vorbis',
      'opus',
      'speex',
      'wma',
    }.contains(codec);
  }
  return {
    'MP1',
    'MP2',
    'MP3',
    'AAC',
    'OPUS',
    'OGG',
    'OGA',
    'WMA',
  }.contains(audio.format.toUpperCase());
}

String audioSpecification(
  CatalogItem item, {
  bool includeBitrate = true,
  bool chinese = false,
}) {
  final audio = item.audio;
  return [
    trackFormat(item),
    if (audio?.format.toUpperCase() == 'NCM' && audio?.codec != null)
      audio!.codec!.toUpperCase(),
    if (audio?.bitsPerSample != null && !isLossyAudio(audio!))
      '${audio.bitsPerSample}-bit${audio.floatingPoint ? ' float' : ''}',
    if (audio?.sampleRate != null) '${audio!.sampleRate! / 1000} kHz',
    if (includeBitrate && audio?.bitrate != null)
      audioBitRate(audio!, chinese: chinese),
  ].where((s) => s.isNotEmpty).join(' · ');
}

String audioBitRate(CatalogAudioInfo audio, {bool chinese = false}) {
  final rate = audio.bitrate;
  if (rate == null || rate.bps <= 0) return chinese ? '未知' : 'Unknown';
  final mode = rate.mode == null ? '' : ' · ${rate.mode!.name.toUpperCase()}';
  final prefix = rate.estimated ? (chinese ? '约 ' : '≈ ') : '';
  final kbps = rate.kind == BitrateKind.fixed && !rate.estimated
      ? (rate.bps / 1000).toStringAsFixed(3).replaceFirst(RegExp(r'\.?0+$'), '')
      : '${(rate.bps / 1000).round()}';
  return '$prefix$kbps kbps$mode';
}

class AlbumSource {
  AlbumSource(this.id, this.directory, this.items);
  final String id, directory;
  final List<CatalogItem> items;
  String locationLabel(List<AlbumSource> sources) {
    final peers = sources.where(
      (s) => trackFormat(s.items.first) == trackFormat(items.first),
    );
    final parts = directory.split('/');
    for (var depth = 1; depth <= parts.length; depth++) {
      final suffix = parts.skip(parts.length - depth).join('/');
      if (peers
              .where(
                (s) =>
                    s.directory == suffix || s.directory.endsWith('/$suffix'),
              )
              .length ==
          1) {
        return suffix;
      }
    }
    final cue = items.first.audio?.cuePath
        ?.replaceAll('\\', '/')
        .split('/')
        .last;
    return [directory, ?cue].join(' / ');
  }

  String specification({required bool chinese}) {
    final specs = items
        .map((item) => audioSpecification(item, includeBitrate: false))
        .where((s) => s.isNotEmpty)
        .toSet();
    if (specs.length == 1) return specs.single;
    final formats = items.map(trackFormat).where((s) => s.isNotEmpty).toSet();
    return '${formats.join(' / ')} · ${chinese ? '混合规格' : 'Mixed specifications'}';
  }
}

/// Conservative source grouping, not recording deduplication. CUE documents
/// retain their identity, including multi-FILE sheets. Separate copies in other
/// directories never collapse. Only explicit CD/Disc folders with matching tags
/// may combine into a multi-disc source.
List<AlbumSource> albumSources(List<CatalogItem> items) {
  if (items.every((item) => item.audio == null)) return [];
  final groups = <String, AlbumSource>{};
  final discFolder = RegExp(
    r'^(?:cd|disc|disk)[ _.-]*0*(\d+)$',
    caseSensitive: false,
  );
  final discOrigins = <String, Map<int, Set<String>>>{};
  String discGroup(String directory, CatalogItem item) => jsonEncode([
    directory.substring(0, directory.lastIndexOf('/')),
    item.audio?.cuePath != null ? 'CUE' : trackFormat(item),
  ]);
  for (final item in items) {
    final audio = item.audio;
    final cue = audio?.cuePath?.replaceAll('\\', '/');
    final directory = cue == null
        ? audio?.sourceDirectory ?? ''
        : cue.substring(0, cue.lastIndexOf('/'));
    final match = discFolder.firstMatch(directory.split('/').last);
    final disc = audio?.discNumber?.toInt();
    if (match == null ||
        !directory.contains('/') ||
        disc == null ||
        int.tryParse(match.group(1)!) != disc) {
      continue;
    }
    ((discOrigins[discGroup(directory, item)] ??= {})[disc] ??= {}).add(
      directory,
    );
  }
  for (final item in items) {
    final audio = item.audio;
    final path = (item.localPath ?? '').replaceAll('\\', '/');
    final cue = audio?.cuePath?.replaceAll('\\', '/');
    var directory =
        audio?.sourceDirectory ??
        (path.contains('/') ? path.substring(0, path.lastIndexOf('/')) : '');
    if (cue != null) directory = cue.substring(0, cue.lastIndexOf('/'));
    final disc = discFolder.firstMatch(directory.split('/').last);
    final multiDisc =
        disc != null &&
        int.tryParse(disc.group(1)!) == audio?.discNumber?.toInt();
    var cueKey = cue;
    final ambiguous =
        directory.contains('/') &&
        (discOrigins[discGroup(directory, item)]?.values.any(
              (dirs) => dirs.length > 1,
            ) ??
            false);
    if (multiDisc && !ambiguous && directory.contains('/')) {
      directory = directory.substring(0, directory.lastIndexOf('/'));
      // Keep distinct rips apart even when their disc folders are siblings.
      cueKey = cue
          ?.split('/')
          .last
          .replaceAll(
            RegExp(r'(?:cd|disc|disk)[ _.-]*\d+', caseSensitive: false),
            'disc',
          );
    }
    final id = jsonEncode([
      directory,
      cue != null ? 'CUE' : trackFormat(item),
      cueKey,
      if (directory.isEmpty) item.reference.id,
    ]);
    (groups[id] ??= AlbumSource(id, directory, [])).items.add(item);
  }
  final result = groups.values.toList()
    ..sort((a, b) {
      final count = b.items.length.compareTo(a.items.length);
      return count != 0 ? count : a.id.compareTo(b.id);
    });
  return result;
}

final albumSourcePreferencesProvider =
    NotifierProvider<AlbumSourcePreferences, Map<String, String>>(
      AlbumSourcePreferences.new,
    );

class AlbumSourcePreferences extends Notifier<Map<String, String>> {
  AlbumSourcePreferences([this.store]);
  final SettingsStore? store;
  @override
  Map<String, String> build() => store?.albumSources ?? const {};
  Future<void> select(String album, String source) async {
    final next = {...state}..remove(album);
    next[album] = source;
    while (next.length > 512) {
      next.remove(next.keys.first);
    }
    state = Map.unmodifiable(next);
    try {
      await store?.setAlbumSources(next);
    } catch (error, stack) {
      DiagnosticsService.instance.report(
        error,
        stack: stack,
        operation: 'settings',
      );
    }
  }
}
