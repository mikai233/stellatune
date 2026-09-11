import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_tree_controller.dart';

import 'catalog_widgets.dart';

class CatalogFolderView extends ConsumerStatefulWidget {
  const CatalogFolderView({
    super.key,
    required this.source,
    required this.path,
    required this.onSelect,
    required this.onRoot,
    required this.child,
    this.onAddFolder,
  });
  final LibrarySource source;
  final List<CatalogItem> path;
  final ValueChanged<List<CatalogItem>> onSelect;
  final VoidCallback onRoot;
  final VoidCallback? onAddFolder;
  final Widget child;
  @override
  ConsumerState<CatalogFolderView> createState() => _CatalogFolderViewState();
}

class _CatalogFolderViewState extends ConsumerState<CatalogFolderView> {
  late final CatalogTreeController tree;
  late final ScrollController scroll;
  @override
  void initState() {
    super.initState();
    tree = ref.read(catalogTreeProvider(widget.source.id));
    scroll = ScrollController(initialScrollOffset: tree.scrollOffset);
    scroll.addListener(() => tree.scrollOffset = scroll.offset);
    Future.microtask(() {
      if (mounted && widget.source.available) {
        tree.expanded.addAll(widget.path.map((item) => item.reference));
        tree.load(null);
      }
    });
  }

  @override
  void dispose() {
    scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: tree,
    builder: (context, _) {
      final l = context.catalogL10n;
      return LayoutBuilder(
        builder: (context, constraints) {
          final width = (constraints.maxWidth * .29).clamp(160.0, 250.0);
          return Row(
            children: [
              TweenAnimationBuilder<double>(
                duration: MediaQuery.disableAnimationsOf(context)
                    ? Duration.zero
                    : const Duration(milliseconds: 240),
                curve: Curves.easeInOutCubic,
                tween: Tween<double>(end: tree.collapsed ? 0 : width),
                child: _treePanel(context, width),
                builder: (context, value, child) => ClipRect(
                  child: SizedBox(
                    key: const ValueKey('catalog-tree-pane'),
                    width: value,
                    child: OverflowBox(
                      alignment: Alignment.centerLeft,
                      minWidth: width,
                      maxWidth: width,
                      child: ExcludeFocus(
                        excluding: tree.collapsed,
                        child: ExcludeSemantics(
                          excluding: tree.collapsed,
                          child: IgnorePointer(
                            ignoring: tree.collapsed,
                            child: child,
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              Expanded(
                child: Column(
                  children: [
                    SizedBox(
                      height: 48,
                      child: Row(
                        children: [
                          Padding(
                            padding: const EdgeInsets.only(left: 8, right: 4),
                            child: IconButton(
                              key: const ValueKey('catalog-toggle-tree'),
                              tooltip: tree.collapsed
                                  ? l.catalogExpandTree
                                  : l.catalogCollapseTree,
                              onPressed: () =>
                                  tree.setCollapsed(!tree.collapsed),
                              style: IconButton.styleFrom(
                                fixedSize: const Size.square(36),
                                minimumSize: const Size.square(36),
                                padding: const EdgeInsets.all(8),
                                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                                shape: RoundedRectangleBorder(
                                  borderRadius: BorderRadius.circular(8),
                                ),
                                hoverColor: Theme.of(context)
                                    .colorScheme
                                    .primary
                                    .withValues(alpha: .06),
                                highlightColor: Theme.of(context)
                                    .colorScheme
                                    .primary
                                    .withValues(alpha: .10),
                              ),
                              icon: Transform.flip(
                                flipX: true,
                                child: const Icon(
                                  Icons.view_sidebar_outlined,
                                  size: 20,
                                ),
                              ),
                            ),
                          ),
                          Expanded(
                            child: SingleChildScrollView(
                              scrollDirection: Axis.horizontal,
                              child: Row(
                                children: [
                                  TextButton(
                                    onPressed: widget.onRoot,
                                    child: Text(l.catalogMusicFolders),
                                  ),
                                  for (
                                    var i = 0;
                                    i < widget.path.length;
                                    i++
                                  ) ...[
                                    const Text('/'),
                                    TextButton(
                                      onPressed: () => widget.onSelect(
                                        widget.path.take(i + 1).toList(),
                                      ),
                                      child: Text(
                                        catalogTitle(context, widget.path[i]),
                                        style: TextStyle(
                                          fontWeight:
                                              i == widget.path.length - 1
                                              ? FontWeight.w600
                                              : FontWeight.normal,
                                        ),
                                      ),
                                    ),
                                  ],
                                ],
                              ),
                            ),
                          ),
                          const SizedBox(width: 12),
                        ],
                      ),
                    ),
                    Expanded(
                      child: widget.path.isEmpty
                          ? Center(
                              child: Padding(
                                padding: const EdgeInsets.all(24),
                                child: Text(
                                  l.catalogSelectFolder,
                                  textAlign: TextAlign.center,
                                  style: TextStyle(
                                    color: Theme.of(context)
                                        .colorScheme
                                        .onSurfaceVariant,
                                  ),
                                ),
                              ),
                            )
                          : widget.child,
                    ),
                  ],
                ),
              ),
            ],
          );
        },
      );
    },
  );

  Widget _treePanel(BuildContext context, double width) {
    final l = context.catalogL10n;
    final rows = <_TreeRow>[];
    void visit(MediaRef? parent, List<CatalogItem> path) {
      final page = tree.page(parent);
      for (final item in page.items) {
        if (path.any((p) => p.reference == item.reference)) continue;
        final next = [...path, item];
        rows.add(_TreeRow(path: next));
        if (tree.expanded.contains(item.reference)) visit(item.reference, next);
      }
      if (page.loading ||
          page.error != null ||
          page.cursor != null ||
          (page.loaded && page.items.isEmpty)) {
        rows.add(_TreeRow(path: path, page: page));
      }
    }

    visit(null, []);
    return Container(
      width: width,
      decoration: BoxDecoration(
        border: Border(
          right: BorderSide(color: Theme.of(context).dividerColor),
        ),
      ),
      child: Column(
        children: [
          SizedBox(
            height: 48,
            child: Padding(
              padding: const EdgeInsets.only(left: 16, right: 4),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      l.catalogMusicFolders,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ),
                  if (widget.onAddFolder != null)
                    IconButton(
                      tooltip: l.tooltipAddFolder,
                      onPressed: widget.onAddFolder,
                      icon: const Icon(Icons.add, size: 18),
                    ),
                ],
              ),
            ),
          ),
          Expanded(
            child: Scrollbar(
              controller: scroll,
              child: ListView.builder(
                key: const ValueKey('catalog-tree-scroll'),
                controller: scroll,
                padding: const EdgeInsets.fromLTRB(6, 0, 6, 12),
                itemCount: rows.length,
                itemBuilder: (context, index) {
                  final row = rows[index];
                  final page = row.page;
                  if (page != null) {
                    final parent = row.path.lastOrNull?.reference;
                    if (page.loading) {
                      return const Padding(
                        padding: EdgeInsets.all(12),
                        child: LinearProgressIndicator(minHeight: 2),
                      );
                    }
                    if (page.error != null) {
                      return Column(
                        children: [
                          Text(
                            page.error!,
                            style: TextStyle(
                              fontSize: 11,
                              color: Theme.of(context).colorScheme.error,
                            ),
                          ),
                          TextButton(
                            onPressed: widget.source.available
                                ? () => tree.load(
                                    parent,
                                    more: page.loaded && page.cursor != null,
                                  )
                                : null,
                            child: Text(l.catalogRetryPage),
                          ),
                          TextButton(
                            onPressed: tree.refresh,
                            child: Text(l.catalogRefreshRetry),
                          ),
                        ],
                      );
                    }
                    return Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 6,
                      ),
                      child: Text(
                        parent == null
                            ? l.catalogNoItems
                            : l.catalogNoSubfolders,
                        style: TextStyle(
                          fontSize: 11,
                          color: Theme.of(context).colorScheme.onSurfaceVariant,
                        ),
                      ),
                    );
                  }
                  final item = row.path.last;
                  final expanded = tree.expanded.contains(item.reference);
                  final selected =
                      widget.path.lastOrNull?.reference == item.reference;
                  final indent = ((row.path.length - 1) * 14.0).clamp(
                    0.0,
                    width * .32,
                  );
                  return Padding(
                    padding: EdgeInsets.only(left: indent),
                    child: Material(
                      color: selected
                          ? Theme.of(context).colorScheme.primary
                                .withValues(alpha: .10)
                          : Colors.transparent,
                      borderRadius: BorderRadius.circular(5),
                      child: InkWell(
                        onTap: widget.source.available
                            ? () => widget.onSelect(row.path)
                            : null,
                        borderRadius: BorderRadius.circular(5),
                        child: SizedBox(
                          height: 34,
                          child: Row(
                            children: [
                              SizedBox(
                                width: 28,
                                child: IconButton(
                                  key: ValueKey(
                                    'tree-expand-${item.reference.id}',
                                  ),
                                  padding: EdgeInsets.zero,
                                  tooltip: expanded ? l.collapse : l.expand,
                                  onPressed: widget.source.available
                                      ? () => unawaited(
                                          tree.toggle(item.reference),
                                        )
                                      : null,
                                  icon: Icon(
                                    expanded
                                        ? Icons.expand_more
                                        : Icons.chevron_right,
                                    size: 17,
                                  ),
                                ),
                              ),
                              Icon(
                                expanded
                                    ? Icons.folder_open_outlined
                                    : Icons.folder_outlined,
                                size: 18,
                              ),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Tooltip(
                                  message: catalogTitle(context, item),
                                  child: Text(
                                    catalogTitle(context, item),
                                    key: ValueKey(
                                      'tree-folder-${item.reference.id}',
                                    ),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                      fontSize: 12,
                                      fontWeight: selected
                                          ? FontWeight.w600
                                          : FontWeight.normal,
                                    ),
                                  ),
                                ),
                              ),
                              const SizedBox(width: 6),
                            ],
                          ),
                        ),
                      ),
                    ),
                  );
                },
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _TreeRow {
  const _TreeRow({required this.path, this.page});
  final List<CatalogItem> path;
  final CatalogTreePage? page;
}
