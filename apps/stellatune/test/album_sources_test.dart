import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/library/album_sources.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_controller.dart';
import 'package:stellatune/app/settings_store.dart';

CatalogItem sourceTrack(
  String id,
  String dir, {
  String format = 'FLAC',
  String? cue,
  int? disc,
  int rate = 44100,
  BitrateInfo? bitrate,
  String? codec,
}) => CatalogItem(
  reference: MediaRef(sourceInstanceId: '1', kind: MediaKind.track, id: id),
  title: 'Song $id',
  artistRefs: const [],
  isSegment: cue != null,
  localPath: '$dir/$id.${format.toLowerCase()}',
  audio: CatalogAudioInfo(
    format: format,
    codec: codec,
    bitrate: bitrate,
    floatingPoint: false,
    bitsPerSample: format == 'MP3' ? null : 16,
    sampleRate: rate,
    cuePath: cue,
    discNumber: disc,
    sourceDirectory: dir,
  ),
);

class _Store extends SettingsStore {
  Map<String, String> saved = {};
  @override
  Map<String, String> get albumSources => saved;
  @override
  Future<void> setAlbumSources(Map<String, String> sources) async {
    saved = Map.of(sources);
  }
}

class _Catalog extends CatalogBridge {
  List<CatalogItem> rows = [sourceTrack('1', '/a'), sourceTrack('2', '/b')];
  @override
  Future<List<LibrarySource>> sources() async => [
    const LibrarySource(
      id: '1',
      name: 'Local',
      local: true,
      available: true,
      browseKinds: [MediaKind.track, MediaKind.album],
      searchKinds: [MediaKind.track],
      sorts: [CatalogSort.default_],
    ),
  ];
  @override
  Future<CatalogPage> browse(CatalogQuery query) async {
    expect(
      query.search,
      isEmpty,
      reason: 'Album search must retain all source choices',
    );
    return CatalogPage(items: rows);
  }

  @override
  Future<CatalogItem> detail(MediaRef reference) async => album;
}

const album = CatalogItem(
  reference: MediaRef(
    sourceInstanceId: '1',
    kind: MediaKind.album,
    id: 'album',
  ),
  title: 'Album',
  isSegment: false,
  artistRefs: [],
);

void main() {
  test('different average bitrates do not change source identity or mark mixed specifications', () {
    final tracks = [
      for (final bps in [180000, 256000])
        sourceTrack(
          '$bps',
          '/same',
          format: 'MP3',
          bitrate: BitrateInfo(
            bps: bps,
            kind: BitrateKind.average,
            estimated: true,
          ),
        ),
    ];
    final sources = albumSources(tracks);
    expect(sources, hasLength(1));
    expect(sources.single.specification(chinese: true), 'MP3 · 44.1 kHz');
    expect(
      sources.single.id,
      albumSources([sourceTrack('old', '/same', format: 'MP3')]).single.id,
    );
  });
  test(
    'NCM keeps container identity and displays decoded audio specifications',
    () {
      final flac = sourceTrack(
        '1',
        '/same',
        format: 'NCM',
        codec: 'flac',
        rate: 96000,
      );
      final mp3 = sourceTrack('2', '/same', format: 'NCM', codec: 'mp3');
      expect(audioSpecification(flac), 'NCM · FLAC · 16-bit · 96.0 kHz');
      expect(audioSpecification(mp3), 'NCM · MP3 · 44.1 kHz');
      expect(isLossyAudio(flac.audio!), false);
      expect(isLossyAudio(mp3.audio!), true);
      expect(
        isLossyAudio(
          sourceTrack('3', '/same', format: 'M4A', codec: 'alac').audio!,
        ),
        false,
      );
      expect(
        isLossyAudio(
          sourceTrack('4', '/same', format: 'M4A', codec: 'aac').audio!,
        ),
        true,
      );
    },
  );
  test(
    'bitrate distinguishes CBR from VBR and leaves unavailable values unknown',
    () {
      const cbr = CatalogAudioInfo(
        format: 'MP3',
        floatingPoint: false,
        sourceDirectory: '/music',
        bitrate: BitrateInfo(
          bps: 320000,
          kind: BitrateKind.fixed,
          estimated: false,
          mode: BitrateMode.cbr,
        ),
      );
      const vbr = CatalogAudioInfo(
        format: 'MP3',
        floatingPoint: false,
        sourceDirectory: '/music',
        bitrate: BitrateInfo(
          bps: 192345,
          kind: BitrateKind.average,
          estimated: true,
          mode: BitrateMode.vbr,
        ),
      );
      const unknown = CatalogAudioInfo(
        format: 'MP3',
        floatingPoint: false,
        sourceDirectory: '/music',
      );
      expect(audioBitRate(cbr), '320 kbps · CBR');
      expect(audioBitRate(vbr), '≈ 192 kbps · VBR');
      expect(audioBitRate(vbr, chinese: true), '约 192 kbps · VBR');
      expect(audioBitRate(unknown), 'Unknown');
      expect(
        audioBitRate(
          const CatalogAudioInfo(
            format: 'WAV',
            floatingPoint: false,
            sourceDirectory: '/music',
            bitrate: BitrateInfo(
              bps: 1411200,
              kind: BitrateKind.fixed,
              estimated: false,
              mode: BitrateMode.cbr,
            ),
          ),
        ),
        '1411.2 kbps · CBR',
      );
    },
  );
  final auditPath = Platform.environment['STELLATUNE_ALBUM_AUDIT'];
  test(
    'real Magical Mirai album separates into ten MP3 and ten CUE tracks',
    () {
      final rows = (jsonDecode(File(auditPath!).readAsStringSync()) as List)
          .cast<Map<String, dynamic>>();
      final items = [
        for (final row in rows)
          CatalogItem(
            reference: MediaRef(
              sourceInstanceId: 'local',
              kind: MediaKind.track,
              id: '${row['id']}',
            ),
            title: row['title'] as String,
            isSegment: row['cue_path'] != null,
            localPath: row['path'] as String,
            artistRefs: const [],
            audio: CatalogAudioInfo(
              format: (row['ext'] as String).toUpperCase(),
              floatingPoint: false,
              sourceDirectory: row['dir_norm'] as String,
              cuePath: row['cue_path'] as String?,
              discNumber: row['disc_number'] as int?,
              sampleRate: row['sample_rate'] as int?,
              bitsPerSample: row['pcm_bits'] as int?,
            ),
          ),
      ];
      final sources = albumSources(items);
      expect(sources.length, 2);
      expect(sources.map((s) => s.items.length), [10, 10]);
      expect(sources.map((s) => trackFormat(s.items.first)).toSet(), {
        'MP3',
        'WAV · CUE',
      });
    },
    skip: auditPath == null
        ? 'Set STELLATUNE_ALBUM_AUDIT to a read-only database export'
        : false,
  );
  test('MP3, CUE, and same-format copies remain independent', () {
    final rows = [
      sourceTrack('1', '/a', format: 'MP3'),
      sourceTrack('2', '/a', format: 'WAV', cue: '/a/album.cue'),
      sourceTrack('3', '/a', format: 'WAV', cue: '/a/album.cue'),
      sourceTrack('4', '/b', format: 'MP3'),
    ];
    final groups = albumSources(rows);
    expect(groups.length, 3);
    expect(groups.first.items.map((i) => i.reference.id), ['2', '3']);
    expect(rows.length, 4);
    expect(trackFormat(rows[1]), 'WAV · CUE');
    expect(audioSpecification(rows[1]), 'WAV · CUE · 16-bit · 44.1 kHz');
    expect(audioSpecification(rows[0]), 'MP3 · 44.1 kHz');
  });

  test(
    'multi-FILE CUE remains one source even across formats and directories',
    () {
      final groups = albumSources([
        sourceTrack('1', '/a/files', format: 'WAV', cue: '/a/disc.cue'),
        sourceTrack('2', '/other', cue: '/a/disc.cue'),
      ]);
      expect(groups.single.items.length, 2);
      expect(groups.single.specification(chinese: true), contains('混合规格'));
    },
  );

  test('only explicitly numbered disc folders with matching tags join', () {
    expect(
      albumSources([
        sourceTrack('1', '/a/CD1', disc: 1),
        sourceTrack('2', '/a/CD01', disc: 1),
        sourceTrack('3', '/a/CD2', disc: 2),
      ]).length,
      3,
      reason: 'Duplicate disc copies must not be guessed into a set',
    );
    expect(
      albumSources([
        sourceTrack('1', '/a/CD1', disc: 1),
        sourceTrack('2', '/a/CD2', disc: 2),
        sourceTrack('3', '/copy/CD1', disc: 1),
      ]).map((s) => s.items.length),
      [2, 1],
    );
    expect(
      albumSources([sourceTrack('1', '/a/CD1'), sourceTrack('2', '/a/CD2')])
          .length,
      2,
    );
    expect(
      albumSources([
        sourceTrack('1', '/a/CD1', disc: 1, cue: '/a/CD1/Album CD1.cue'),
        sourceTrack('2', '/a/CD2', disc: 2, cue: '/a/CD2/Album CD2.cue'),
      ]).single.items.length,
      2,
    );
  });

  test(
    'selection persists, search stays within it, missing source falls back',
    () async {
      final store = _Store();
      final bridge = _Catalog();
      final container = ProviderContainer(
        overrides: [
          catalogBridgeProvider.overrideWithValue(bridge),
          albumSourcePreferencesProvider.overrideWith(
            () => AlbumSourcePreferences(store),
          ),
        ],
      );
      addTearDown(container.dispose);
      final controller = container.read(catalogControllerProvider.notifier);
      await controller.refreshSources();
      expect(container.read(catalogVisibleItemsProvider).length, 2);
      await controller.open(album);
      final groups = container.read(catalogAlbumSourcesProvider);
      await container
          .read(albumSourcePreferencesProvider.notifier)
          .select(
            container.read(catalogControllerProvider).albumPreferenceKey,
            groups.last.id,
          );
      expect(
        container.read(catalogVisibleItemsProvider).single.reference.id,
        '2',
      );
      await controller.setSearch('Song 1');
      expect(container.read(catalogVisibleItemsProvider), isEmpty);
      expect(container.read(catalogAlbumSourcesProvider).length, 2);
      await controller.setSearch('');
      await controller.back();
      await controller.open(album);
      expect(
        container.read(catalogVisibleItemsProvider).single.reference.id,
        '2',
      );
      final persisted = ProviderContainer(
        overrides: [
          albumSourcePreferencesProvider.overrideWith(
            () => AlbumSourcePreferences(store),
          ),
        ],
      );
      expect(
        persisted.read(albumSourcePreferencesProvider).values.single,
        groups.last.id,
      );
      persisted.dispose();
      bridge.rows = [bridge.rows.first];
      await controller.refresh();
      expect(
        container.read(catalogVisibleItemsProvider).single.reference.id,
        '1',
      );
    },
  );
}
