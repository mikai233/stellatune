import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('zh'),
  ];

  /// No description provided for @appTitle.
  ///
  /// In en, this message translates to:
  /// **'Stellatune'**
  String get appTitle;

  /// No description provided for @openFile.
  ///
  /// In en, this message translates to:
  /// **'Open File'**
  String get openFile;

  /// No description provided for @play.
  ///
  /// In en, this message translates to:
  /// **'Play'**
  String get play;

  /// No description provided for @pause.
  ///
  /// In en, this message translates to:
  /// **'Pause'**
  String get pause;

  /// No description provided for @stop.
  ///
  /// In en, this message translates to:
  /// **'Stop'**
  String get stop;

  /// No description provided for @state.
  ///
  /// In en, this message translates to:
  /// **'State'**
  String get state;

  /// No description provided for @position.
  ///
  /// In en, this message translates to:
  /// **'Position'**
  String get position;

  /// No description provided for @track.
  ///
  /// In en, this message translates to:
  /// **'Track'**
  String get track;

  /// No description provided for @error.
  ///
  /// In en, this message translates to:
  /// **'Error'**
  String get error;

  /// No description provided for @log.
  ///
  /// In en, this message translates to:
  /// **'Log'**
  String get log;

  /// No description provided for @navLibrary.
  ///
  /// In en, this message translates to:
  /// **'Library'**
  String get navLibrary;

  /// No description provided for @navPlaylists.
  ///
  /// In en, this message translates to:
  /// **'Playlists'**
  String get navPlaylists;

  /// No description provided for @navSources.
  ///
  /// In en, this message translates to:
  /// **'Sources'**
  String get navSources;

  /// No description provided for @navQueue.
  ///
  /// In en, this message translates to:
  /// **'Queue'**
  String get navQueue;

  /// No description provided for @navSettings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get navSettings;

  /// No description provided for @libraryTitle.
  ///
  /// In en, this message translates to:
  /// **'Library'**
  String get libraryTitle;

  /// No description provided for @sourcesTitle.
  ///
  /// In en, this message translates to:
  /// **'Sources'**
  String get sourcesTitle;

  /// No description provided for @queueTitle.
  ///
  /// In en, this message translates to:
  /// **'Queue'**
  String get queueTitle;

  /// No description provided for @queueSourceTitle.
  ///
  /// In en, this message translates to:
  /// **'Current Queue Source'**
  String get queueSourceTitle;

  /// No description provided for @queueSourceHint.
  ///
  /// In en, this message translates to:
  /// **'Queue source changes only when you play a track from this view'**
  String get queueSourceHint;

  /// No description provided for @queueSourceUnset.
  ///
  /// In en, this message translates to:
  /// **'Not set yet'**
  String get queueSourceUnset;

  /// No description provided for @settingsTitle.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settingsTitle;

  /// No description provided for @sourcesTypeLabel.
  ///
  /// In en, this message translates to:
  /// **'Source type'**
  String get sourcesTypeLabel;

  /// No description provided for @sourcesRefreshTypes.
  ///
  /// In en, this message translates to:
  /// **'Reload source types'**
  String get sourcesRefreshTypes;

  /// No description provided for @sourcesConfigJsonLabel.
  ///
  /// In en, this message translates to:
  /// **'Config JSON'**
  String get sourcesConfigJsonLabel;

  /// No description provided for @sourcesRequestJsonLabel.
  ///
  /// In en, this message translates to:
  /// **'Request JSON'**
  String get sourcesRequestJsonLabel;

  /// No description provided for @sourcesLoadItems.
  ///
  /// In en, this message translates to:
  /// **'Load items'**
  String get sourcesLoadItems;

  /// No description provided for @sourcesNoTypes.
  ///
  /// In en, this message translates to:
  /// **'No source plugins loaded.'**
  String get sourcesNoTypes;

  /// No description provided for @sourcesNoItems.
  ///
  /// In en, this message translates to:
  /// **'No source items.'**
  String get sourcesNoItems;

  /// No description provided for @sourcesItemsCount.
  ///
  /// In en, this message translates to:
  /// **'{count} items'**
  String sourcesItemsCount(int count);

  /// No description provided for @settingsOutputTitle.
  ///
  /// In en, this message translates to:
  /// **'Output Device'**
  String get settingsOutputTitle;

  /// No description provided for @settingsBackend.
  ///
  /// In en, this message translates to:
  /// **'Audio Backend'**
  String get settingsBackend;

  /// No description provided for @settingsBackendShared.
  ///
  /// In en, this message translates to:
  /// **'Shared (WASAPI Shared)'**
  String get settingsBackendShared;

  /// No description provided for @settingsBackendWasapiExclusive.
  ///
  /// In en, this message translates to:
  /// **'Exclusive (WASAPI Exclusive)'**
  String get settingsBackendWasapiExclusive;

  /// No description provided for @settingsDevice.
  ///
  /// In en, this message translates to:
  /// **'Device'**
  String get settingsDevice;

  /// No description provided for @settingsDeviceDefault.
  ///
  /// In en, this message translates to:
  /// **'System Default'**
  String get settingsDeviceDefault;

  /// No description provided for @settingsDeviceAutoWithName.
  ///
  /// In en, this message translates to:
  /// **'Auto ({name})'**
  String settingsDeviceAutoWithName(String name);

  /// No description provided for @settingsMatchTrackSampleRate.
  ///
  /// In en, this message translates to:
  /// **'Match track sample rate (Exclusive)'**
  String get settingsMatchTrackSampleRate;

  /// No description provided for @settingsGaplessPlayback.
  ///
  /// In en, this message translates to:
  /// **'Gapless playback (prefer stream reuse)'**
  String get settingsGaplessPlayback;

  /// No description provided for @settingsSeekTrackFade.
  ///
  /// In en, this message translates to:
  /// **'Fade on play/pause/seek/track switch'**
  String get settingsSeekTrackFade;

  /// No description provided for @settingsPluginsTitle.
  ///
  /// In en, this message translates to:
  /// **'Plugins'**
  String get settingsPluginsTitle;

  /// No description provided for @settingsPluginDir.
  ///
  /// In en, this message translates to:
  /// **'Plugin dir'**
  String get settingsPluginDir;

  /// No description provided for @settingsNoPlugins.
  ///
  /// In en, this message translates to:
  /// **'No plugins installed.'**
  String get settingsNoPlugins;

  /// No description provided for @settingsNoLoadedPlugins.
  ///
  /// In en, this message translates to:
  /// **'No plugins loaded.'**
  String get settingsNoLoadedPlugins;

  /// No description provided for @settingsInstallPlugin.
  ///
  /// In en, this message translates to:
  /// **'Install plugin'**
  String get settingsInstallPlugin;

  /// No description provided for @settingsInstallPluginPickFolder.
  ///
  /// In en, this message translates to:
  /// **'Pick plugin file (.zip/.dll/.so/.dylib)'**
  String get settingsInstallPluginPickFolder;

  /// No description provided for @settingsInstallPluginMissingManifest.
  ///
  /// In en, this message translates to:
  /// **'Unsupported plugin package.'**
  String get settingsInstallPluginMissingManifest;

  /// No description provided for @settingsPluginInstalled.
  ///
  /// In en, this message translates to:
  /// **'Plugin installed'**
  String get settingsPluginInstalled;

  /// No description provided for @settingsOpenPluginDir.
  ///
  /// In en, this message translates to:
  /// **'Open plugin directory'**
  String get settingsOpenPluginDir;

  /// No description provided for @settingsUninstallPlugin.
  ///
  /// In en, this message translates to:
  /// **'Uninstall plugin'**
  String get settingsUninstallPlugin;

  /// No description provided for @settingsUninstallPluginConfirm.
  ///
  /// In en, this message translates to:
  /// **'Uninstall \"{name}\"?'**
  String settingsUninstallPluginConfirm(String name);

  /// No description provided for @settingsPluginUninstalled.
  ///
  /// In en, this message translates to:
  /// **'Plugin uninstalled'**
  String get settingsPluginUninstalled;

  /// No description provided for @settingsUninstallPluginFailed.
  ///
  /// In en, this message translates to:
  /// **'Failed to uninstall plugin'**
  String get settingsUninstallPluginFailed;

  /// No description provided for @settingsDspTitle.
  ///
  /// In en, this message translates to:
  /// **'DSP'**
  String get settingsDspTitle;

  /// No description provided for @settingsLyricsTitle.
  ///
  /// In en, this message translates to:
  /// **'Lyrics'**
  String get settingsLyricsTitle;

  /// No description provided for @settingsLyricsCacheSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Store online lyrics in local SQLite cache for faster loading.'**
  String get settingsLyricsCacheSubtitle;

  /// No description provided for @settingsClearLyricsCache.
  ///
  /// In en, this message translates to:
  /// **'Clear lyrics cache'**
  String get settingsClearLyricsCache;

  /// No description provided for @settingsClearLyricsCacheDone.
  ///
  /// In en, this message translates to:
  /// **'Lyrics cache cleared'**
  String get settingsClearLyricsCacheDone;

  /// No description provided for @settingsClearLyricsCacheFailed.
  ///
  /// In en, this message translates to:
  /// **'Failed to clear lyrics cache'**
  String get settingsClearLyricsCacheFailed;

  /// No description provided for @settingsEnableGain.
  ///
  /// In en, this message translates to:
  /// **'Enable Gain (example plugin)'**
  String get settingsEnableGain;

  /// No description provided for @settingsGain.
  ///
  /// In en, this message translates to:
  /// **'Gain'**
  String get settingsGain;

  /// No description provided for @settingsNoGainFound.
  ///
  /// In en, this message translates to:
  /// **'No \"gain\" DSP type found.'**
  String get settingsNoGainFound;

  /// No description provided for @settingsExamplePluginNote.
  ///
  /// In en, this message translates to:
  /// **'This is the built-in example DSP plugin (gain).'**
  String get settingsExamplePluginNote;

  /// No description provided for @settingsApplied.
  ///
  /// In en, this message translates to:
  /// **'Applied'**
  String get settingsApplied;

  /// No description provided for @apply.
  ///
  /// In en, this message translates to:
  /// **'Apply'**
  String get apply;

  /// No description provided for @reset.
  ///
  /// In en, this message translates to:
  /// **'Reset'**
  String get reset;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @uninstall.
  ///
  /// In en, this message translates to:
  /// **'Uninstall'**
  String get uninstall;

  /// No description provided for @refresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get refresh;

  /// No description provided for @libraryAllMusic.
  ///
  /// In en, this message translates to:
  /// **'All music'**
  String get libraryAllMusic;

  /// No description provided for @includeSubfolders.
  ///
  /// In en, this message translates to:
  /// **'Include subfolders'**
  String get includeSubfolders;

  /// No description provided for @expand.
  ///
  /// In en, this message translates to:
  /// **'Expand'**
  String get expand;

  /// No description provided for @collapse.
  ///
  /// In en, this message translates to:
  /// **'Collapse'**
  String get collapse;

  /// No description provided for @tooltipAddFolder.
  ///
  /// In en, this message translates to:
  /// **'Add folder'**
  String get tooltipAddFolder;

  /// No description provided for @tooltipScan.
  ///
  /// In en, this message translates to:
  /// **'Scan'**
  String get tooltipScan;

  /// No description provided for @tooltipForceScan.
  ///
  /// In en, this message translates to:
  /// **'Force rescan'**
  String get tooltipForceScan;

  /// No description provided for @dialogSelectMusicFolder.
  ///
  /// In en, this message translates to:
  /// **'Select music folder'**
  String get dialogSelectMusicFolder;

  /// No description provided for @searchHint.
  ///
  /// In en, this message translates to:
  /// **'Search title / artist / album / path'**
  String get searchHint;

  /// No description provided for @noFoldersHint.
  ///
  /// In en, this message translates to:
  /// **'No folders yet. Click “Add folder” to start scanning.'**
  String get noFoldersHint;

  /// No description provided for @noResultsHint.
  ///
  /// In en, this message translates to:
  /// **'No results. Add a folder and scan, then search.'**
  String get noResultsHint;

  /// No description provided for @scanStatusScanning.
  ///
  /// In en, this message translates to:
  /// **'Scanning…'**
  String get scanStatusScanning;

  /// No description provided for @scanStatusFinished.
  ///
  /// In en, this message translates to:
  /// **'Scan finished'**
  String get scanStatusFinished;

  /// No description provided for @scanDurationMs.
  ///
  /// In en, this message translates to:
  /// **'{ms}ms'**
  String scanDurationMs(int ms);

  /// No description provided for @scanLabelScanned.
  ///
  /// In en, this message translates to:
  /// **'scanned'**
  String get scanLabelScanned;

  /// No description provided for @scanLabelUpdated.
  ///
  /// In en, this message translates to:
  /// **'updated'**
  String get scanLabelUpdated;

  /// No description provided for @scanLabelSkipped.
  ///
  /// In en, this message translates to:
  /// **'skipped'**
  String get scanLabelSkipped;

  /// No description provided for @scanLabelErrors.
  ///
  /// In en, this message translates to:
  /// **'errors'**
  String get scanLabelErrors;

  /// No description provided for @menuPlay.
  ///
  /// In en, this message translates to:
  /// **'Play'**
  String get menuPlay;

  /// No description provided for @menuEnqueue.
  ///
  /// In en, this message translates to:
  /// **'Enqueue'**
  String get menuEnqueue;

  /// No description provided for @menuAddToPlaylist.
  ///
  /// In en, this message translates to:
  /// **'Add to playlist'**
  String get menuAddToPlaylist;

  /// No description provided for @menuRemoveFromCurrentPlaylist.
  ///
  /// In en, this message translates to:
  /// **'Remove from current playlist'**
  String get menuRemoveFromCurrentPlaylist;

  /// No description provided for @playlistSectionTitle.
  ///
  /// In en, this message translates to:
  /// **'Playlists'**
  String get playlistSectionTitle;

  /// No description provided for @foldersSectionTitle.
  ///
  /// In en, this message translates to:
  /// **'Folders'**
  String get foldersSectionTitle;

  /// No description provided for @playlistCreateTooltip.
  ///
  /// In en, this message translates to:
  /// **'Create playlist'**
  String get playlistCreateTooltip;

  /// No description provided for @playlistCreateTitle.
  ///
  /// In en, this message translates to:
  /// **'Create playlist'**
  String get playlistCreateTitle;

  /// No description provided for @playlistRenameTitle.
  ///
  /// In en, this message translates to:
  /// **'Rename playlist'**
  String get playlistRenameTitle;

  /// No description provided for @playlistNameHint.
  ///
  /// In en, this message translates to:
  /// **'Enter playlist name'**
  String get playlistNameHint;

  /// No description provided for @playlistDeleteTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete playlist'**
  String get playlistDeleteTitle;

  /// No description provided for @playlistDeleteConfirm.
  ///
  /// In en, this message translates to:
  /// **'Delete playlist \"{name}\"?'**
  String playlistDeleteConfirm(String name);

  /// No description provided for @playlistRenameAction.
  ///
  /// In en, this message translates to:
  /// **'Rename'**
  String get playlistRenameAction;

  /// No description provided for @playlistDeleteAction.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get playlistDeleteAction;

  /// No description provided for @playlistTrackCount.
  ///
  /// In en, this message translates to:
  /// **'{count} tracks'**
  String playlistTrackCount(int count);

  /// No description provided for @playlistEmptyHint.
  ///
  /// In en, this message translates to:
  /// **'No playlists yet'**
  String get playlistEmptyHint;

  /// No description provided for @playlistSelectionCount.
  ///
  /// In en, this message translates to:
  /// **'{count} selected'**
  String playlistSelectionCount(int count);

  /// No description provided for @playlistSelectAll.
  ///
  /// In en, this message translates to:
  /// **'Select all'**
  String get playlistSelectAll;

  /// No description provided for @playlistAllSelected.
  ///
  /// In en, this message translates to:
  /// **'All selected'**
  String get playlistAllSelected;

  /// No description provided for @playlistBatchAddToPlaylist.
  ///
  /// In en, this message translates to:
  /// **'Add selected to playlist'**
  String get playlistBatchAddToPlaylist;

  /// No description provided for @playlistBatchRemoveFromCurrent.
  ///
  /// In en, this message translates to:
  /// **'Remove selected from current playlist'**
  String get playlistBatchRemoveFromCurrent;

  /// No description provided for @likedAddTooltip.
  ///
  /// In en, this message translates to:
  /// **'Add to Liked Music'**
  String get likedAddTooltip;

  /// No description provided for @likedRemoveTooltip.
  ///
  /// In en, this message translates to:
  /// **'Remove from Liked Music'**
  String get likedRemoveTooltip;

  /// No description provided for @likedPlaylistName.
  ///
  /// In en, this message translates to:
  /// **'Liked Music'**
  String get likedPlaylistName;

  /// No description provided for @ok.
  ///
  /// In en, this message translates to:
  /// **'OK'**
  String get ok;

  /// No description provided for @queueShuffle.
  ///
  /// In en, this message translates to:
  /// **'Shuffle'**
  String get queueShuffle;

  /// No description provided for @repeatOff.
  ///
  /// In en, this message translates to:
  /// **'Repeat: Off'**
  String get repeatOff;

  /// No description provided for @repeatAll.
  ///
  /// In en, this message translates to:
  /// **'Repeat: All'**
  String get repeatAll;

  /// No description provided for @repeatOne.
  ///
  /// In en, this message translates to:
  /// **'Repeat: One'**
  String get repeatOne;

  /// No description provided for @nowPlayingNone.
  ///
  /// In en, this message translates to:
  /// **'(none)'**
  String get nowPlayingNone;

  /// No description provided for @queueEmpty.
  ///
  /// In en, this message translates to:
  /// **'(queue empty)'**
  String get queueEmpty;

  /// No description provided for @tooltipPrevious.
  ///
  /// In en, this message translates to:
  /// **'Previous'**
  String get tooltipPrevious;

  /// No description provided for @tooltipNext.
  ///
  /// In en, this message translates to:
  /// **'Next'**
  String get tooltipNext;

  /// No description provided for @volume.
  ///
  /// In en, this message translates to:
  /// **'Volume'**
  String get volume;

  /// No description provided for @tooltipVolume.
  ///
  /// In en, this message translates to:
  /// **'Volume'**
  String get tooltipVolume;

  /// No description provided for @tooltipShuffle.
  ///
  /// In en, this message translates to:
  /// **'Shuffle'**
  String get tooltipShuffle;

  /// No description provided for @tooltipRepeat.
  ///
  /// In en, this message translates to:
  /// **'Repeat'**
  String get tooltipRepeat;

  /// No description provided for @tooltipPlayMode.
  ///
  /// In en, this message translates to:
  /// **'Play mode'**
  String get tooltipPlayMode;

  /// No description provided for @playModeSequential.
  ///
  /// In en, this message translates to:
  /// **'Sequential'**
  String get playModeSequential;

  /// No description provided for @playModeShuffle.
  ///
  /// In en, this message translates to:
  /// **'Shuffle'**
  String get playModeShuffle;

  /// No description provided for @playModeRepeatAll.
  ///
  /// In en, this message translates to:
  /// **'Repeat all'**
  String get playModeRepeatAll;

  /// No description provided for @playModeRepeatOne.
  ///
  /// In en, this message translates to:
  /// **'Repeat one'**
  String get playModeRepeatOne;

  /// No description provided for @lyricsMoreShowLyrics.
  ///
  /// In en, this message translates to:
  /// **'Show lyrics'**
  String get lyricsMoreShowLyrics;

  /// No description provided for @lyricsMoreHideLyrics.
  ///
  /// In en, this message translates to:
  /// **'Hide lyrics'**
  String get lyricsMoreHideLyrics;

  /// No description provided for @lyricsMoreChooseCandidate.
  ///
  /// In en, this message translates to:
  /// **'Choose lyrics candidate'**
  String get lyricsMoreChooseCandidate;

  /// No description provided for @lyricsCandidatesLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Failed to load'**
  String get lyricsCandidatesLoadFailed;

  /// No description provided for @lyricsCandidatesEmpty.
  ///
  /// In en, this message translates to:
  /// **'No candidate lyrics found'**
  String get lyricsCandidatesEmpty;

  /// No description provided for @lyricsCandidateApplied.
  ///
  /// In en, this message translates to:
  /// **'Lyrics candidate applied'**
  String get lyricsCandidateApplied;

  /// No description provided for @lyricsCandidateApplyFailed.
  ///
  /// In en, this message translates to:
  /// **'Failed to apply lyrics candidate'**
  String get lyricsCandidateApplyFailed;

  /// No description provided for @noLyrics.
  ///
  /// In en, this message translates to:
  /// **'No lyrics'**
  String get noLyrics;

  /// No description provided for @tooltipBack.
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get tooltipBack;

  /// No description provided for @menuMore.
  ///
  /// In en, this message translates to:
  /// **'More'**
  String get menuMore;

  /// No description provided for @tooltipMinimize.
  ///
  /// In en, this message translates to:
  /// **'Minimize'**
  String get tooltipMinimize;

  /// No description provided for @tooltipMaximize.
  ///
  /// In en, this message translates to:
  /// **'Maximize'**
  String get tooltipMaximize;

  /// No description provided for @tooltipClose.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get tooltipClose;

  /// No description provided for @tooltipFullscreen.
  ///
  /// In en, this message translates to:
  /// **'Toggle Fullscreen'**
  String get tooltipFullscreen;

  /// No description provided for @settingsAppearanceTitle.
  ///
  /// In en, this message translates to:
  /// **'Appearance'**
  String get settingsAppearanceTitle;

  /// No description provided for @settingsLanguage.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get settingsLanguage;

  /// No description provided for @settingsThemeMode.
  ///
  /// In en, this message translates to:
  /// **'Theme Mode'**
  String get settingsThemeMode;

  /// No description provided for @settingsThemeSystem.
  ///
  /// In en, this message translates to:
  /// **'Follow System'**
  String get settingsThemeSystem;

  /// No description provided for @settingsThemeLight.
  ///
  /// In en, this message translates to:
  /// **'Light'**
  String get settingsThemeLight;

  /// No description provided for @settingsThemeDark.
  ///
  /// In en, this message translates to:
  /// **'Dark'**
  String get settingsThemeDark;

  /// No description provided for @settingsLocaleSystem.
  ///
  /// In en, this message translates to:
  /// **'Follow System'**
  String get settingsLocaleSystem;

  /// No description provided for @settingsLocaleEn.
  ///
  /// In en, this message translates to:
  /// **'English'**
  String get settingsLocaleEn;

  /// No description provided for @settingsLocaleZh.
  ///
  /// In en, this message translates to:
  /// **'Chinese'**
  String get settingsLocaleZh;

  /// No description provided for @settingsCloseToTray.
  ///
  /// In en, this message translates to:
  /// **'Minimize to tray on close'**
  String get settingsCloseToTray;

  /// No description provided for @settingsCloseToTraySubtitle.
  ///
  /// In en, this message translates to:
  /// **'Hide the window to system tray when clicking the close button instead of exiting'**
  String get settingsCloseToTraySubtitle;

  /// No description provided for @trayRestore.
  ///
  /// In en, this message translates to:
  /// **'Restore'**
  String get trayRestore;

  /// No description provided for @trayExit.
  ///
  /// In en, this message translates to:
  /// **'Exit'**
  String get trayExit;

  /// No description provided for @settingsSourceConfigSaved.
  ///
  /// In en, this message translates to:
  /// **'Source config saved'**
  String get settingsSourceConfigSaved;

  /// No description provided for @settingsPluginInstallFailed.
  ///
  /// In en, this message translates to:
  /// **'Failed to install plugin: {error}'**
  String settingsPluginInstallFailed(String error);

  /// No description provided for @settingsSinkLoadTargetsFailed.
  ///
  /// In en, this message translates to:
  /// **'Failed to load output sink targets'**
  String get settingsSinkLoadTargetsFailed;

  /// No description provided for @settingsSinkRouteCleared.
  ///
  /// In en, this message translates to:
  /// **'Output sink route cleared'**
  String get settingsSinkRouteCleared;

  /// No description provided for @settingsSinkRouteApplied.
  ///
  /// In en, this message translates to:
  /// **'Output sink route applied'**
  String get settingsSinkRouteApplied;

  /// No description provided for @settingsSinkLocalDevice.
  ///
  /// In en, this message translates to:
  /// **'Local Device'**
  String get settingsSinkLocalDevice;

  /// No description provided for @settingsSinkPluginSink.
  ///
  /// In en, this message translates to:
  /// **'Plugin Sink'**
  String get settingsSinkPluginSink;

  /// No description provided for @settingsSinkLoadTargets.
  ///
  /// In en, this message translates to:
  /// **'Load Targets'**
  String get settingsSinkLoadTargets;

  /// No description provided for @settingsSinkClearRoute.
  ///
  /// In en, this message translates to:
  /// **'Clear Route'**
  String get settingsSinkClearRoute;

  /// No description provided for @settingsSinkApplyRoute.
  ///
  /// In en, this message translates to:
  /// **'Apply Route'**
  String get settingsSinkApplyRoute;

  /// No description provided for @deviceLocal.
  ///
  /// In en, this message translates to:
  /// **'Local Device'**
  String get deviceLocal;

  /// No description provided for @deviceLocalSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Local Output'**
  String get deviceLocalSubtitle;

  /// No description provided for @about.
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get about;

  /// No description provided for @dlna.
  ///
  /// In en, this message translates to:
  /// **'DLNA'**
  String get dlna;

  /// No description provided for @dlnaSearchFailed.
  ///
  /// In en, this message translates to:
  /// **'Discovery failed: {error}'**
  String dlnaSearchFailed(String error);

  /// No description provided for @dlnaNoDevices.
  ///
  /// In en, this message translates to:
  /// **'No DLNA devices found'**
  String get dlnaNoDevices;

  /// No description provided for @dlnaNoDevicesSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Ensure devices are on same network and multicast is allowed.'**
  String get dlnaNoDevicesSubtitle;

  /// No description provided for @dlnaNoVolumeSupport.
  ///
  /// In en, this message translates to:
  /// **'No volume control support'**
  String get dlnaNoVolumeSupport;

  /// No description provided for @dlnaNoAvTransportSupport.
  ///
  /// In en, this message translates to:
  /// **'AVTransport not supported (cannot play)'**
  String get dlnaNoAvTransportSupport;

  /// No description provided for @dlnaSwitchedToLocal.
  ///
  /// In en, this message translates to:
  /// **'Switched to local output'**
  String get dlnaSwitchedToLocal;

  /// No description provided for @dlnaSelected.
  ///
  /// In en, this message translates to:
  /// **'Selected DLNA: {name}'**
  String dlnaSelected(String name);
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'zh'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
