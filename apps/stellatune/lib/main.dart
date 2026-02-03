import 'dart:async';
import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/platform/rust_runtime.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  await initRustRuntime();
  final bridge = await CoreBridge.create();

  runApp(MyApp(bridge: bridge));
}

class MyApp extends StatelessWidget {
  const MyApp({required this.bridge, super.key});

  final CoreBridge bridge;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'StellaTune',
      theme: ThemeData(colorSchemeSeed: Colors.indigo, useMaterial3: true),
      home: PlayerPage(bridge: bridge),
    );
  }
}

class PlayerPage extends StatefulWidget {
  const PlayerPage({required this.bridge, super.key});

  final CoreBridge bridge;

  @override
  State<PlayerPage> createState() => _PlayerPageState();
}

class _PlayerPageState extends State<PlayerPage> {
  StreamSubscription<Event>? _sub;

  PlayerState _state = PlayerState.stopped;
  String _positionMs = '0';
  String _track = '(none)';
  String? _lastError;
  String _lastLog = '';

  @override
  void initState() {
    super.initState();

    _sub = widget.bridge.events().listen((event) {
      event.when(
        stateChanged: (state) => setState(() => _state = state),
        position: (ms) => setState(() => _positionMs = ms.toString()),
        trackChanged: (path) => setState(() => _track = path),
        error: (message) => setState(() => _lastError = message),
        log: (message) => setState(() => _lastLog = message),
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
    return Scaffold(
      appBar: AppBar(title: const Text('StellaTune (Rust mock engine)')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('State: $_state'),
            Text('Position: ${_positionMs}ms'),
            Text('Track: $_track'),
            if (_lastError != null) Text('Error: $_lastError'),
            if (_lastLog.isNotEmpty) Text('Log: $_lastLog'),
            const SizedBox(height: 16),
            Wrap(
              spacing: 12,
              runSpacing: 12,
              children: [
                FilledButton(
                  onPressed: widget.bridge.play,
                  child: const Text('Play'),
                ),
                FilledButton.tonal(
                  onPressed: widget.bridge.pause,
                  child: const Text('Pause'),
                ),
                OutlinedButton(
                  onPressed: widget.bridge.stop,
                  child: const Text('Stop'),
                ),
              ],
            ),
            const SizedBox(height: 16),
            Wrap(
              spacing: 12,
              runSpacing: 12,
              children: [
                OutlinedButton(
                  onPressed: () => widget.bridge.seek(0),
                  child: const Text('Seek 0'),
                ),
                OutlinedButton(
                  onPressed: widget.bridge.next,
                  child: const Text('Next'),
                ),
                OutlinedButton(
                  onPressed: widget.bridge.previous,
                  child: const Text('Previous'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
