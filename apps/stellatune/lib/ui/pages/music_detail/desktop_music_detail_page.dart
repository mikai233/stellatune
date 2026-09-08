import 'package:flutter/material.dart';

import 'music_detail_scaffold.dart';

class DesktopMusicDetailPage extends StatelessWidget {
  const DesktopMusicDetailPage({super.key});

  @override
  Widget build(BuildContext context) =>
      const MusicDetailScaffold(mobile: false);
}
