import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

class LogRecordTile extends StatelessWidget {
  const LogRecordTile({
    super.key,
    required this.record,
    required this.selected,
    required this.onTap,
  });
  final LogRecord record;
  final bool selected;
  final VoidCallback onTap;

  /// Two message lines, one source line, and padding. Fixed for a given text
  /// scale so large scroll jumps never measure all intervening log entries.
  static double extent(BuildContext context) {
    final scaler = MediaQuery.textScalerOf(context);
    return 28 +
        3 +
        (scaler.scale(13) * 1.6).ceilToDouble() * 2 +
        (scaler.scale(11) * 1.4).ceilToDouble();
  }

  static String preview(String message) {
    if (message.length <= 2048) return message;
    var end = 2048;
    final last = message.codeUnitAt(end - 1);
    if (last >= 0xd800 && last <= 0xdbff) end--;
    return '${message.substring(0, end)}…';
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final time = DateTime.fromMillisecondsSinceEpoch(record.timestampMs.toInt())
        .toIso8601String()
        .substring(11, 23);
    final color = record.level == 'ERROR' ? scheme.error : scheme.primary;
    return Material(
      color: selected
          ? scheme.primary.withValues(alpha: .09)
          : Colors.transparent,
      child: InkWell(
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
          decoration: BoxDecoration(
            border: Border(
              bottom: BorderSide(
                color: scheme.outlineVariant.withValues(alpha: .35),
              ),
            ),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                width: 94,
                child: Text(
                  time,
                  style: TextStyle(
                    fontSize: 12,
                    color: scheme.onSurfaceVariant,
                    height: 1.8,
                  ),
                ),
              ),
              Container(
                width: 55,
                padding: const EdgeInsets.symmetric(vertical: 3),
                decoration: BoxDecoration(
                  color: color.withValues(alpha: .09),
                  borderRadius: BorderRadius.circular(5),
                ),
                child: Text(
                  record.level,
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    fontSize: 10,
                    color: color,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      preview(record.message),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontSize: 13, height: 1.6),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      '${record.source} · ${record.target}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 11,
                        height: 1.4,
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class LogRecordDetails extends StatelessWidget {
  const LogRecordDetails({
    super.key,
    required this.record,
    required this.chinese,
    required this.onClose,
  });
  final LogRecord record;
  final bool chinese;
  final VoidCallback onClose;
  String get _text =>
      '${DateTime.fromMillisecondsSinceEpoch(record.timestampMs.toInt()).toIso8601String()} ${record.level} [${record.source}] ${record.target}\nID: ${record.id}${record.pluginId == null ? '' : '\nPlugin: ${record.pluginId}'}${record.generation == null ? '' : '\nGeneration: ${record.generation}'}\n\n${record.message}\n\n${record.details}';
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(20, 10, 10, 10),
        child: Row(
          children: [
            Expanded(
              child: Text(
                chinese ? '日志详情' : 'Details',
                style: Theme.of(context).textTheme.titleSmall,
              ),
            ),
            TextButton.icon(
              onPressed: () => Clipboard.setData(ClipboardData(text: _text)),
              icon: const Icon(Icons.copy_outlined, size: 16),
              label: Text(chinese ? '复制完整详情' : 'Copy details'),
            ),
            IconButton(
              tooltip: chinese ? '返回列表' : 'Back to list',
              onPressed: onClose,
              icon: const Icon(Icons.close, size: 18),
            ),
          ],
        ),
      ),
      const Divider(height: 1),
      Expanded(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(20),
          child: SelectableText(
            _text,
            style: const TextStyle(fontSize: 13, height: 1.65),
          ),
        ),
      ),
    ],
  );
}
