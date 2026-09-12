import 'package:flutter/material.dart';
import 'package:stellatune/app/app_bootstrap.dart';
import 'package:stellatune/bridge/api/library.dart' as library_api;
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/ui/diagnostics/diagnostics_overlay.dart';

/// Paint before loading platform services, with an exit path on startup failure.
class StartupApp extends StatefulWidget {
  const StartupApp({
    super.key,
    required this.loadApp,
    required this.onExit,
    this.rebuildLibrary,
  });

  final Future<Widget> Function() loadApp;
  final Future<void> Function() onExit;
  final Future<void> Function(String dbPath)? rebuildLibrary;

  @override
  State<StartupApp> createState() => _StartupAppState();
}

class _StartupAppState extends State<StartupApp> {
  Widget? _app;
  Object? _error;
  bool _rebuilt = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final app = await widget.loadApp();
      if (mounted) setState(() => _app = app);
    } catch (error, stack) {
      logger.e('application startup failed', error: error, stackTrace: stack);
      DiagnosticsService.instance.report(
        error,
        stack: stack,
        operation: 'startup',
        notify: false,
      );
      if (mounted) setState(() => _error = error);
    }
  }

  Future<void> _rebuild(LibraryRebuildRequired error) async {
    setState(() => _error = null);
    try {
      if (widget.rebuildLibrary case final rebuild?) {
        await rebuild(error.dbPath);
      } else {
        await library_api.libraryRebuild(dbPath: error.dbPath);
      }
      // Runtime singletons have already shut down after bootstrap failure.
      // Restarting avoids reusing disposed native services.
      if (mounted) {
        setState(() {
          _rebuilt = true;
          _error = StateError('rebuilt');
        });
      }
    } catch (error, stack) {
      DiagnosticsService.instance.report(
        error,
        stack: stack,
        operation: 'library_rebuild',
        notify: false,
      );
      if (mounted) setState(() => _error = error);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_app != null) return _app!;
    final chinese =
        WidgetsBinding.instance.platformDispatcher.locale.languageCode == 'zh';
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      builder: (context, child) =>
          DiagnosticsOverlay(child: child ?? const SizedBox.shrink()),
      theme: ThemeData(colorSchemeSeed: const Color(0xff596d84)),
      home: Scaffold(
        body: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 560),
            child: Padding(
              padding: const EdgeInsets.all(32),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (_error == null)
                    const CircularProgressIndicator()
                  else
                    Icon(
                      _rebuilt
                          ? Icons.check_circle_outline
                          : Icons.error_outline,
                      size: 40,
                    ),
                  const SizedBox(height: 24),
                  Text(
                    _rebuilt
                        ? (chinese ? '音乐库已重建' : 'Library rebuilt')
                        : _error == null
                        ? (chinese
                              ? '正在启动 Stellatune…'
                              : 'Starting Stellatune…')
                        : (chinese
                              ? '无法启动 Stellatune'
                              : 'Unable to start Stellatune'),
                    style: const TextStyle(fontSize: 22),
                  ),
                  if (_error != null) ...[
                    const SizedBox(height: 16),
                    if (_error case final LibraryRebuildRequired rebuild) ...[
                      Text(
                        chinese
                            ? 'CUE 分轨需要重建音乐库。曲目、歌单关联、收藏和播放队列将被清空；扫描目录、设置和插件配置会保留。重启后重新扫描即可。'
                            : 'CUE support requires rebuilding the library. Tracks, playlist links, favorites and playback state will be cleared. Scan folders, settings and plugin configuration are retained. Scan again after restarting.',
                      ),
                      const SizedBox(height: 16),
                      FilledButton(
                        onPressed: () => _rebuild(rebuild),
                        child: Text(chinese ? '重建音乐库' : 'Rebuild library'),
                      ),
                    ] else if (_rebuilt)
                      Text(
                        chinese ? '音乐库已重建，请重新启动 Stellatune 后扫描。' : 'Library rebuilt. Restart Stellatune and scan your folders.',
                      ),
                    ConstrainedBox(
                      constraints: const BoxConstraints(maxHeight: 200),
                      child: SingleChildScrollView(
                        child: TextButton(
                          onPressed: DiagnosticsService.instance.open,
                          child: Text(chinese ? '查看日志' : 'View logs'),
                        ),
                      ),
                    ),
                  ],
                  const SizedBox(height: 24),
                  OutlinedButton(
                    onPressed: widget.onExit,
                    child: Text(chinese ? '退出' : 'Exit'),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
