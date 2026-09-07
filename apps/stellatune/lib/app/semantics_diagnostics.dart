import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

/// Dumps the current Dart semantics tree and its widget owners on F8.
///
/// Native AXTree errors do not pass through FlutterError.onError. Capture this
/// in the same app session as the native error: node IDs change after restart.
void installSemanticsDiagnostics() {
  if (!kDebugMode) return;
  var writing = false;
  HardwareKeyboard.instance.addHandler((event) {
    if (event is! KeyDownEvent || event.logicalKey != LogicalKeyboardKey.f8) {
      return false;
    }
    if (writing) return true;
    writing = true;
    final binding = WidgetsBinding.instance;
    binding.addPostFrameCallback((_) async {
      try {
        final report = StringBuffer()
          ..writeln(
            'Stellatune semantics snapshot: ${DateTime.now().toIso8601String()}',
          )
          ..writeln('PID: $pid; semanticsEnabled: ${binding.semanticsEnabled}')
          ..writeln('This is the Dart tree, not the native Windows AXTree.');
        for (final view in binding.renderViews) {
          report.writeln('\nVIEW ${view.flutterView.viewId}');
          final root = view.owner?.semanticsOwner?.rootSemanticsNode;
          report.writeln(root?.toStringDeep() ?? 'No active semantics tree.');
          report.writeln('\nNODE OWNERS');
          void visit(RenderObject object) {
            final node = object.debugSemantics;
            if (node != null) {
              report.writeln('NODE ${node.id}: ${node.getSemanticsData()}');
              report.writeln('OWNER ${object.debugCreator}\n');
            }
            object.visitChildren(visit);
          }

          visit(view);
        }
        final file = File(
          '${Directory.systemTemp.path}${Platform.pathSeparator}'
          'stellatune-semantics-$pid-${DateTime.now().microsecondsSinceEpoch}.txt',
        );
        await file.writeAsString(report.toString(), flush: true);
        debugPrint('[semantics] Snapshot saved: ${file.path}');
      } catch (error, stack) {
        debugPrint('[semantics] Snapshot failed: $error');
        debugPrintStack(stackTrace: stack);
      } finally {
        writing = false;
      }
    });
    binding.scheduleFrame();
    return true;
  });
  debugPrint(
    '[semantics] Press F8 in the app to save node IDs and widget owners.',
  );
}
