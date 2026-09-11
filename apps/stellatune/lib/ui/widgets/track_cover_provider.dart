import 'dart:io';

import 'package:flutter/painting.dart';

/// Shared decode/cache key for small library covers and the playback bar.
ImageProvider localTrackCoverProvider(String coverDir, int trackId) =>
    ResizeImage(
      FileImage(File('$coverDir${Platform.pathSeparator}$trackId')),
      width: 96,
      height: 96,
      allowUpscaling: false,
    );
