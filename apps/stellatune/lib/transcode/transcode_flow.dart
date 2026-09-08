import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'dart:async';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:path/path.dart' as p;
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/transcode/transcode_encoder_picker.dart';
import 'package:stellatune/transcode/transcode_options_dialog.dart';
import 'package:stellatune/transcode/transcode_progress_view.dart';
import 'package:stellatune/transcode/transcode_task_controller.dart';

typedef TranscodeAction = Future<void> Function(
  TrackLite track, {
  Offset? anchorGlobalPosition,
});

/// Bind at the feature's page boundary; generic track lists only emit an action.
TranscodeAction transcodeAction(BuildContext context, PlayerBridge bridge) =>
    (track, {anchorGlobalPosition}) => showTranscodeFlow(
      context,
      bridge: bridge,
      track: track,
      anchorGlobalPosition: anchorGlobalPosition,
    );

Future<void> showTranscodeFlow(
  BuildContext context, {
  required PlayerBridge bridge,
  required TrackLite track,
  Offset? anchorGlobalPosition,
}) async {
  final l10n = AppLocalizations.of(context)!;
  try {
    final encoder = await pickTranscodeEncoder(
      context,
      bridge: bridge,
      anchorGlobalPosition: anchorGlobalPosition,
    );
    if (encoder == null || !context.mounted) return;
    final params = await editTranscodeOptions(context, encoder);
    if (params == null || !context.mounted) return;
    final sourcePath = track.path.trim();
    if (sourcePath.isEmpty) throw StateError('source path is empty');
    final location = await getSaveLocation(
      suggestedName: _outputFileName(track, encoder),
    );
    final outputPath = location?.path.trim();
    if (outputPath == null || outputPath.isEmpty || !context.mounted) return;
    final taskId =
        'transcode_${DateTime.now().microsecondsSinceEpoch}_${track.id}';
    final controller = TranscodeTaskController(
      outputPath: outputPath,
      start: () => bridge.transcodeTrackLocal(
        taskId: taskId,
        sourcePath: sourcePath,
        outputPath: outputPath,
        encoderPluginId: encoder.pluginId,
        encoderTypeId: encoder.typeId,
        encoderConfigJson: params.configJson,
        encoderOptionsJson: params.optionsJson,
      ),
      cancelTask: () => bridge.transcodeCancel(taskId: taskId),
    );
    final outcome = await showTranscodeProgress(
      context,
      controller: controller,
      encoderName: encoder.displayName,
      sourceName: _trackName(track),
    );
    if (outcome == null || !context.mounted) return;
    if (outcome.result == TranscodeResult.failed) {
      DiagnosticsService.instance.report(
        outcome.error ?? StateError('Transcoding failed'),
        operation: 'transcode',
      );
      return;
    }
    final message = switch (outcome.result) {
      TranscodeResult.completed => l10n.transcodeSucceededWithPath(
        outcome.outputPath,
      ),
      TranscodeResult.canceled => l10n.transcodeCanceled,
      TranscodeResult.failed => l10n.transcodeFailedWithError(
        outcome.error?.toString() ?? l10n.error,
      ),
    };
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(behavior: SnackBarBehavior.floating, content: Text(message)),
    );
  } catch (error) {
    DiagnosticsService.instance.report(error, operation: 'transcode_start');
  }
}

Future<TranscodeOutcome?> showTranscodeProgress(
  BuildContext context, {
  required TranscodeTaskController controller,
  required String encoderName,
  required String sourceName,
}) async {
  try {
    return await showGeneralDialog<TranscodeOutcome>(
      context: context,
      barrierDismissible: false,
      barrierLabel: AppLocalizations.of(context)!.transcodeProgressDialogTitle,
      barrierColor: Colors.black.withValues(alpha: 0.38),
      transitionDuration: const Duration(milliseconds: 180),
      pageBuilder: (_, _, _) => _TranscodeTaskDialog(
        controller: controller,
        encoderName: encoderName,
        sourceName: sourceName,
      ),
      transitionBuilder: (_, animation, _, child) => FadeTransition(
        opacity: animation,
        child: ScaleTransition(
          scale: Tween<double>(begin: 0.94, end: 1).animate(
            CurvedAnimation(
              parent: animation,
              curve: Curves.easeOutCubic,
              reverseCurve: Curves.easeInCubic,
            ),
          ),
          child: child,
        ),
      ),
    );
  } catch (_) {
    controller.dispose();
    rethrow;
  }
}

class _TranscodeTaskDialog extends StatefulWidget {
  const _TranscodeTaskDialog({
    required this.controller,
    required this.encoderName,
    required this.sourceName,
  });
  final TranscodeTaskController controller;
  final String encoderName;
  final String sourceName;
  @override
  State<_TranscodeTaskDialog> createState() => _TranscodeTaskDialogState();
}

class _TranscodeTaskDialogState extends State<_TranscodeTaskDialog> {
  @override
  void initState() {
    super.initState();
    unawaited(_run());
  }

  Future<void> _run() async {
    final result = await widget.controller.run();
    if (!mounted) return;
    final route = ModalRoute.of(context);
    final navigator = Navigator.of(context);
    // Completion owns this specific dialog; never pop an unrelated route above it.
    if (route?.isCurrent ?? false) {
      navigator.pop(result);
    } else if (route != null) {
      navigator.removeRoute(route, result);
    }
  }

  @override
  void dispose() {
    widget.controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => PopScope(
    canPop: false,
    child: SafeArea(
      child: Center(
        child: TranscodeProgressDialogCard(
          controller: widget.controller,
          encoderName: widget.encoderName,
          sourceName: widget.sourceName,
        ),
      ),
    ),
  );
}

String _trackName(TrackLite track) {
  final title = track.title?.trim();
  if (title != null && title.isNotEmpty) return title;
  final name = p.basenameWithoutExtension(track.path).trim();
  return name.isEmpty ? 'Track' : name;
}

String _outputFileName(TrackLite track, EncoderTypeDescriptor encoder) {
  final name = _trackName(track)
      .replaceAll(RegExp(r'[\\/:*?"<>|]'), '_')
      .trim();
  final segments = encoder.typeId.toLowerCase().split(RegExp(r'[^a-z0-9]+'));
  const ignored = {'encoder', 'encode', 'audio', 'plugin'};
  final extension =
      segments.reversed
          .where((s) => !ignored.contains(s) && s.length >= 2 && s.length <= 8)
          .firstOrNull ??
      'out';
  return '${name.isEmpty ? 'track' : name}.$extension';
}
