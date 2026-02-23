import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';

class DlnaActionResult {
  const DlnaActionResult({
    this.applySelection = false,
    this.selected,
    this.message,
  });

  final bool applySelection;
  final DlnaRenderer? selected;
  final String? message;
}

class DlnaDialog extends StatefulWidget {
  const DlnaDialog({super.key, required this.selected});

  final DlnaRenderer? selected;

  @override
  State<DlnaDialog> createState() => _DlnaDialogState();
}

class _DlnaDialogState extends State<DlnaDialog> {
  late Future<List<DlnaRenderer>> _future;
  DlnaRenderer? _selected;

  @override
  void initState() {
    super.initState();
    _selected = widget.selected;
    _future = const DlnaBridge().discoverRenderers(
      timeout: const Duration(milliseconds: 1200),
    );
  }

  void _refresh() {
    setState(() {
      _future = const DlnaBridge().discoverRenderers(
        timeout: const Duration(milliseconds: 1200),
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);

    final okLabel = MaterialLocalizations.of(context).okButtonLabel;
    final cancelLabel = MaterialLocalizations.of(context).cancelButtonLabel;
    final screenH = MediaQuery.sizeOf(context).height;
    final listHeight = (screenH * 0.45).clamp(260.0, 420.0);

    return Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(24, 20, 24, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Row(
                children: [
                  Icon(Icons.cast, color: theme.colorScheme.primary),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(l10n.dlna, style: theme.textTheme.titleLarge),
                  ),
                  IconButton(
                    tooltip: l10n.refresh,
                    onPressed: _refresh,
                    icon: const Icon(Icons.refresh),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  l10n.settingsOutputTitle,
                  style: theme.textTheme.labelMedium?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
              const SizedBox(height: 12),
              SizedBox(
                height: listHeight,
                child: FutureBuilder<List<DlnaRenderer>>(
                  future: _future,
                  builder: (context, snapshot) {
                    final data = snapshot.data;
                    if (snapshot.connectionState != ConnectionState.done) {
                      return const Center(child: CircularProgressIndicator());
                    }
                    if (snapshot.hasError) {
                      return _DlnaEmptyState(
                        icon: Icons.error_outline,
                        title: l10n.dlnaSearchFailed(snapshot.error.toString()),
                        subtitle: '${snapshot.error}',
                        onRetry: _refresh,
                      );
                    }

                    final devices = data ?? const [];
                    if (devices.isEmpty) {
                      return _DlnaEmptyState(
                        icon: Icons.wifi_off,
                        title: l10n.dlnaNoDevices,
                        subtitle: l10n.dlnaNoDevicesSubtitle,
                        onRetry: _refresh,
                      );
                    }

                    return Container(
                      decoration: BoxDecoration(
                        color: theme.colorScheme.surfaceContainerLow,
                        borderRadius: BorderRadius.circular(16),
                        border: Border.all(
                          color: theme.colorScheme.outlineVariant,
                        ),
                      ),
                      clipBehavior: Clip.antiAlias,
                      child: ListView.separated(
                        shrinkWrap: true,
                        itemCount: devices.length + 1,
                        separatorBuilder: (context, index) =>
                            const Divider(height: 1),
                        itemBuilder: (context, i) {
                          if (i == 0) {
                            final selected = _selected == null;
                            return ListTile(
                              dense: true,
                              leading: const Icon(Icons.computer),
                              title: Text(l10n.deviceLocal),
                              subtitle: Text(l10n.deviceLocalSubtitle),
                              trailing: selected
                                  ? Icon(
                                      Icons.check_circle,
                                      color: theme.colorScheme.primary,
                                    )
                                  : null,
                              selected: selected,
                              onTap: () => setState(() => _selected = null),
                            );
                          }

                          final d = devices[i - 1];
                          final ok = d.avTransportControlUrl != null;
                          final selected = _selected?.usn == d.usn;
                          final volOk = d.renderingControlUrl != null;
                          final subtitle = ok
                              ? (volOk ? null : l10n.dlnaNoVolumeSupport)
                              : l10n.dlnaNoAvTransportSupport;
                          return ListTile(
                            dense: true,
                            enabled: ok,
                            leading: const Icon(Icons.speaker),
                            title: Text(d.friendlyName),
                            subtitle: subtitle == null ? null : Text(subtitle),
                            trailing: selected
                                ? Icon(
                                    Icons.check_circle,
                                    color: theme.colorScheme.primary,
                                  )
                                : null,
                            selected: selected,
                            onTap: ok
                                ? () => setState(() => _selected = d)
                                : null,
                          );
                        },
                      ),
                    );
                  },
                ),
              ),
              const SizedBox(height: 16),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: Text(cancelLabel),
                  ),
                  const SizedBox(width: 8),
                  FilledButton(
                    onPressed: () {
                      final d = _selected;
                      if (d == null) {
                        Navigator.of(context).pop(
                          DlnaActionResult(
                            applySelection: true,
                            selected: null,
                            message: l10n.dlnaSwitchedToLocal,
                          ),
                        );
                        return;
                      }
                      if (d.avTransportControlUrl == null) {
                        Navigator.of(context).pop(
                          DlnaActionResult(
                            applySelection: false,
                            message: l10n.dlnaNoAvTransportSupport,
                          ),
                        );
                        return;
                      }
                      Navigator.of(context).pop(
                        DlnaActionResult(
                          applySelection: true,
                          selected: d,
                          message: l10n.dlnaSelected(d.friendlyName),
                        ),
                      );
                    },
                    child: Text(okLabel),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _DlnaEmptyState extends StatelessWidget {
  const _DlnaEmptyState({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.onRetry,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 12),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 36, color: theme.colorScheme.onSurfaceVariant),
            const SizedBox(height: 12),
            Text(title, style: theme.textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              subtitle,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            FilledButton.tonalIcon(
              onPressed: onRetry,
              icon: const Icon(Icons.refresh),
              label: Text(l10n.refresh),
            ),
          ],
        ),
      ),
    );
  }
}
