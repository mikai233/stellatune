import 'package:flutter/material.dart';
import 'package:stellatune/library/album_sources.dart';
import 'package:stellatune/library/catalog_bridge.dart';

/// Static cached properties: no metadata requests during scrolling or hover.
class CatalogAudioLabel extends StatelessWidget {
  const CatalogAudioLabel({
    super.key,
    required this.item,
    this.compact = false,
  });
  final CatalogItem item;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final chinese = Localizations.localeOf(context).languageCode == 'zh';
    final label = trackFormat(item);
    return Align(
      alignment: Alignment.centerLeft,
      widthFactor: 1,
      heightFactor: 1,
      child: Tooltip(
        message:
            '${audioSpecification(item, chinese: chinese)}\n${chinese ? '点击查看曲目信息' : 'Click for track information'}',
        child: InkWell(
          onTap: () => showCatalogTrackInfo(context, item),
          borderRadius: BorderRadius.circular(4),
          child: Padding(
            padding: EdgeInsets.symmetric(
              horizontal: 6,
              vertical: compact ? 2 : 3,
            ),
            child: Text(
              label.isEmpty ? '—' : label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontSize: compact ? 10 : 11,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

void showCatalogTrackInfo(BuildContext context, CatalogItem item) {
  final chinese = Localizations.localeOf(context).languageCode == 'zh';
  String tr(String zh, String en) => chinese ? zh : en;
  final audio = item.audio;
  final fields = <String, String>{
    tr('音频规格', 'Audio'): audioSpecification(
      item,
      includeBitrate: false,
      chinese: chinese,
    ),
    tr('编码', 'Codec'): audio?.codec ?? '—',
    tr('声道数', 'Channels'): audio?.channels?.toString() ?? '—',
    if (audio != null && (audio.bitrate != null || isLossyAudio(audio)))
      item.isSegment
          ? tr('源文件平均码率', 'Source file average bit rate')
          : audio.bitrate?.kind == BitrateKind.average
          ? tr('平均码率', 'Average bit rate')
          : audio.bitrate?.kind == BitrateKind.nominal
          ? tr('标称码率', 'Nominal bit rate')
          : tr('码率', 'Bit rate'): audioBitRate(
        audio,
        chinese: chinese,
      ),
    tr('文件', 'File'): item.localPath ?? '—',
    if (audio?.cuePath != null) 'CUE': audio!.cuePath!,
    if (audio?.startFrame != null && audio?.endFrame != null)
      tr('采样帧区间（结束不含）', 'Sample frames (end exclusive)'):
          '${audio!.startFrame} – ${audio.endFrame}',
  };
  showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(item.title),
      content: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (final field in fields.entries) ...[
                Text(field.key, style: Theme.of(context).textTheme.labelMedium),
                const SizedBox(height: 4),
                SelectableText(field.value.isEmpty ? '—' : field.value),
                const SizedBox(height: 14),
              ],
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: Text(tr('关闭', 'Close')),
        ),
      ],
    ),
  );
}
