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
  String get libraryTitle => 'Library';

  @override
  String get queueTitle => 'Queue';

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
}
