import 'dart:io';

import 'package:stellatune/platform/directory_access_service.dart';
import 'package:stellatune/platform/directory_access_noop.dart';
import 'package:stellatune/platform/macos_directory_access.dart';

DirectoryAccessService createDirectoryAccessService() {
  if (Platform.isMacOS) {
    return MacosDirectoryAccessService();
  }
  return const NoopDirectoryAccessService();
}
