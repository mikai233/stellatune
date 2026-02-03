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
  /// **'StellaTune'**
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

  /// No description provided for @navQueue.
  ///
  /// In en, this message translates to:
  /// **'Queue'**
  String get navQueue;

  /// No description provided for @libraryTitle.
  ///
  /// In en, this message translates to:
  /// **'Library'**
  String get libraryTitle;

  /// No description provided for @queueTitle.
  ///
  /// In en, this message translates to:
  /// **'Queue'**
  String get queueTitle;

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
