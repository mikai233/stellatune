import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_tree_controller.dart';

class TreeBridge extends CatalogBridge {
  final requests = <(CatalogQuery, Completer<CatalogPage>)>[];
  @override
  Future<CatalogPage> browse(CatalogQuery query) {
    final request = Completer<CatalogPage>();
    requests.add((query, request));
    return request.future;
  }
}

CatalogItem folder(String id) => CatalogItem(
  isSegment: false,
  reference: MediaRef(sourceInstanceId: 'a', kind: MediaKind.folder, id: id),
  title: id,
  artistRefs: const [],
);

void main() {
  test(
    'large trees load only expanded branches and retain opaque paging on retry',
    () async {
      final bridge = TreeBridge();
      final tree = CatalogTreeController(bridge, 'a');
      addTearDown(tree.dispose);
      final root = tree.load(null);
      bridge.requests.last.$2.complete(
        CatalogPage(
          items: [for (var i = 0; i < 100; i++) folder('$i')],
          nextCursor: 'next-100',
        ),
      );
      await Future<void>.delayed(Duration.zero);
      expect(bridge.requests, hasLength(2));
      expect(bridge.requests.last.$1.cursor, 'next-100');
      bridge.requests.last.$2.completeError(StateError('offline'));
      await root;
      expect(tree.page(null).items, hasLength(100));
      final retry = tree.load(null, more: true);
      expect(bridge.requests.last.$1.cursor, 'next-100');
      bridge.requests.last.$2.complete(CatalogPage(items: [folder('100')]));
      await retry;
      final branch = folder('0').reference;
      final open = tree.toggle(branch);
      expect(bridge.requests.last.$1.parent, branch);
      bridge.requests.last.$2.complete(CatalogPage(items: [folder('child')]));
      await open;
      await tree.toggle(branch);
      await tree.toggle(branch);
      expect(bridge.requests, hasLength(4));
      expect(tree.page(null).items, hasLength(101));
      expect(tree.page(null).cursor, isNull);
    },
  );
  test(
    'refresh discards stale responses and reloads expanded branches',
    () async {
      final bridge = TreeBridge();
      final tree = CatalogTreeController(bridge, 'a');
      addTearDown(tree.dispose);
      final old = tree.load(null);
      final refreshed = tree.refresh();
      tree.expanded.add(folder('new').reference);
      bridge.requests[0].$2.complete(CatalogPage(items: [folder('old')]));
      await old;
      expect(tree.page(null).loading, isTrue);
      bridge.requests[1].$2.complete(CatalogPage(items: [folder('new')]));
      await Future<void>.delayed(Duration.zero);
      expect(bridge.requests.last.$1.parent?.id, 'new');
      bridge.requests.last.$2.complete(const CatalogPage(items: []));
      await refreshed;
      expect(tree.page(null).items.single.title, 'new');
      expect(tree.page(folder('new').reference).loaded, isTrue);
    },
  );
  test(
    'cursor cycles fail visibly and disposal ignores outstanding requests',
    () async {
      final bridge = TreeBridge();
      final tree = CatalogTreeController(bridge, 'a');
      final root = tree.load(null);
      bridge.requests.last.$2.complete(
        CatalogPage(items: [folder('0')], nextCursor: 'same'),
      );
      await Future<void>.delayed(Duration.zero);
      bridge.requests.last.$2.complete(
        CatalogPage(items: [folder('1')], nextCursor: 'same'),
      );
      await root;
      expect(tree.page(null).error, isNotNull);
      expect(tree.page(null).items, hasLength(1));
      final refresh = tree.refresh();
      tree.dispose();
      bridge.requests.last.$2.completeError(StateError('late'));
      await refresh;
    },
  );
}
