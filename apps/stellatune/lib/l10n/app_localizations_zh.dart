// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get appTitle => 'StellaTune';

  @override
  String get openFile => '打开文件';

  @override
  String get play => '播放';

  @override
  String get pause => '暂停';

  @override
  String get stop => '停止';

  @override
  String get state => '状态';

  @override
  String get position => '进度';

  @override
  String get track => '曲目';

  @override
  String get error => '错误';

  @override
  String get log => '日志';

  @override
  String get navLibrary => '音乐库';

  @override
  String get navQueue => '队列';

  @override
  String get libraryTitle => '音乐库';

  @override
  String get queueTitle => '队列';

  @override
  String get libraryAllMusic => '全部音乐';

  @override
  String get includeSubfolders => '包含子文件夹';

  @override
  String get expand => '展开';

  @override
  String get collapse => '折叠';

  @override
  String get tooltipAddFolder => '添加文件夹';

  @override
  String get tooltipScan => '扫描';

  @override
  String get dialogSelectMusicFolder => '选择音乐文件夹';

  @override
  String get searchHint => '搜索 标题 / 歌手 / 专辑 / 路径';

  @override
  String get noFoldersHint => '还没有添加文件夹，点击“添加文件夹”开始扫描。';

  @override
  String get noResultsHint => '没有结果。先添加文件夹并扫描，然后再搜索。';

  @override
  String get scanStatusScanning => '扫描中…';

  @override
  String get scanStatusFinished => '扫描完成';

  @override
  String scanDurationMs(int ms) {
    return '$ms毫秒';
  }

  @override
  String get scanLabelScanned => '已扫描';

  @override
  String get scanLabelUpdated => '已更新';

  @override
  String get scanLabelSkipped => '已跳过';

  @override
  String get scanLabelErrors => '错误';

  @override
  String get menuPlay => '播放';

  @override
  String get menuEnqueue => '加入队列';

  @override
  String get queueShuffle => '随机';

  @override
  String get repeatOff => '循环：关';

  @override
  String get repeatAll => '循环：全部';

  @override
  String get repeatOne => '循环：单曲';

  @override
  String get nowPlayingNone => '(无)';

  @override
  String get queueEmpty => '(队列为空)';

  @override
  String get tooltipPrevious => '上一首';

  @override
  String get tooltipNext => '下一首';

  @override
  String get tooltipShuffle => '随机播放';

  @override
  String get tooltipRepeat => '循环播放';
}
