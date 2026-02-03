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

  @override
  void initState() {
    super.initState();
    _expanded.addAll(widget.roots);
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
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    final roots = widget.roots.toList()..sort();
    final allFolders = <String>{
      ...widget.folders,
      ...roots,
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

    final visible = <_FolderRow>[
      _FolderRow(folder: '', depth: 0, isRoot: false),
      for (final r in roots) ..._buildVisibleForRoot(r, children),
    ];

    return ListView.builder(
      itemCount: visible.length,
      itemBuilder: (context, i) {
        final row = visible[i];
        final isAll = row.folder.isEmpty;
        final selected = widget.selectedFolder == row.folder;

        final hasChildren =
            row.folder.isNotEmpty &&
            (children[row.folder]?.isNotEmpty ?? false);
        final expanded = _expanded.contains(row.folder);
        final isExcluded =
            widget.isEditing && widget.excludedFolders.contains(row.folder);

        final title = isAll ? l10n.libraryAllMusic : _basename(row.folder);

        final showDelete = widget.isEditing && !isAll && !isExcluded;
        final showRestore = widget.isEditing && !isAll && isExcluded;
        final trailing = (hasChildren || showDelete || showRestore)
            ? Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (hasChildren)
                    IconButton(
                      tooltip: expanded ? l10n.collapse : l10n.expand,
                      icon: Icon(
                        expanded ? Icons.expand_less : Icons.expand_more,
                      ),
                      onPressed: () {
                        setState(() {
                          if (expanded) {
                            _expanded.remove(row.folder);
                          } else {
                            _expanded.add(row.folder);
                          }
                        });
                      },
                    ),
                  if (showDelete)
                    IconButton(
                      tooltip: MaterialLocalizations.of(
                        context,
                      ).deleteButtonTooltip,
                      icon: const Icon(Icons.close),
                      onPressed: widget.onDeleteFolder == null
                          ? null
                          : () => widget.onDeleteFolder!(row.folder),
                    ),
                  if (showRestore)
                    IconButton(
                      icon: const Icon(Icons.undo),
                      onPressed: widget.onRestoreFolder == null
                          ? null
                          : () => widget.onRestoreFolder!(row.folder),
                    ),
                ],
              )
            : null;

        final theme = Theme.of(context);
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
    final stack = <_FolderRow>[];
    final rootChildren = children[root] ?? const <String>[];
    for (final c in rootChildren.reversed) {
      stack.add(_FolderRow(folder: c, depth: 1, isRoot: false));
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
