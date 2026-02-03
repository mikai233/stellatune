import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/platform/rust_runtime.dart';
import 'package:file_picker/file_picker.dart';
import 'package:stellatune/l10n/app_localizations.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  await initRustRuntime();
  final bridge = await PlayerBridge.create();

  runApp(
    ProviderScope(
      overrides: [playerBridgeProvider.overrideWithValue(bridge)],
      child: const MyApp(),
    ),
  );
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      onGenerateTitle: (context) => AppLocalizations.of(context)!.appTitle,
      theme: ThemeData(colorSchemeSeed: Colors.indigo, useMaterial3: true),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: const PlayerPage(),
    );
  }
}

class PlayerPage extends ConsumerStatefulWidget {
  const PlayerPage({super.key});

  @override
  ConsumerState<PlayerPage> createState() => _PlayerPageState();
}

class _PlayerPageState extends ConsumerState<PlayerPage> {
  StreamSubscription<Event>? _sub;

  PlayerState _state = PlayerState.stopped;
  String _positionMs = '0';
  String _track = '(none)';
  String? _lastError;
  String _lastLog = '';

  @override
  void initState() {
    super.initState();

    final bridge = ref.read(playerBridgeProvider);
    _sub = bridge.events().listen((event) {
      event.when(
        stateChanged: (state) {
          ref.read(loggerProvider).d('state = $state');
          setState(() => _state = state);
        },
        position: (ms) => setState(() => _positionMs = ms.toString()),
        trackChanged: (path) {
          ref.read(loggerProvider).i('track = $path');
          setState(() => _track = path);
        },
        error: (message) {
          ref.read(loggerProvider).e(message);
          setState(() => _lastError = message);
        },
        log: (message) {
          ref.read(loggerProvider).d(message);
          setState(() => _lastLog = message);
        },
      );
    });
  }

  @override
  void dispose() {
    unawaited(_sub?.cancel());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final bridge = ref.read(playerBridgeProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.appTitle)),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('${l10n.state}: $_state'),
            Text('${l10n.position}: ${_positionMs}ms'),
            Text('${l10n.track}: $_track'),
            if (_lastError != null) Text('${l10n.error}: $_lastError'),
            if (_lastLog.isNotEmpty) Text('${l10n.log}: $_lastLog'),
            const SizedBox(height: 16),
            Wrap(
              spacing: 12,
              runSpacing: 12,
              children: [
                FilledButton.tonal(
                  onPressed: _pickAndLoad,
                  child: Text(l10n.openFile),
                ),
                FilledButton(onPressed: bridge.play, child: Text(l10n.play)),
                FilledButton.tonal(
                  onPressed: bridge.pause,
                  child: Text(l10n.pause),
                ),
                OutlinedButton(onPressed: bridge.stop, child: Text(l10n.stop)),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _pickAndLoad() async {
    final result = await FilePicker.platform.pickFiles(
      type: FileType.custom,
      allowedExtensions: const ['mp3', 'flac', 'wav'],
    );

    final path = result?.files.single.path;
    if (path == null) return;

    setState(() {
      _lastError = null;
      _lastLog = '';
    });

    final bridge = ref.read(playerBridgeProvider);
    await bridge.load(path);
  }
}
