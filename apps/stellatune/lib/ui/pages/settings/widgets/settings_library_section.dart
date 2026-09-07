import 'package:flutter/material.dart';

import 'settings_section_card.dart';

class SettingsLibrarySection extends StatelessWidget {
  const SettingsLibrarySection({
    super.key,
    required this.roots,
    required this.onAdd,
    required this.onRemove,
    required this.onScan,
    required this.isScanning,
    this.status,
  });
  final List<String> roots;
  final VoidCallback onAdd;
  final ValueChanged<String> onRemove;
  final ValueChanged<bool> onScan;
  final bool isScanning;
  final String? status;
  @override
  Widget build(BuildContext context) => SettingsSectionCard(
    title: '音乐库',
    icon: Icons.folder_rounded,
    subtitle: '管理本地音乐文件与媒体库',
    children: [
      Padding(
        padding: const EdgeInsets.all(12),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '音乐文件夹',
                    style: TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
                  ),
                  SizedBox(height: 4),
                  Text(
                    '添加文件夹后扫描其中的音乐',
                    style: TextStyle(
                      fontSize: 12,
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
            OutlinedButton(onPressed: onAdd, child: Text('添加文件夹')),
          ],
        ),
      ),
      if (roots.isEmpty)
        const Padding(padding: EdgeInsets.all(12), child: Text('尚未添加音乐文件夹')),
      for (final root in roots)
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
          child: Material(
            color: Theme.of(context).colorScheme.surfaceContainerLow,
            borderRadius: BorderRadius.circular(8),
            child: Padding(
              padding: const EdgeInsets.only(left: 12),
              child: Row(
                children: [
                  const Icon(Icons.folder, size: 18),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Tooltip(
                      message: root,
                      child: Text(
                        root,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(fontSize: 12),
                      ),
                    ),
                  ),
                  IconButton(
                    tooltip: '从音乐库移除文件夹',
                    onPressed: () => onRemove(root),
                    icon: const Icon(Icons.close, size: 17),
                  ),
                ],
              ),
            ),
          ),
        ),
      if (status != null)
        Padding(
          padding: const EdgeInsets.all(12),
          child: Text(status!, style: const TextStyle(fontSize: 12)),
        ),
      Padding(
        padding: const EdgeInsets.all(12),
        child: Wrap(
          spacing: 10,
          runSpacing: 10,
          children: [
            OutlinedButton.icon(
              onPressed: isScanning ? null : () => onScan(false),
              icon: const Icon(Icons.refresh, size: 18),
              label: Text(isScanning ? '扫描中…' : '重新扫描音乐库'),
            ),
            TextButton(
              onPressed: isScanning ? null : () => onScan(true),
              child: const Text('强制重新扫描'),
            ),
          ],
        ),
      ),
    ],
  );
}
