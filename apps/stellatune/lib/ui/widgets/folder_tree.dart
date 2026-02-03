import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';

class FolderTree extends StatefulWidget {
  const FolderTree({
    super.key,
    required this.roots,
    required this.folders,
    required this.selectedFolder,
    required this.onSelectAll,
    required this.onSelectFolder,
  });

  final List<String> roots;
  final List<String> folders;

  /// Normalized. Empty means "All music".
  final String selectedFolder;
  final VoidCallback onSelectAll;
  final void Function(String folder) onSelectFolder;

  @override
  State<FolderTree> createState() => _FolderTreeState();
}

class _FolderTreeState extends State<FolderTree> {
  final Set<String> _expanded = <String>{};

  @override
  void didUpdateWidget(covariant FolderTree oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.roots != widget.roots) {
      _expanded.addAll(widget.roots);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    final roots = widget.roots.toList()..sort();
    final allFolders = <String>{...widget.folders, ...roots};

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

        final title = isAll ? l10n.libraryAllMusic : _basename(row.folder);

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
          title: Text(title, maxLines: 1, overflow: TextOverflow.ellipsis),
          trailing: hasChildren
              ? IconButton(
                  tooltip: expanded ? l10n.collapse : l10n.expand,
                  icon: Icon(expanded ? Icons.expand_less : Icons.expand_more),
                  onPressed: () {
                    setState(() {
                      if (expanded) {
                        _expanded.remove(row.folder);
                      } else {
                        _expanded.add(row.folder);
                      }
                    });
                  },
                )
              : null,
          onTap: () {
            if (isAll) {
              widget.onSelectAll();
            } else {
              widget.onSelectFolder(row.folder);
              if (hasChildren && !_expanded.contains(row.folder)) {
                setState(() => _expanded.add(row.folder));
              }
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
    _expanded.add(root);
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
