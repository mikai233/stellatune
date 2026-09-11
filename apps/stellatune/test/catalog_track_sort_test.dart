import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_track_sort.dart';

void main() {
  final items = [
    CatalogItem(
      reference: const MediaRef(
        sourceInstanceId: '1',
        kind: MediaKind.track,
        id: 'a',
      ),
      title: 'Zulu',
      artist: 'Amy',
      album: 'Beta',
      durationMs: 9000,
      artistRefs: const [],
    ),
    CatalogItem(
      reference: const MediaRef(
        sourceInstanceId: '1',
        kind: MediaKind.track,
        id: 'b',
      ),
      title: 'alpha',
      artist: 'zoe',
      album: 'Alpha',
      durationMs: 10000,
      artistRefs: const [],
    ),
    CatalogItem(
      reference: const MediaRef(
        sourceInstanceId: '1',
        kind: MediaKind.track,
        id: 'c',
      ),
      title: ' ALPHA ',
      artist: 'amy',
      album: 'Beta',
      durationMs: 9000,
      artistRefs: const [],
    ),
    const CatalogItem(
      reference: MediaRef(
        sourceInstanceId: '1',
        kind: MediaKind.track,
        id: 'd',
      ),
      title: '',
      artistRefs: [],
    ),
  ];
  List<String> ids(List<CatalogItem> rows) =>
      rows.map((r) => r.reference.id).toList();

  test('columns sort both ways with stable ties and missing values last', () {
    final expected = {
      CatalogTrackColumn.title: (['b', 'c', 'a', 'd'], ['a', 'b', 'c', 'd']),
      CatalogTrackColumn.artist: (['a', 'c', 'b', 'd'], ['b', 'a', 'c', 'd']),
      CatalogTrackColumn.album: (['b', 'a', 'c', 'd'], ['a', 'c', 'b', 'd']),
      CatalogTrackColumn.duration: (['a', 'c', 'b', 'd'], ['b', 'a', 'c', 'd']),
    };
    for (final entry in expected.entries) {
      final sort = const CatalogTrackSort().toggle(entry.key);
      expect(ids(sort.apply(items)), entry.value.$1);
      expect(ids(sort.toggle(entry.key).apply(items)), entry.value.$2);
      expect(
        sort.toggle(CatalogTrackColumn.original).apply(items),
        same(items),
      );
    }
    expect(ids(items), ['a', 'b', 'c', 'd']);
  });
}
