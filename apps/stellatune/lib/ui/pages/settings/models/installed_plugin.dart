import 'package:path/path.dart' as p;

class InstalledPlugin {
  const InstalledPlugin({
    required this.dirPath,
    required this.id,
    required this.name,
    required this.hasWebUi,
    required this.infoJson,
    required this.installState,
    required this.uninstallRetryCount,
    required this.uninstallLastError,
  });

  final String dirPath;
  final String? id;
  final String? name;
  final bool hasWebUi;
  final String? infoJson;
  final String installState;
  final int uninstallRetryCount;
  final String? uninstallLastError;

  String get nameOrDir => name ?? p.basename(dirPath);
  bool get isInstalled => installState == 'installed';
  bool get isPendingUninstall => installState == 'pending_uninstall';
  bool get isDeleteFailed => installState == 'delete_failed';
}
