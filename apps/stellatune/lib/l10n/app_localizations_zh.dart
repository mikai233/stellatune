// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get appTitle => '星律';

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
  String playbackUnavailable(String reason) {
    return '无法播放：$reason';
  }

  @override
  String get playabilityReasonPluginsUnavailable => '插件正在重载';

  @override
  String get playabilityReasonLocalTrackLocatorEmpty => '本地轨道路径为空';

  @override
  String get playabilityReasonNoDecoderForLocalTrack => '当前格式没有可用解码器';

  @override
  String get playabilityReasonDecoderProbeFailed => '解码器探测失败';

  @override
  String get playabilityReasonInvalidSourceTrackLocator => '音源定位信息无效';

  @override
  String get playabilityReasonSourceCatalogUnavailable => '音源插件不可用';

  @override
  String get playabilityReasonSourceDecoderUnavailable => '音源解码器不可用';

  @override
  String get playabilityReasonUnsupportedTrack => '不支持的音轨';

  @override
  String playabilityReasonUnknown(String code) {
    return '未知原因（$code）';
  }

  @override
  String get log => '日志';

  @override
  String get navLibrary => '音乐库';

  @override
  String get navPlaylists => '歌单';

  @override
  String get navSources => '来源';

  @override
  String get navQueue => '队列';

  @override
  String get navSettings => '设置';

  @override
  String get libraryTitle => '音乐库';

  @override
  String get sourcesTitle => '来源';

  @override
  String get queueTitle => '队列';

  @override
  String get queueSourceTitle => '当前队列来源';

  @override
  String get queueSourceHint => '仅在当前视图点击播放歌曲时才会更新队列来源';

  @override
  String get queueSourceUnset => '尚未设置';

  @override
  String get settingsTitle => '设置';

  @override
  String get sourcesTypeLabel => '来源类型';

  @override
  String get sourcesRefreshTypes => '刷新来源类型';

  @override
  String get sourcesConfigJsonLabel => '配置 JSON';

  @override
  String get sourcesRequestJsonLabel => '请求 JSON';

  @override
  String get sourcesLoadItems => '加载条目';

  @override
  String get sourcesNoTypes => '未加载任何来源插件。';

  @override
  String get sourcesNoItems => '没有来源条目。';

  @override
  String sourcesItemsCount(int count) {
    return '$count 条';
  }

  @override
  String get settingsOutputTitle => '输出设备';

  @override
  String get settingsBackend => '音频后端';

  @override
  String get settingsBackendShared => '共享 (WASAPI Shared)';

  @override
  String get settingsBackendWasapiExclusive => '独占 (WASAPI Exclusive)';

  @override
  String get settingsDevice => '输出设备';

  @override
  String get settingsDeviceDefault => '系统默认';

  @override
  String settingsDeviceAutoWithName(String name) {
    return '自动选择（$name）';
  }

  @override
  String get settingsMatchTrackSampleRate => '采样率匹配曲目（独占）';

  @override
  String get settingsGaplessPlayback => '无缝播放（优先复用输出流）';

  @override
  String get settingsSeekTrackFade => '播放/暂停/Seek/切歌时淡入淡出';

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
  String get settingsInstallPluginPickFolder => '选择插件文件（.zip/.dll/.so/.dylib）';

  @override
  String get settingsInstallPluginMissingManifest => '不支持的插件包。';

  @override
  String get settingsPluginInstalled => '插件已安装';

  @override
  String get settingsOpenPluginDir => '打开插件目录';

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
  String get settingsLyricsTitle => '歌词';

  @override
  String get settingsLyricsCacheSubtitle => '将在线歌词缓存到本地 SQLite，提升后续加载速度。';

  @override
  String get settingsClearLyricsCache => '清空歌词缓存';

  @override
  String get settingsClearLyricsCacheDone => '歌词缓存已清空';

  @override
  String get settingsClearLyricsCacheFailed => '清空歌词缓存失败';

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
  String get menuAddToPlaylist => '添加到歌单';

  @override
  String get menuRemoveFromCurrentPlaylist => '从当前歌单移除';

  @override
  String get playlistSectionTitle => '歌单';

  @override
  String get foldersSectionTitle => '文件夹';

  @override
  String get playlistCreateTooltip => '新建歌单';

  @override
  String get playlistCreateTitle => '新建歌单';

  @override
  String get playlistRenameTitle => '重命名歌单';

  @override
  String get playlistNameHint => '输入歌单名称';

  @override
  String get playlistDeleteTitle => '删除歌单';

  @override
  String playlistDeleteConfirm(String name) {
    return '确定要删除歌单「$name」吗？';
  }

  @override
  String get playlistRenameAction => '重命名';

  @override
  String get playlistDeleteAction => '删除';

  @override
  String playlistTrackCount(int count) {
    return '$count 首';
  }

  @override
  String get playlistEmptyHint => '还没有歌单';

  @override
  String playlistSelectionCount(int count) {
    return '已选 $count 首';
  }

  @override
  String get playlistSelectAll => '全选';

  @override
  String get playlistAllSelected => '已全选';

  @override
  String get playlistBatchAddToPlaylist => '批量添加到歌单';

  @override
  String get playlistBatchRemoveFromCurrent => '批量从当前歌单移除';

  @override
  String get likedAddTooltip => '添加到我喜欢的音乐';

  @override
  String get likedRemoveTooltip => '取消喜欢';

  @override
  String get likedPlaylistName => '我喜欢的音乐';

  @override
  String get ok => '确定';

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
  String get lyricsMoreShowLyrics => '显示歌词';

  @override
  String get lyricsMoreHideLyrics => '隐藏歌词';

  @override
  String get lyricsMoreChooseCandidate => '选择候选歌词';

  @override
  String get lyricsCandidatesLoadFailed => '加载失败';

  @override
  String get lyricsCandidatesEmpty => '没有可用候选歌词';

  @override
  String get lyricsCandidateApplied => '已应用候选歌词';

  @override
  String get lyricsCandidateApplyFailed => '应用候选歌词失败';

  @override
  String get noLyrics => '无歌词';

  @override
  String get tooltipBack => '返回';

  @override
  String get menuMore => '更多';

  @override
  String get tooltipMinimize => '最小化';

  @override
  String get tooltipMaximize => '最大化/还原';

  @override
  String get tooltipClose => '关闭';

  @override
  String get tooltipFullscreen => '全屏切换';

  @override
  String get settingsAppearanceTitle => '外观';

  @override
  String get settingsLanguage => '语言';

  @override
  String get settingsThemeMode => '主题模式';

  @override
  String get settingsThemeSystem => '跟随系统';

  @override
  String get settingsThemeLight => '浅色';

  @override
  String get settingsThemeDark => '深色';

  @override
  String get settingsLocaleSystem => '跟随系统';

  @override
  String get settingsLocaleEn => '英文';

  @override
  String get settingsLocaleZh => '中文';

  @override
  String get settingsCloseToTray => '关闭窗口时隐藏到托盘';

  @override
  String get settingsCloseToTraySubtitle => '点击关闭按钮时，将应用隐藏到系统托盘而不是完全退出';

  @override
  String get trayRestore => '显示主界面';

  @override
  String get trayExit => '完全退出';

  @override
  String get settingsSourceConfigSaved => '来源配置已保存';

  @override
  String settingsPluginInstallFailed(String error) {
    return '插件安装失败: $error';
  }

  @override
  String get settingsSinkLoadTargetsFailed => '加载输出目标失败';

  @override
  String get settingsSinkRouteCleared => '输出路由已清除';

  @override
  String get settingsSinkRouteApplied => '输出路由已应用';

  @override
  String get settingsSinkLocalDevice => '本地设备';

  @override
  String get settingsSinkPluginSink => '插件输出';

  @override
  String get settingsSinkLoadTargets => '加载目标';

  @override
  String get settingsSinkClearRoute => '清除路由';

  @override
  String get settingsSinkApplyRoute => '应用路由';

  @override
  String get deviceLocal => '本机';

  @override
  String get deviceLocalSubtitle => '本地输出';

  @override
  String get about => '关于';

  @override
  String get dlna => 'DLNA';

  @override
  String dlnaSearchFailed(String error) {
    return '发现失败: $error';
  }

  @override
  String get dlnaNoDevices => '未发现 DLNA 设备';

  @override
  String get dlnaNoDevicesSubtitle => '请确保设备与本机在同一局域网内，并关闭可能拦截组播的代理/VPN。';

  @override
  String get dlnaNoVolumeSupport => '不支持音量控制';

  @override
  String get dlnaNoAvTransportSupport => '不支持 AVTransport（无法播放）';

  @override
  String get dlnaSwitchedToLocal => '已切换到本地输出';

  @override
  String dlnaSelected(String name) {
    return '已选择 DLNA：$name';
  }
}
