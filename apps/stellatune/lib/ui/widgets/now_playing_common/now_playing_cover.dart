import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:stellatune/player/queue_models.dart';

class NowPlayingCover extends StatelessWidget {
  const NowPlayingCover({
    super.key,
    required this.coverDir,
    required this.trackId,
    this.cover,
    required this.primaryColor,
    required this.onTap,
  });

  final String coverDir;
  final int? trackId;
  final QueueCover? cover;
  final Color primaryColor;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final placeholder = Container(
      width: 48,
      height: 48,
      decoration: BoxDecoration(
        color: primaryColor.withValues(alpha: 0.12),
        border: Border.all(color: primaryColor.withValues(alpha: 0.18)),
      ),
      child: Icon(Icons.music_note, color: primaryColor),
    );

    if (trackId == null) {
      return MouseRegion(
        cursor: onTap != null
            ? SystemMouseCursors.click
            : SystemMouseCursors.basic,
        child: GestureDetector(
          onTap: onTap,
          child: _buildCoverByRef(placeholder),
        ),
      );
    }

    final coverPath = '$coverDir${Platform.pathSeparator}$trackId';
    final provider = ResizeImage(
      FileImage(File(coverPath)),
      width: 96,
      height: 96,
      allowUpscaling: false,
    );

    return MouseRegion(
      cursor: onTap != null
          ? SystemMouseCursors.click
          : SystemMouseCursors.basic,
      child: GestureDetector(
        onTap: onTap,
        child: Image(
          image: provider,
          width: 48,
          height: 48,
          fit: BoxFit.cover,
          gaplessPlayback: true,
          errorBuilder: (context, error, stackTrace) =>
              _buildCoverByRef(placeholder),
        ),
      ),
    );
  }

  Widget _buildCoverByRef(Widget placeholder) {
    final ref = cover;
    if (ref == null) return placeholder;
    switch (ref.kind) {
      case QueueCoverKind.url:
        return ClipRRect(
          borderRadius: BorderRadius.circular(2),
          child: Image.network(
            ref.value,
            width: 48,
            height: 48,
            fit: BoxFit.cover,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
      case QueueCoverKind.file:
        return ClipRRect(
          borderRadius: BorderRadius.circular(2),
          child: Image.file(
            File(ref.value),
            width: 48,
            height: 48,
            fit: BoxFit.cover,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
      case QueueCoverKind.data:
        final bytes = _decodeCoverBytes(ref.value);
        if (bytes == null) return placeholder;
        return ClipRRect(
          borderRadius: BorderRadius.circular(2),
          child: Image.memory(
            bytes,
            width: 48,
            height: 48,
            fit: BoxFit.cover,
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
