import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'catalog_bridge.dart';

/// Per-source state survives collapsing the pane and switching library tabs.
final catalogTreeProvider = Provider.family<CatalogTreeController, String>((
  ref,
  source,
) {
  final tree = CatalogTreeController(ref.watch(catalogBridgeProvider), source);
  ref.onDispose(tree.dispose);
  return tree;
});

class CatalogTreePage {
  List<CatalogItem> items = const [];
  String? cursor, error;
  bool loading = false, loaded = false;
  final cursors = <String>{};
}

class CatalogTreeController extends ChangeNotifier {
  CatalogTreeController(this.bridge, this.sourceId);
  final CatalogBridge bridge;
  final String sourceId;
  final pages = <MediaRef?, CatalogTreePage>{};
  final expanded = <MediaRef>{};
  double scrollOffset = 0;
  bool collapsed = false;
  int _generation = 0;
  bool _disposed = false;

  CatalogTreePage page(MediaRef? parent) =>
      pages.putIfAbsent(parent, CatalogTreePage.new);

  Future<void> toggle(MediaRef parent) async {
    if (!expanded.remove(parent)) {
      expanded.add(parent);
      notifyListeners();
      await load(parent);
    } else {
      notifyListeners();
    }
  }

  void setCollapsed(bool value) {
    collapsed = value;
    notifyListeners();
  }

  Future<void> refresh() async {
    _generation++;
    pages.clear();
    await load(null);
  }

  Future<void> load(MediaRef? parent, {bool more = false}) async {
    final target = page(parent);
    if (_disposed ||
        target.loading ||
        (more
            ? target.cursor == null
            : target.loaded && target.error == null)) {
      return;
    }
    final generation = _generation;
    var cursor = more ? target.cursor : null;
    final items = more ? [...target.items] : <CatalogItem>[];
    final seen = items.map((item) => item.reference).toSet();
    if (!more) target.cursors.clear();
    target.loading = true;
    target.error = null;
    notifyListeners();
    final publishTimer = Stopwatch()..start();
    var published = false;
    try {
      do {
        final result = await bridge.browse(
          CatalogQuery(
            sourceInstanceId: sourceId,
            kind: MediaKind.folder,
            parent: parent,
            search: '',
            sort: CatalogSort.default_,
            cursor: cursor,
            limit: 200,
          ),
        );
        if (_disposed || generation != _generation) return;
        final pageRefs = <MediaRef>{};
        if (result.items.any(
              (item) =>
                  item.reference.sourceInstanceId != sourceId ||
                  item.reference.kind != MediaKind.folder ||
                  seen.contains(item.reference) ||
                  !pageRefs.add(item.reference),
            ) ||
            (result.nextCursor != null &&
                (result.nextCursor == cursor ||
                    target.cursors.contains(result.nextCursor)))) {
          throw StateError('Folder collection changed; refresh to continue');
        }
        if (cursor != null) target.cursors.add(cursor);
        items.addAll(result.items);
        seen.addAll(pageRefs);
        cursor = result.nextCursor;
        if (!published ||
            cursor == null ||
            publishTimer.elapsedMilliseconds >= 100) {
          target.items = List.unmodifiable(items);
          target.cursor = cursor;
          target.loaded = true;
          notifyListeners();
          published = true;
          publishTimer.reset();
        }
      } while (cursor != null && !_disposed && generation == _generation);
    } catch (e) {
      if (!_disposed && generation == _generation) {
        target.items = List.unmodifiable(items);
        target.cursor = cursor;
        target.error = DiagnosticsService.instance.failureMessage(
          e,
          operation: 'catalog',
        );
      }
    } finally {
      if (!_disposed && generation == _generation) {
        target.loading = false;
        notifyListeners();
      }
    }
    if (!_disposed && generation == _generation && target.error == null) {
      // Refresh only materialized, expanded branches, never the entire library.
      for (final item in target.items) {
        if (expanded.contains(item.reference) && item.reference != parent) {
          await load(item.reference);
        }
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _generation++;
    super.dispose();
  }
}
