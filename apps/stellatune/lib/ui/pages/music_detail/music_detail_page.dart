import 'dart:io';

import 'package:flutter/material.dart';
import 'package:stellatune/ui/pages/music_detail/desktop_music_detail_page.dart';
import 'package:stellatune/ui/pages/music_detail/mobile_music_detail_page.dart';

/// Full-screen music detail page showing album cover, track info and lyrics placeholder.
class MusicDetailPage extends StatelessWidget {
  const MusicDetailPage({super.key});

  @override
  Widget build(BuildContext context) {
    if (Platform.isAndroid || Platform.isIOS) {
      return const MobileMusicDetailPage();
    }
    return const DesktopMusicDetailPage();
  }
}
