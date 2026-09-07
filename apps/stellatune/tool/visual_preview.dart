import 'package:stellatune/ui/theme/desktop_theme.dart';

import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:stellatune/ui/preview/home_preview.dart';
import 'package:stellatune/ui/preview/library_preview.dart';
import 'package:stellatune/ui/pages/library/desktop_library_view.dart';
import 'package:window_manager/window_manager.dart';

/// flutter run -d windows -t tool/visual_preview.dart
/// Add --dart-define=VISUAL_CAPTURE=true for an automatic native PNG capture.
Future<void> main() async {
  final binding = WidgetsFlutterBinding.ensureInitialized();
  const page = String.fromEnvironment('VISUAL_PAGE', defaultValue: 'home');
  const themeName = String.fromEnvironment(
    'VISUAL_THEME',
    defaultValue: 'daylight',
  );
  final preset = DesktopThemePreset.values.firstWhere(
    (p) => p.name == themeName,
  );
  final library = page.startsWith('library');
  if (library) await LibraryVisualPreview.prepareCovers();
  await windowManager.ensureInitialized();
  await windowManager.waitUntilReadyToShow(
    const WindowOptions(
      size: Size(1536, 1024),
      minimumSize: Size(900, 700),
      title: 'Stellatune · Visual preview',
      titleBarStyle: TitleBarStyle.hidden,
    ),
    () async {
      await windowManager.show();
    },
  );
  const key = ValueKey('native-visual-root');
  runApp(
    RepaintBoundary(
      key: key,
      child: HomeVisualPreview(
        desktopTheme: preset,
        initialDestination: page == 'settings'
            ? 3
            : library
            ? 1
            : 0,
        contentBuilder: library
            ? (_) => LibraryVisualPreview(
                initialSection: page == 'library-albums'
                    ? LibrarySection.albums
                    : LibrarySection.songs,
              )
            : null,
        onMinimize: () => windowManager.minimize(),
        onMaximize: () async {
          if (await windowManager.isMaximized()) {
            await windowManager.unmaximize();
          } else {
            await windowManager.maximize();
          }
        },
        onClose: () => windowManager.close(),
        onDrag: () => windowManager.startDragging(),
      ),
    ),
  );
  if (const bool.fromEnvironment('VISUAL_CAPTURE')) {
    await Future<void>.delayed(const Duration(seconds: 3));
    await binding.endOfFrame;
    RenderRepaintBoundary? root;
    void visit(Element element) {
      if (element.widget.key == key) {
        root = element.findRenderObject() as RenderRepaintBoundary;
      }
      element.visitChildren(visit);
    }

    visit(binding.rootElement!);
    final image = await root!.toImage(pixelRatio: 1);
    final data = await image.toByteData(format: ui.ImageByteFormat.png);
    image.dispose();
    final dir = await Directory('build/visual-review').create(recursive: true);
    final file = File('${dir.path}/$page-windows-native.png');
    await file.writeAsBytes(data!.buffer.asUint8List());
    debugPrint('VISUAL_CAPTURE ${file.absolute.path}');
    exit(0);
  }
}
