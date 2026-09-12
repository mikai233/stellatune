import 'catalog_bridge.dart';
import 'album_sources.dart';

enum CatalogTrackColumn { original, title, artist, album, duration, format }

/// View sorting works on the complete collection, independently of provider paging.
class CatalogTrackSort {
  const CatalogTrackSort({
    this.column = CatalogTrackColumn.original,
    this.descending = false,
  });

  final CatalogTrackColumn column;
  final bool descending;
  String get key => '${column.name}:$descending';

  CatalogTrackSort toggle(CatalogTrackColumn next) => CatalogTrackSort(
    column: next,
    descending: next != CatalogTrackColumn.original && next == column
        ? !descending
        : false,
  );

  List<CatalogItem> apply(List<CatalogItem> items) {
    if (column == CatalogTrackColumn.original) return items;
    // Preserve source order for equal values, even when reversing direction.
    final indexed = items.indexed.toList();
    String? text(CatalogItem item) {
      final value = switch (column) {
        CatalogTrackColumn.title => item.title,
        CatalogTrackColumn.artist => item.artist,
        CatalogTrackColumn.album => item.album,
        CatalogTrackColumn.format => trackFormat(item),
        _ => null,
      }?.trim().toLowerCase();
      return value == null || value.isEmpty ? null : value;
    }

    indexed.sort((a, b) {
      final Object? left = column == CatalogTrackColumn.duration
          ? a.$2.durationMs?.toInt()
          : text(a.$2);
      final Object? right = column == CatalogTrackColumn.duration
          ? b.$2.durationMs?.toInt()
          : text(b.$2);
      // Missing metadata stays at the end in both directions.
      if (left == null && right != null) return 1;
      if (right == null && left != null) return -1;
      var order = left == null
          ? 0
          : left is int
          ? left.compareTo(right as int)
          : (left as String).compareTo(right as String);
      if (descending) order = -order;
      return order == 0 ? a.$1.compareTo(b.$1) : order;
    });
    return List.unmodifiable(indexed.map((entry) => entry.$2));
  }
}
