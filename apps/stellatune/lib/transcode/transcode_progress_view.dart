import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/transcode/transcode_task_controller.dart';

class TranscodeProgressDialogCard extends StatelessWidget {
  const TranscodeProgressDialogCard({
    super.key,
    required this.controller,
    required this.encoderName,
    required this.sourceName,
  });

  final TranscodeTaskController controller;
  final String encoderName;
  final String sourceName;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    final l10n = AppLocalizations.of(context)!;
    final surfaceTop = colorScheme.surfaceContainerHighest.withValues(
      alpha: 0.98,
    );
    final surfaceBottom = colorScheme.surface.withValues(alpha: 0.98);
    final borderColor = colorScheme.outlineVariant.withValues(alpha: 0.42);

    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 560),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 20),
        child: Material(
          color: Colors.transparent,
          child: ListenableBuilder(
            listenable: controller,
            builder: (context, child) {
              final event = controller.progress;
              final canceling = controller.canceling;
              final phase = event?.phase.trim().toLowerCase() ?? 'started';
              final processed = event?.processedFrames ?? BigInt.zero;
              final total = event?.totalFrames;
              final writtenBytes = event?.writtenBytes ?? BigInt.zero;
              final elapsedMs = event?.elapsedMs;
              final progress = _progressRatio(processed, total);
              final statusColor = switch (phase) {
                'failed' => colorScheme.error,
                'canceled' => colorScheme.error,
                'completed' => colorScheme.primary,
                _ => colorScheme.primary,
              };
              final progressText = progress == null
                  ? '...'
                  : '${(progress * 100).clamp(0, 100).toStringAsFixed(1)}%';
              final isTerminal =
                  phase == 'failed' ||
                  phase == 'completed' ||
                  phase == 'canceled';

              return AnimatedContainer(
                duration: const Duration(milliseconds: 220),
                curve: Curves.easeOutCubic,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(24),
                  border: Border.all(color: borderColor),
                  gradient: LinearGradient(
                    begin: Alignment.topCenter,
                    end: Alignment.bottomCenter,
                    colors: [surfaceTop, surfaceBottom],
                  ),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withValues(alpha: 0.18),
                      blurRadius: 28,
                      offset: const Offset(0, 12),
                    ),
                  ],
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Container(
                      width: double.infinity,
                      padding: const EdgeInsets.fromLTRB(20, 16, 20, 16),
                      decoration: BoxDecoration(
                        borderRadius: const BorderRadius.vertical(
                          top: Radius.circular(24),
                        ),
                        gradient: LinearGradient(
                          colors: [
                            colorScheme.primary.withValues(alpha: 0.20),
                            colorScheme.primary.withValues(alpha: 0.08),
                          ],
                        ),
                      ),
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Container(
                            width: 42,
                            height: 42,
                            decoration: BoxDecoration(
                              color: colorScheme.primary.withValues(
                                alpha: 0.16,
                              ),
                              borderRadius: BorderRadius.circular(12),
                            ),
                            child: Icon(
                              Icons.transform_rounded,
                              color: colorScheme.primary,
                            ),
                          ),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  l10n.transcodeProgressDialogTitle,
                                  style: theme.textTheme.titleMedium?.copyWith(
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                                const SizedBox(height: 4),
                                Text(
                                  l10n.transcodeProgressDialogSubtitle(
                                    encoderName,
                                  ),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: theme.textTheme.bodySmall?.copyWith(
                                    color: colorScheme.onSurfaceVariant,
                                  ),
                                ),
                                const SizedBox(height: 2),
                                Text(
                                  sourceName,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: theme.textTheme.bodySmall?.copyWith(
                                    color: colorScheme.onSurfaceVariant,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                    Padding(
                      padding: const EdgeInsets.fromLTRB(20, 16, 20, 20),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              Expanded(
                                child: Text(
                                  _phaseLabel(l10n, phase),
                                  style: theme.textTheme.bodyMedium?.copyWith(
                                    color: statusColor,
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                              ),
                              Text(
                                progressText,
                                style: theme.textTheme.bodyMedium?.copyWith(
                                  fontFeatures: const [
                                    FontFeature.tabularFigures(),
                                  ],
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                            ],
                          ),
                          const SizedBox(height: 10),
                          ClipRRect(
                            borderRadius: BorderRadius.circular(999),
                            child: SizedBox(
                              height: 10,
                              child: progress == null
                                  ? LinearProgressIndicator(
                                      value: null,
                                      backgroundColor:
                                          colorScheme.surfaceContainerHighest,
                                      valueColor: AlwaysStoppedAnimation<Color>(
                                        colorScheme.primary,
                                      ),
                                    )
                                  : TweenAnimationBuilder<double>(
                                      tween: Tween<double>(
                                        begin: 0,
                                        end: progress,
                                      ),
                                      duration: const Duration(
                                        milliseconds: 260,
                                      ),
                                      curve: Curves.easeOutCubic,
                                      builder: (context, value, child) {
                                        return LinearProgressIndicator(
                                          value: value,
                                          backgroundColor: colorScheme
                                              .surfaceContainerHighest,
                                          valueColor:
                                              AlwaysStoppedAnimation<Color>(
                                                statusColor,
                                              ),
                                        );
                                      },
                                    ),
                            ),
                          ),
                          const SizedBox(height: 14),
                          Wrap(
                            spacing: 8,
                            runSpacing: 8,
                            children: [
                              _TranscodeMetricChip(
                                label: l10n.transcodeStatProcessed,
                                value: _formatFrames(processed, total),
                              ),
                              _TranscodeMetricChip(
                                label: l10n.transcodeStatWritten,
                                value: _formatBytes(writtenBytes),
                              ),
                              _TranscodeMetricChip(
                                label: l10n.transcodeStatElapsed,
                                value: _formatElapsed(elapsedMs),
                              ),
                            ],
                          ),
                          const SizedBox(height: 14),
                          if (controller.cancelError case final error?)
                            Text(
                              DiagnosticsService.instance.messageFor(
                                error,
                                operation: 'transcode_cancel',
                              ),
                              style: TextStyle(color: colorScheme.error),
                            ),
                          Align(
                            alignment: Alignment.centerRight,
                            child: TextButton.icon(
                              onPressed: canceling || isTerminal
                                  ? null
                                  : () {
                                      unawaited(controller.cancel());
                                    },
                              icon: canceling
                                  ? const SizedBox(
                                      width: 14,
                                      height: 14,
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      ),
                                    )
                                  : const Icon(Icons.close_rounded),
                              label: Text(
                                canceling
                                    ? l10n.transcodeCanceling
                                    : l10n.transcodeCancel,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              );
            },
          ),
        ),
      ),
    );
  }

  String _phaseLabel(AppLocalizations l10n, String phase) {
    return switch (phase) {
      'failed' => l10n.transcodeStateFailed,
      'canceled' => l10n.transcodeStateCanceled,
      'completed' => l10n.transcodeStateCompleted,
      'progress' => l10n.transcodeStateProcessing,
      _ => l10n.transcodeStatePreparing,
    };
  }

  double? _progressRatio(BigInt processed, BigInt? total) {
    if (total == null || total <= BigInt.zero) return null;
    final capped = processed > total ? total : processed;
    final numerator = capped.toDouble();
    final denominator = total.toDouble();
    if (denominator <= 0) return null;
    return numerator / denominator;
  }

  String _formatFrames(BigInt processed, BigInt? total) {
    final processedText = _groupDigits(processed);
    if (total == null || total <= BigInt.zero) {
      return processedText;
    }
    return '$processedText / ${_groupDigits(total)}';
  }

  String _formatBytes(BigInt bytes) {
    final value = bytes < BigInt.zero ? BigInt.zero : bytes;
    final units = <String>['B', 'KB', 'MB', 'GB', 'TB'];
    var v = value.toDouble();
    var idx = 0;
    while (v >= 1024 && idx < units.length - 1) {
      v /= 1024;
      idx += 1;
    }
    final digits = v >= 100 ? 0 : (v >= 10 ? 1 : 2);
    return '${v.toStringAsFixed(digits)} ${units[idx]}';
  }

  String _formatElapsed(BigInt? elapsedMs) {
    final ms = elapsedMs?.toInt() ?? 0;
    final totalSeconds = (ms / 1000).floor().clamp(0, 24 * 60 * 60 * 99);
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }

  String _groupDigits(BigInt value) {
    final raw = value.toString();
    final buffer = StringBuffer();
    for (var i = 0; i < raw.length; i += 1) {
      final idxFromEnd = raw.length - i;
      buffer.write(raw[i]);
      if (idxFromEnd > 1 && idxFromEnd % 3 == 1) {
        buffer.write(',');
      }
    }
    return buffer.toString();
  }
}

class _TranscodeMetricChip extends StatelessWidget {
  const _TranscodeMetricChip({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(999),
        color: colorScheme.surfaceContainerHighest.withValues(alpha: 0.72),
        border: Border.all(
          color: colorScheme.outlineVariant.withValues(alpha: 0.38),
        ),
      ),
      child: RichText(
        text: TextSpan(
          style: theme.textTheme.bodySmall?.copyWith(
            color: colorScheme.onSurface,
            fontFeatures: const [FontFeature.tabularFigures()],
          ),
          children: [
            TextSpan(
              text: '$label: ',
              style: const TextStyle(fontWeight: FontWeight.w600),
            ),
            TextSpan(text: value),
          ],
        ),
      ),
    );
  }
}
