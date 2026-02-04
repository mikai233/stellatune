import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';

class FolderTree extends StatefulWidget {
  const FolderTree({
    super.key,
    required this.roots,
    required this.folders,
    this.excludedFolders = const [],
    required this.selectedFolder,
    required this.onSelectAll,
    required this.onSelectFolder,
    this.isEditing = false,
    this.onDeleteFolder,
    this.onRestoreFolder,
  });

  final List<String> roots;
  final List<String> folders;
  final List<String> excludedFolders;

  /// Normalized. Empty means "All music".
  final String selectedFolder;
  final VoidCallback onSelectAll;
  final void Function(String folder) onSelectFolder;
  final bool isEditing;
  final void Function(String folder)? onDeleteFolder;
  final void Function(String folder)? onRestoreFolder;

  @override
  State<FolderTree> createState() => _FolderTreeState();
}

class _FolderTreeState extends State<FolderTree> {
  final Set<String> _expanded = <String>{};
  List<String> _rootsSorted = const [];
  Map<String, List<String>> _children = const {};
  Set<String> _excludedSet = const {};
  List<_FolderRow> _visibleRows = const [];
  GlobalKey<AnimatedListState> _listKey = GlobalKey<AnimatedListState>();
  int _listVersion = 0;
  static const _animDuration = Duration(milliseconds: 180);

  void _toggleExpanded(_FolderRow row, int index) {
    if (row.folder.isEmpty) return;

    if (_expanded.contains(row.folder)) {
      _collapseAt(row, index);
    } else {
      _expandAt(row, index);
    }
  }

  @override
  void initState() {
    super.initState();
    _expanded.addAll(widget.roots);
    _rebuildIndex();
    _visibleRows = _computeVisibleRows();
  }

  @override
  void didUpdateWidget(covariant FolderTree oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.roots != widget.roots) {
      // Auto-expand newly added roots, but don't forcibly re-expand roots the
      // user has collapsed.
      for (final r in widget.roots) {
        if (!oldWidget.roots.contains(r)) {
          _expanded.add(r);
        }
      }
    }

    if (oldWidget.roots != widget.roots ||
        oldWidget.folders != widget.folders ||
        oldWidget.excludedFolders != widget.excludedFolders ||
        oldWidget.isEditing != widget.isEditing) {
      setState(() {
        _rebuildIndex();
        _visibleRows = _computeVisibleRows();
        _listKey = GlobalKey<AnimatedListState>();
        _listVersion++;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return KeyedSubtree(
      key: ValueKey(_listVersion),
      child: AnimatedList(
        key: _listKey,
        initialItemCount: _visibleRows.length,
        itemBuilder: (context, index, animation) {
          final row = _visibleRows[index];
          return _buildAnimatedRow(
            context,
            row,
            index,
            animation,
            isRemoving: false,
          );
        },
      ),
    );
  }

  void _rebuildIndex() {
    final rootsSorted = widget.roots.toList()..sort();
    final allFolders = <String>{
      ...widget.folders,
      ...rootsSorted,
      if (widget.isEditing) ...widget.excludedFolders,
    };

    final children = <String, List<String>>{};
    for (final f in allFolders) {
      final parent = _parentOf(f);
      if (parent == null) continue;
      // Do not connect folders to a bare drive root like "D:".
      if (_isDriveRoot(parent)) continue;
      (children[parent] ??= <String>[]).add(f);
    }
    for (final entry in children.entries) {
      entry.value.sort();
    }

    _rootsSorted = rootsSorted;
    _children = children;
    _excludedSet = widget.isEditing
        ? widget.excludedFolders.toSet()
        : const <String>{};
  }

  List<_FolderRow> _computeVisibleRows() {
    final out = <_FolderRow>[
      _FolderRow(folder: '', depth: 0, isRoot: false),
      for (final r in _rootsSorted) ..._buildVisibleForRoot(r, _children),
    ];
    return out;
  }

  void _expandAt(_FolderRow row, int index) {
    _expanded.add(row.folder);

    final inserted = _buildVisibleDescendants(row.folder, row.depth, _children);
    if (inserted.isNotEmpty) {
      final insertAt = index + 1;
      _visibleRows.insertAll(insertAt, inserted);
      final list = _listKey.currentState;
      if (list != null) {
        for (var i = 0; i < inserted.length; i++) {
          list.insertItem(insertAt + i, duration: _animDuration);
        }
      }
    }

    setState(() {});
  }

  void _collapseAt(_FolderRow row, int index) {
    _expanded.remove(row.folder);

    final list = _listKey.currentState;
    final removeAt = index + 1;

    while (removeAt < _visibleRows.length &&
        _visibleRows[removeAt].depth > row.depth) {
      final removed = _visibleRows.removeAt(removeAt);
      list?.removeItem(
        removeAt,
        (context, animation) => _buildAnimatedRow(
          context,
          removed,
          removeAt,
          animation,
          isRemoving: true,
        ),
        duration: _animDuration,
      );
    }

    setState(() {});
  }

  Widget _buildAnimatedRow(
    BuildContext context,
    _FolderRow row,
    int index,
    Animation<double> animation, {
    required bool isRemoving,
  }) {
    final curve = isRemoving ? Curves.easeInCubic : Curves.easeOutCubic;
    final curved = CurvedAnimation(parent: animation, curve: curve);
    final child = _buildRowTile(context, row, index);

    return ClipRect(
      child: SizeTransition(
        sizeFactor: curved,
        axisAlignment: -1,
        child: isRemoving ? IgnorePointer(child: child) : child,
      ),
    );
  }

  Widget _buildRowTile(BuildContext context, _FolderRow row, int index) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final materialL10n = MaterialLocalizations.of(context);

    const buttonExtent = 40.0;
    const buttonIconSize = 20.0;

    final isAll = row.folder.isEmpty;
    final selected = widget.selectedFolder == row.folder;

    final hasChildren =
        row.folder.isNotEmpty && (_children[row.folder]?.isNotEmpty ?? false);
    final expanded = _expanded.contains(row.folder);
    final isExcluded = widget.isEditing && _excludedSet.contains(row.folder);

    final title = isAll ? l10n.libraryAllMusic : _basename(row.folder);

    final showDelete = widget.isEditing && !isAll && !isExcluded;
    final showRestore = widget.isEditing && !isAll && isExcluded;

    final trailing = (hasChildren || showDelete || showRestore)
        ? FittedBox(
            fit: BoxFit.scaleDown,
            alignment: Alignment.centerRight,
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (hasChildren)
                  Tooltip(
                    message: expanded ? l10n.collapse : l10n.expand,
                    child: InkResponse(
                      // Use onTapDown so expand/collapse feels more "instant"
                      // (IconButton triggers on tap-up).
                      onTapDown: (_) => _toggleExpanded(row, index),
                      radius: buttonExtent / 2,
                      child: SizedBox(
                        width: buttonExtent,
                        height: buttonExtent,
                        child: Icon(
                          expanded ? Icons.expand_less : Icons.expand_more,
                          size: buttonIconSize,
                        ),
                      ),
                    ),
                  ),
                if (showDelete)
                  IconButton(
                    tooltip: materialL10n.deleteButtonTooltip,
                    constraints: const BoxConstraints.tightFor(
                      width: buttonExtent,
                      height: buttonExtent,
                    ),
                    padding: EdgeInsets.zero,
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.close, size: buttonIconSize),
                    onPressed: widget.onDeleteFolder == null
                        ? null
                        : () => widget.onDeleteFolder!(row.folder),
                  ),
                if (showRestore)
                  IconButton(
                    constraints: const BoxConstraints.tightFor(
                      width: buttonExtent,
                      height: buttonExtent,
                    ),
                    padding: EdgeInsets.zero,
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.undo, size: buttonIconSize),
                    onPressed: widget.onRestoreFolder == null
                        ? null
                        : () => widget.onRestoreFolder!(row.folder),
                  ),
              ],
            ),
          )
        : null;

    final titleColor = isExcluded ? theme.colorScheme.error : null;

    return ListTile(
      dense: true,
      selected: selected,
      leading: SizedBox(
        width: 24 + row.depth * 14,
        child: Align(
          alignment: Alignment.centerRight,
          child: row.depth == 0
              ? const Icon(Icons.folder_outlined, size: 18)
              : const Icon(Icons.subdirectory_arrow_right, size: 16),
        ),
      ),
      title: Text(
        title,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: titleColor == null ? null : TextStyle(color: titleColor),
      ),
      trailing: trailing,
      onTap: () {
        if (isAll) {
          widget.onSelectAll();
        } else {
          widget.onSelectFolder(row.folder);
        }
      },
    );
  }

  List<_FolderRow> _buildVisibleForRoot(
    String root,
    Map<String, List<String>> children,
  ) {
    final out = <_FolderRow>[];
    out.add(_FolderRow(folder: root, depth: 0, isRoot: true));

    if (!_expanded.contains(root)) return out;

    out.addAll(_buildVisibleDescendants(root, 0, children));

    return out;
  }

  List<_FolderRow> _buildVisibleDescendants(
    String folder,
    int folderDepth,
    Map<String, List<String>> children,
  ) {
    if (!_expanded.contains(folder)) return const [];

    final out = <_FolderRow>[];
    final stack = <_FolderRow>[];

    final folderChildren = children[folder] ?? const <String>[];
    for (final c in folderChildren.reversed) {
      stack.add(_FolderRow(folder: c, depth: folderDepth + 1, isRoot: false));
    }

    while (stack.isNotEmpty) {
      final cur = stack.removeLast();
      out.add(cur);
      if (!_expanded.contains(cur.folder)) continue;
      final kids = children[cur.folder] ?? const <String>[];
      for (final c in kids.reversed) {
        stack.add(_FolderRow(folder: c, depth: cur.depth + 1, isRoot: false));
      }
    }

    return out;
  }

  static String _basename(String path) {
    final parts = path.split('/');
    return parts.isEmpty ? path : parts.last;
  }

  static String? _parentOf(String path) {
    final idx = path.lastIndexOf('/');
    if (idx <= 0) return null;
    return path.substring(0, idx);
  }

  static bool _isDriveRoot(String path) =>
      path.length == 2 && path.endsWith(':');
}

class _FolderRow {
  const _FolderRow({
    required this.folder,
    required this.depth,
    required this.isRoot,
  });

  final String folder;
  final int depth;
  final bool isRoot;
}
