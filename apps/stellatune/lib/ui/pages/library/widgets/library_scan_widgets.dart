import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';

class LibraryScanStatusCard extends StatelessWidget {
  const LibraryScanStatusCard({
    super.key,
    required this.isScanning,
    required this.scanned,
    required this.updated,
    required this.skipped,
    required this.errors,
    required this.durationMs,
  });

  final bool isScanning;
  final int scanned;
  final int updated;
  final int skipped;
  final int errors;
  final int? durationMs;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final title = isScanning
        ? l10n.scanStatusScanning
        : l10n.scanStatusFinished;
    final subtitle = durationMs == null
        ? null
        : l10n.scanDurationMs(durationMs!);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Row(
          children: [
            if (isScanning)
              const SizedBox(
                width: 18,
                height: 18,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            else
              const Icon(Icons.check_circle_outline),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: Theme.of(context).textTheme.titleMedium),
                  if (subtitle != null)
                    Text(
                      subtitle,
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                ],
              ),
            ),
            LibraryScanStat(label: l10n.scanLabelScanned, value: scanned),
            LibraryScanStat(label: l10n.scanLabelUpdated, value: updated),
            LibraryScanStat(label: l10n.scanLabelSkipped, value: skipped),
            LibraryScanStat(label: l10n.scanLabelErrors, value: errors),
          ],
        ),
      ),
    );
  }
}

class LibraryScanStat extends StatelessWidget {
  const LibraryScanStat({super.key, required this.label, required this.value});

  final String label;
  final int value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(left: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(
            value.toString(),
            style: Theme.of(context).textTheme.titleMedium,
          ),
          Text(label, style: Theme.of(context).textTheme.bodySmall),
        ],
      ),
    );
  }
}
