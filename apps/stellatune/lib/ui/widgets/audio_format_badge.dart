import 'dart:convert';

import 'package:flutter/material.dart';

class AudioFormatBadge extends StatelessWidget {
  const AudioFormatBadge({super.key, required this.path, this.sampleRate});

  final String path;
  final int? sampleRate;

  @override
  Widget build(BuildContext context) {
    final extension = _resolveExtension(path);
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

  String _resolveExtension(String rawPath) {
    final text = rawPath.trim();
    if (text.isEmpty) return '';

    final fromLocator = _extensionFromLocator(text);
    if (fromLocator.isNotEmpty) return fromLocator;
    return _normalizeExtension(text);
  }

  String _extensionFromLocator(String text) {
    if (!text.startsWith('{') || !text.endsWith('}')) return '';
    try {
      final decoded = jsonDecode(text);
      if (decoded is! Map) return '';
      final map = decoded.cast<Object?, Object?>();

      final extHint = _asText(map['ext_hint']);
      final normalizedExtHint = _normalizeExtension(extHint ?? '');
      if (normalizedExtHint.isNotEmpty) return normalizedExtHint;

      final pathHint = _asText(map['path_hint']);
      final normalizedPathHint = _normalizeExtension(pathHint ?? '');
      if (normalizedPathHint.isNotEmpty) return normalizedPathHint;
    } catch (_) {
      // Fallback to plain-path parsing below.
    }
    return '';
  }

  String? _asText(Object? value) {
    if (value == null) return null;
    final text = value.toString().trim();
    return text.isEmpty ? null : text;
  }

  String _normalizeExtension(String source) {
    var text = source.trim();
    if (text.isEmpty) return '';

    final queryStart = text.indexOf('?');
    if (queryStart >= 0) {
      text = text.substring(0, queryStart);
    }
    final fragmentStart = text.indexOf('#');
    if (fragmentStart >= 0) {
      text = text.substring(0, fragmentStart);
    }

    final slash = text.lastIndexOf(RegExp(r'[\\/]'));
    if (slash >= 0 && slash + 1 < text.length) {
      text = text.substring(slash + 1);
    }

    final dot = text.lastIndexOf('.');
    if (dot < 0 || dot + 1 >= text.length) return '';
    final ext = text.substring(dot + 1).toUpperCase();
    if (!RegExp(r'^[A-Z0-9]{1,10}$').hasMatch(ext)) return '';
    return ext;
  }

  bool _isHiRes(String ext) {
    final e = ext.toLowerCase();
    return const ['flac', 'wav', 'dsd', 'dsf', 'dff', 'ape'].contains(e);
  }
}
