import 'dart:io';

import 'package:flutter/material.dart';

class HomeArtwork extends StatelessWidget {
  const HomeArtwork({
    super.key,
    required this.asset,
    this.filePath,
    this.alignment = Alignment.center,
  });
  final String asset;
  final String? filePath;
  final Alignment alignment;

  @override
  Widget build(BuildContext context) {
    Widget fallback() => Image.asset(
      asset,
      fit: BoxFit.cover,
      alignment: alignment,
      errorBuilder: (_, _, _) => const ColoredBox(
        color: Color(0xFF76818B),
        child: Center(child: Icon(Icons.music_note, color: Colors.white54)),
      ),
    );
    if (filePath == null) return fallback();
    return Image.file(
      File(filePath!),
      fit: BoxFit.cover,
      alignment: alignment,
      cacheWidth: 400,
      frameBuilder: (_, child, frame, _) => frame == null ? fallback() : child,
      errorBuilder: (_, _, _) => fallback(),
    );
  }
}
