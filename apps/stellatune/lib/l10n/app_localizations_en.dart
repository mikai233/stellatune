// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'StellaTune';

  @override
  String get openFile => 'Open File';

  @override
  String get play => 'Play';

  @override
  String get pause => 'Pause';

  @override
  String get stop => 'Stop';

  @override
  String get state => 'State';

  @override
  String get position => 'Position';

  @override
  String get track => 'Track';

  @override
  String get error => 'Error';

  @override
  String get log => 'Log';

  @override
  String get navLibrary => 'Library';

  @override
  String get navQueue => 'Queue';

  @override
  String get navSettings => 'Settings';

  @override
  String get libraryTitle => 'Library';

  @override
  String get queueTitle => 'Queue';

  @override
  String get settingsTitle => 'Settings';

  @override
  String get settingsPluginsTitle => 'Plugins';

  @override
  String get settingsPluginDir => 'Plugin dir';

  @override
  String get settingsNoPlugins => 'No plugins installed.';

  @override
  String get settingsNoLoadedPlugins => 'No plugins loaded.';

  @override
  String get settingsInstallPlugin => 'Install plugin';

  @override
  String get settingsInstallPluginPickFolder => 'Pick plugin folder';

  @override
  String get settingsInstallPluginMissingManifest =>
      'Missing plugin.toml in the selected folder.';

  @override
  String get settingsPluginInstalled => 'Plugin installed';

  @override
  String get settingsUninstallPlugin => 'Uninstall plugin';

  @override
  String settingsUninstallPluginConfirm(String name) {
    return 'Uninstall \"$name\"?';
  }

  @override
  String get settingsPluginUninstalled => 'Plugin uninstalled';

  @override
  String get settingsUninstallPluginFailed => 'Failed to uninstall plugin';

  @override
  String get settingsDspTitle => 'DSP';

  @override
  String get settingsEnableGain => 'Enable Gain (example plugin)';

  @override
  String get settingsGain => 'Gain';

  @override
  String get settingsNoGainFound => 'No \"gain\" DSP type found.';

  @override
  String get settingsExamplePluginNote =>
      'This is the built-in example DSP plugin (gain).';

  @override
  String get settingsApplied => 'Applied';

  @override
  String get apply => 'Apply';

  @override
  String get reset => 'Reset';

  @override
  String get cancel => 'Cancel';

  @override
  String get uninstall => 'Uninstall';

  @override
  String get refresh => 'Refresh';

  @override
  String get libraryAllMusic => 'All music';

  @override
  String get includeSubfolders => 'Include subfolders';

  @override
  String get expand => 'Expand';

  @override
  String get collapse => 'Collapse';

  @override
  String get tooltipAddFolder => 'Add folder';

  @override
  String get tooltipScan => 'Scan';

  @override
  String get tooltipForceScan => 'Force rescan';

  @override
  String get dialogSelectMusicFolder => 'Select music folder';

  @override
  String get searchHint => 'Search title / artist / album / path';

  @override
  String get noFoldersHint =>
      'No folders yet. Click “Add folder” to start scanning.';

  @override
  String get noResultsHint => 'No results. Add a folder and scan, then search.';

  @override
  String get scanStatusScanning => 'Scanning…';

  @override
  String get scanStatusFinished => 'Scan finished';

  @override
  String scanDurationMs(int ms) {
    return '${ms}ms';
  }

  @override
  String get scanLabelScanned => 'scanned';

  @override
  String get scanLabelUpdated => 'updated';

  @override
  String get scanLabelSkipped => 'skipped';

  @override
  String get scanLabelErrors => 'errors';

  @override
  String get menuPlay => 'Play';

  @override
  String get menuEnqueue => 'Enqueue';

  @override
  String get queueShuffle => 'Shuffle';

  @override
  String get repeatOff => 'Repeat: Off';

  @override
  String get repeatAll => 'Repeat: All';

  @override
  String get repeatOne => 'Repeat: One';

  @override
  String get nowPlayingNone => '(none)';

  @override
  String get queueEmpty => '(queue empty)';

  @override
  String get tooltipPrevious => 'Previous';

  @override
  String get tooltipNext => 'Next';

  @override
  String get volume => 'Volume';

  @override
  String get tooltipVolume => 'Volume';

  @override
  String get tooltipShuffle => 'Shuffle';

  @override
  String get tooltipRepeat => 'Repeat';

  @override
  String get tooltipPlayMode => 'Play mode';

  @override
  String get playModeSequential => 'Sequential';

  @override
  String get playModeShuffle => 'Shuffle';

  @override
  String get playModeRepeatAll => 'Repeat all';

  @override
  String get playModeRepeatOne => 'Repeat one';

  @override
  String get noLyrics => 'No lyrics';

  @override
  String get tooltipBack => 'Back';

  @override
  String get menuMore => 'More';
}
