import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/ui/pages/library_page.dart';
import 'package:stellatune/ui/pages/queue_page.dart';
import 'package:stellatune/ui/pages/settings_page.dart';
import 'package:stellatune/ui/pages/shell/desktop_shell.dart';
import 'package:stellatune/ui/pages/shell/mobile_shell.dart';
import 'package:stellatune/ui/widgets/open_container_shader_warmup.dart';

class ShellPage extends ConsumerStatefulWidget {
  const ShellPage({super.key});

  @override
  ConsumerState<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends ConsumerState<ShellPage> {
  int _index = 0;

  @override
  Widget build(BuildContext context) {
    final body = switch (_index) {
      0 => const LibraryPage(),
      1 => const QueuePage(),
      _ => const SettingsPage(),
    };

    if (Platform.isAndroid || Platform.isIOS) {
      return MobileShell(
        selectedIndex: _index,
        onDestinationSelected: (v) => setState(() => _index = v),
        child: body,
      );
    }

    return DesktopShell(
      selectedIndex: _index,
      onDestinationSelected: (v) => setState(() => _index = v),
      child: Stack(children: [body, const OpenContainerShaderWarmup()]),
    );
  }
}
