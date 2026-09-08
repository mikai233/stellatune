import 'dart:io';

import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';

String encoderSubtitle(EncoderTypeDescriptor encoder) =>
    '${encoder.pluginName} (${encoder.pluginId}) · ${encoder.typeId}';

Future<EncoderTypeDescriptor?> pickTranscodeEncoder(
  BuildContext context, {
  required PlayerBridge bridge,
  Offset? anchorGlobalPosition,
}) async {
  final l10n = AppLocalizations.of(context)!;
  List<EncoderTypeDescriptor> encoders;
  try {
    // Refresh for each invocation: plugin installation/disable can change choices.
    encoders = List.of(await bridge.encoderListTypes())
      ..sort((a, b) {
        final display = a.displayName.toLowerCase().compareTo(
          b.displayName.toLowerCase(),
        );
        if (display != 0) return display;
        final plugin = a.pluginName.toLowerCase().compareTo(
          b.pluginName.toLowerCase(),
        );
        if (plugin != 0) return plugin;
        final type = a.typeId.compareTo(b.typeId);
        return type == 0 ? a.pluginId.compareTo(b.pluginId) : type;
      });
  } catch (error) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('${l10n.transcodeLoadEncodersFailed}: $error')),
      );
    }
    return null;
  }
  if (!context.mounted) return null;
  if (encoders.isEmpty) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(l10n.transcodeNoEncoders)));
    return null;
  }
  final desktop = Platform.isWindows || Platform.isLinux || Platform.isMacOS;
  if (desktop && anchorGlobalPosition != null) {
    final overlayContext = Overlay.of(context).context;
    if (!overlayContext.mounted) return null;
    final overlay = overlayContext.findRenderObject();
    if (overlay is! RenderBox) return null;
    final theme = Theme.of(context);
    return showMenu<EncoderTypeDescriptor>(
      context: context,
      position: RelativeRect.fromLTRB(
        anchorGlobalPosition.dx,
        anchorGlobalPosition.dy,
        overlay.size.width - anchorGlobalPosition.dx,
        overlay.size.height - anchorGlobalPosition.dy,
      ),
      constraints: const BoxConstraints(maxWidth: 420, maxHeight: 460),
      items: [
        for (final encoder in encoders)
          PopupMenuItem(
            value: encoder,
            child: Row(
              children: [
                const Icon(Icons.file_upload_outlined, size: 18),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        encoder.displayName,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      const SizedBox(height: 2),
                      Text(
                        encoderSubtitle(encoder),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
  return showModalBottomSheet<EncoderTypeDescriptor>(
    context: context,
    showDragHandle: true,
    builder: (context) => SafeArea(
      child: SizedBox(
        height: (encoders.length * 64.0 + 104).clamp(220.0, 460.0),
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
              child: Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  l10n.transcodeSelectEncoderTitle,
                  style: Theme.of(context).textTheme.titleMedium
                      ?.copyWith(fontWeight: FontWeight.w600),
                ),
              ),
            ),
            Expanded(
              child: ListView.separated(
                itemCount: encoders.length,
                separatorBuilder: (_, _) => const Divider(height: 1),
                itemBuilder: (context, index) {
                  final encoder = encoders[index];
                  return ListTile(
                    leading: const Icon(Icons.file_upload_outlined),
                    title: Text(encoder.displayName),
                    subtitle: Text(encoderSubtitle(encoder)),
                    onTap: () => Navigator.of(context).pop(encoder),
                  );
                },
              ),
            ),
          ],
        ),
      ),
    ),
  );
}
