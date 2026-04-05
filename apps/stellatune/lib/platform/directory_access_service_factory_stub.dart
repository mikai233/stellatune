import 'package:stellatune/platform/directory_access_noop.dart';
import 'package:stellatune/platform/directory_access_service.dart';

DirectoryAccessService createDirectoryAccessService() =>
    const NoopDirectoryAccessService();
