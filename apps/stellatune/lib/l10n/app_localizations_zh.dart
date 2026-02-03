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
}
