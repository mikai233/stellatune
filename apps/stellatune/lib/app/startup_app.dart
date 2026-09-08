import 'package:flutter/material.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/ui/diagnostics/diagnostics_overlay.dart';

/// Paint before loading platform services, with an exit path on startup failure.
class StartupApp extends StatefulWidget {
  const StartupApp({super.key, required this.loadApp, required this.onExit});

  final Future<Widget> Function() loadApp;
  final Future<void> Function() onExit;

  @override
  State<StartupApp> createState() => _StartupAppState();
}

class _StartupAppState extends State<StartupApp> {
  Widget? _app;
  Object? _error;

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
                    const Icon(Icons.error_outline, size: 40),
                  const SizedBox(height: 24),
                  Text(
                    _error == null
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
