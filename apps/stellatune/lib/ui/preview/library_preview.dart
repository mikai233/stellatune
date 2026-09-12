import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/ui/pages/home/home_placeholders.dart';
import 'package:stellatune/ui/preview/library_sample_view.dart';
import 'package:stellatune/ui/widgets/track_list.dart';

/// Fixed fixtures used only by visual tests and the separate preview entry.
class LibraryVisualPreview extends StatefulWidget {
  const LibraryVisualPreview({
    super.key,
    this.initialSection = LibrarySection.songs,
  });
  final LibrarySection initialSection;
  static const coverDir = 'build/visual-review/library-covers';
  static final tracks = [
    for (var i = 0; i < 18; i++)
      TrackLite(
        isSegment: false,
        id: i + 1,
        path: 'preview-$i.flac',
        title: [
          '开始懂了',
          '世界是一片孤独海',
          '夜に駆ける',
          'First Love',
          '告白气球',
          'The Moment',
          '逆光',
          '蓝色时分',
          'Letters',
        ][i % 9],
        artist: [
          '孙燕姿',
          'WOVOP / 洛天依',
          'YOASOBI',
          '宇多田光',
          '周杰伦',
          'Novo Amor',
          '孙燕姿',
          'RADWIMPS',
          'Novo Amor',
        ][i % 9],
        album: [
          '我要的幸福',
          '世界是一片孤独海',
          'THE BOOK',
          'First Love',
          '周杰伦的床边故事',
          'Stefanie',
          '逆光',
          '天気の子',
          'Cannot Be, Whatsoever',
        ][i % 9],
        durationMs: 248000 + i * 7000,
      ),
  ];
  static Future<void> prepareCovers() async {
    await Directory(coverDir).create(recursive: true);
    for (var i = 0; i < tracks.length; i++) {
      final bytes = await rootBundle.load(
        HomePlaceholders.artwork[i % HomePlaceholders.artwork.length],
      );
      await File('$coverDir/${tracks[i].id}').writeAsBytes(
        bytes.buffer.asUint8List(bytes.offsetInBytes, bytes.lengthInBytes),
      );
    }
  }

  @override
  State<LibraryVisualPreview> createState() => _LibraryVisualPreviewState();
}

class _LibraryVisualPreviewState extends State<LibraryVisualPreview> {
  late LibrarySection section = widget.initialSection;
  final liked = <int>{1, 5};
  @override
  Widget build(BuildContext context) => SampleLibraryView(
    tracks: LibraryVisualPreview.tracks,
    coverDir: LibraryVisualPreview.coverDir,
    section: section,
    onSectionChanged: (value) => setState(() => section = value),
    foldersView: const Center(child: Text('D:/Music')),
    onAddFolder: () {},
    onScan: (_) {},
    trackListBuilder: (items) => TrackList(
      tableLayout: true,
      coverDir: LibraryVisualPreview.coverDir,
      items: items,
      likedTrackIds: liked,
      playlists: const [],
      currentPlaylistId: null,
      onActivate: (index, tracks) async {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('播放：${tracks[index].title}')));
      },
      onEnqueue: (_) async {},
      onSetLiked: (track, value) async => setState(() {
        if (value) {
          liked.add(track.id.toInt());
        } else {
          liked.remove(track.id.toInt());
        }
      }),
      onAddToPlaylist: (_, _) async {},
      onRemoveFromPlaylist: (_, _) async {},
    ),
  );
}
