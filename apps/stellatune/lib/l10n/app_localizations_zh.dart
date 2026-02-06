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
  String get navSettings => '设置';

  @override
  String get libraryTitle => '音乐库';

  @override
  String get queueTitle => '队列';

  @override
  String get settingsTitle => '设置';

  @override
  String get settingsOutputTitle => '输出设备';

  @override
  String get settingsBackend => '音频后端';

  @override
  String get settingsBackendShared => '共享 (WASAPI Shared)';

  @override
  String get settingsBackendWasapiExclusive => '独占 (WASAPI Exclusive)';

  @override
  String get settingsBackendAsioExternal => 'ASIO (外部进程)';

  @override
  String get settingsDevice => '输出设备';

  @override
  String get settingsDeviceDefault => '系统默认';

  @override
  String settingsDeviceAutoWithName(String name) {
    return '自动选择（$name）';
  }

  @override
  String get settingsMatchTrackSampleRate => '采样率匹配曲目（独占/ASIO）';

  @override
  String get settingsGaplessPlayback => '无缝播放（优先复用输出流）';

  @override
  String get settingsPluginsTitle => '插件';

  @override
  String get settingsPluginDir => '插件目录';

  @override
  String get settingsNoPlugins => '未安装任何插件。';

  @override
  String get settingsNoLoadedPlugins => '没有加载任何插件。';

  @override
  String get settingsInstallPlugin => '安装插件';

  @override
  String get settingsInstallPluginPickFolder => '选择插件文件夹';

  @override
  String get settingsInstallPluginMissingManifest => '所选文件夹缺少 plugin.toml。';

  @override
  String get settingsPluginInstalled => '插件已安装';

  @override
  String get settingsUninstallPlugin => '卸载插件';

  @override
  String settingsUninstallPluginConfirm(String name) {
    return '确定要卸载「$name」吗？';
  }

  @override
  String get settingsPluginUninstalled => '插件已卸载';

  @override
  String get settingsUninstallPluginFailed => '卸载插件失败';

  @override
  String get settingsDspTitle => 'DSP';

  @override
  String get settingsEnableGain => '启用增益（示例插件）';

  @override
  String get settingsGain => '增益';

  @override
  String get settingsNoGainFound => '未找到 \"gain\" DSP 类型。';

  @override
  String get settingsExamplePluginNote => '这是内置的示例 DSP 插件（增益）。';

  @override
  String get settingsApplied => '已应用';

  @override
  String get apply => '应用';

  @override
  String get reset => '重置';

  @override
  String get cancel => '取消';

  @override
  String get uninstall => '卸载';

  @override
  String get refresh => '刷新';

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
  String get tooltipForceScan => '强制扫描';

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
  String get volume => '音量';

  @override
  String get tooltipVolume => '音量';

  @override
  String get tooltipShuffle => '随机播放';

  @override
  String get tooltipRepeat => '循环播放';

  @override
  String get tooltipPlayMode => '播放模式';

  @override
  String get playModeSequential => '顺序播放';

  @override
  String get playModeShuffle => '随机播放';

  @override
  String get playModeRepeatAll => '列表循环';

  @override
  String get playModeRepeatOne => '单曲循环';

  @override
  String get noLyrics => '无歌词';

  @override
  String get tooltipBack => '返回';

  @override
  String get menuMore => '更多';
}
