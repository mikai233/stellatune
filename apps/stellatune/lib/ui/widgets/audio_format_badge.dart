import 'package:flutter/material.dart';

class AudioFormatBadge extends StatelessWidget {
  const AudioFormatBadge({super.key, required this.path, this.sampleRate});

  final String path;
  final int? sampleRate;

  @override
  Widget build(BuildContext context) {
    final extension = _getExtension(path);
    if (extension.isEmpty) return const SizedBox.shrink();

    final theme = Theme.of(context);
    final isHiRes =
        _isHiRes(extension) || (sampleRate != null && sampleRate! > 48000);

    // Color logic based on quality/format
    final Color badgeColor;
    if (isHiRes) {
      badgeColor = Colors.amber.shade700;
    } else if (['MP3', 'AAC', 'M4A', 'OGG'].contains(extension)) {
      badgeColor = theme.colorScheme.outline;
    } else {
      badgeColor = theme.colorScheme.secondary;
    }

    final label = _getLabel(extension, isHiRes, sampleRate);

    return Container(
      margin: const EdgeInsets.only(right: 6),
      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 0.5),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(3),
        border: Border.all(
          color: badgeColor.withValues(alpha: 0.4),
          width: 0.8,
        ),
        color: badgeColor.withValues(alpha: 0.08),
      ),
      child: Text(
        label,
        style: TextStyle(
          color: badgeColor,
          fontSize: 9,
          fontWeight: FontWeight.w800,
          letterSpacing: 0.4,
          fontFamily: 'monospace',
        ),
      ),
    );
  }

  String _getLabel(String ext, bool isHiRes, int? sampleRate) {
    if (sampleRate != null && sampleRate > 0) {
      final khz = (sampleRate / 1000)
          .toStringAsFixed(1)
          .replaceAll(RegExp(r'\.0$'), '');
      if (isHiRes) {
        return 'HI-RES $khz\u{1D458}Hz';
      }
      return '$ext $khz\u{1D458}Hz';
    }
    return isHiRes && ext == 'FLAC' ? 'HI-RES' : ext;
  }

  String _getExtension(String path) {
    final parts = path.split('.');
    if (parts.length < 2) return '';
    return parts.last.toUpperCase();
  }

  bool _isHiRes(String ext) {
    final e = ext.toLowerCase();
    return const ['flac', 'wav', 'dsd', 'dsf', 'dff', 'ape'].contains(e);
  }
}
