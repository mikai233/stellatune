import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/transcode/transcode_encoder_picker.dart';
import 'package:stellatune/ui/forms/schema_form.dart';

class TranscodeLaunchParams {
  const TranscodeLaunchParams({required this.configJson, this.optionsJson});
  final String configJson;
  final String? optionsJson;
}

Future<TranscodeLaunchParams?> editTranscodeOptions(
  BuildContext context,
  EncoderTypeDescriptor encoder,
) => showDialog<TranscodeLaunchParams>(
  context: context,
  builder: (_) => _TranscodeOptionsDialog(encoder: encoder),
);

class _TranscodeOptionsDialog extends StatefulWidget {
  const _TranscodeOptionsDialog({required this.encoder});
  final EncoderTypeDescriptor encoder;
  @override
  State<_TranscodeOptionsDialog> createState() =>
      _TranscodeOptionsDialogState();
}

class _TranscodeOptionsDialogState extends State<_TranscodeOptionsDialog> {
  final _optionsController = TextEditingController();
  late String _configDraft = widget.encoder.defaultConfigJson;
  String? _error;
  @override
  void dispose() {
    _optionsController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final encoder = widget.encoder;
    return AlertDialog(
      title: Text(l10n.transcodeParamsDialogTitle),
      content: SizedBox(
        width: 620,
        child: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                encoder.displayName,
                style: theme.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 4),
              Text(
                encoderSubtitle(encoder),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: 14),
              SchemaForm(
                schemaJson: encoder.configSchemaJson,
                initialValueJson: _configDraft,
                fallbackLabel: l10n.transcodeParamsConfigLabel,
                onChangedJson: (json) => _configDraft = json,
              ),
              const SizedBox(height: 10),
              TextField(
                controller: _optionsController,
                minLines: 2,
                maxLines: 6,
                decoration: InputDecoration(
                  border: const OutlineInputBorder(),
                  labelText: l10n.transcodeParamsOptionsLabel,
                  helperText: l10n.transcodeParamsOptionsHint,
                ),
              ),
              if (_error != null) ...[
                const SizedBox(height: 10),
                Text(
                  _error!,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.error,
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () {
            try {
              final config = _configDraft.trim();
              final options = _optionsController.text.trim();
              Navigator.of(context).pop(
                TranscodeLaunchParams(
                  configJson: jsonEncode(
                    jsonDecode(config.isEmpty ? '{}' : config),
                  ),
                  optionsJson: options.isEmpty
                      ? null
                      : jsonEncode(jsonDecode(options)),
                ),
              );
            } catch (_) {
              setState(() => _error = l10n.transcodeParamsInvalidJson);
            }
          },
          child: Text(l10n.transcodeParamsConfirm),
        ),
      ],
    );
  }
}
