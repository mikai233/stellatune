import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:stellatune/player/queue_models.dart';

class QueueCoverImage extends StatelessWidget {
  const QueueCoverImage({
    super.key,
    required this.cover,
    required this.placeholder,
    required this.size,
  });

  final QueueCover? cover;
  final Widget placeholder;
  final double size;

  @override
  Widget build(BuildContext context) {
    final ref = cover;
    if (ref == null) return placeholder;
    final radius = BorderRadius.circular(size == 44 ? 11 : 8);
    switch (ref.kind) {
      case QueueCoverKind.url:
        return ClipRRect(
          borderRadius: radius,
          child: Image.network(
            ref.value,
            width: size,
            height: size,
            fit: BoxFit.cover,
            filterQuality: FilterQuality.low,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
      case QueueCoverKind.file:
        return ClipRRect(
          borderRadius: radius,
          child: Image.file(
            File(ref.value),
            width: size,
            height: size,
            fit: BoxFit.cover,
            filterQuality: FilterQuality.low,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
      case QueueCoverKind.data:
        final bytes = _decodeCoverBytes(ref.value);
        if (bytes == null) return placeholder;
        return ClipRRect(
          borderRadius: radius,
          child: Image.memory(
            bytes,
            width: size,
            height: size,
            fit: BoxFit.cover,
            filterQuality: FilterQuality.low,
            gaplessPlayback: true,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
    }
  }

  Uint8List? _decodeCoverBytes(String raw) {
    final text = raw.trim();
    if (text.isEmpty) return null;
    final data = () {
      if (text.startsWith('data:')) {
        final comma = text.indexOf(',');
        if (comma <= 0 || comma >= text.length - 1) return '';
        return text.substring(comma + 1);
      }
      return text;
    }();
    if (data.isEmpty) return null;
    try {
      return base64Decode(data);
    } catch (_) {
      return null;
    }
  }
}
