import 'package:flutter/material.dart';

/// Shared column widths keep the header and virtualized rows aligned.
class TrackTableLayout extends StatelessWidget {
  const TrackTableLayout({
    super.key,
    required this.number,
    required this.title,
    required this.artist,
    required this.album,
    required this.duration,
    required this.format,
    required this.actions,
  });
  final Widget number, title, artist, album, duration, format, actions;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, size) => Padding(
      padding: EdgeInsets.symmetric(horizontal: size.maxWidth < 600 ? 16 : 28),
      child: Row(
        children: [
          SizedBox(width: 40, child: number),
          Expanded(flex: 4, child: title),
          const SizedBox(width: 16),
          Expanded(flex: 2, child: artist),
          if (size.maxWidth >= 760) ...[
            const SizedBox(width: 16),
            Expanded(flex: 3, child: album),
          ],
          const SizedBox(width: 12),
          SizedBox(width: 60, child: duration),
          if (size.maxWidth >= 620) SizedBox(width: 62, child: format),
          SizedBox(width: 76, child: actions),
        ],
      ),
    ),
  );
}

class TrackTableHeader extends StatelessWidget {
  const TrackTableHeader({super.key});
  @override
  Widget build(BuildContext context) => SizedBox(
    height: 38,
    child: DefaultTextStyle(
      style: TextStyle(
        fontFamily: 'NotoSansSC',
        fontSize: 11,
        color: Theme.of(context).colorScheme.onSurfaceVariant,
      ),
      child: const TrackTableLayout(
        number: Text('#'),
        title: Padding(padding: EdgeInsets.only(left: 44), child: Text('标题')),
        artist: Text('艺术家'),
        album: Text('专辑'),
        duration: Text('时长'),
        format: Text('格式'),
        actions: Align(
          alignment: Alignment.centerLeft,
          child: Padding(
            padding: EdgeInsets.only(left: 10),
            child: Icon(Icons.favorite_border, size: 15),
          ),
        ),
      ),
    ),
  );
}
