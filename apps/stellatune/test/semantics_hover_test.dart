import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/ui/widgets/app_select.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/ui/diagnostics/diagnostics_overlay.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/track_list/widgets/track_list_tile.dart';
import 'package:stellatune/ui/widgets/folder_tree.dart';
import 'package:stellatune/ui/widgets/track_list.dart';
import 'package:stellatune/ui/widgets/custom_title_bar.dart';
import 'package:stellatune/ui/widgets/now_playing_common/volume_popup_button.dart';

// Widget tests normally discard semantics updates. Check the serialized child
// references as well as widget behavior; this is not a complete native AXTree
// validator. Windows integration remains necessary for native-only failures.
class _RecordingBinding extends AutomatedTestWidgetsFlutterBinding {
  final nodes = <int, Map<Symbol, dynamic>>{};
  final errors = <String>[];

  @override
  ui.SemanticsUpdateBuilder createSemanticsUpdateBuilder() =>
      _RecordingBuilder(this);

  void record(List<Map<Symbol, dynamic>> updates) {
    final oldParents = <int, int>{};
    for (final node in nodes.values) {
      for (final child in node[#childrenInTraversalOrder] as Iterable<int>) {
        oldParents[child] = node[#id] as int;
      }
    }
    final removed = <int>{};
    void removeSubtree(int id) {
      if (!removed.add(id)) return;
      for (final child
          in (nodes[id]?[#childrenInTraversalOrder] as Iterable<int>?) ??
              <int>[]) {
        removeSubtree(child);
      }
    }

    for (final update in updates) {
      final id = update[#id] as int;
      final nextChildren = update[#childrenInTraversalOrder] as Iterable<int>;
      for (final oldChild
          in (nodes[id]?[#childrenInTraversalOrder] as Iterable<int>?) ??
              <int>[]) {
        if (!nextChildren.contains(oldChild)) removeSubtree(oldChild);
      }
      for (final child in nextChildren) {
        if (oldParents.containsKey(child) && oldParents[child] != id) {
          removeSubtree(child);
        }
      }
    }
    for (final id in removed) {
      nodes.remove(id);
    }
    for (final node in updates) {
      nodes[node[#id] as int] = node;
    }
    final reachable = <int>{};
    void visit(int id) {
      if (!reachable.add(id)) return;
      final node = nodes[id];
      if (node == null) {
        errors.add('Referenced node $id has no data');
        return;
      }
      for (final child in node[#childrenInTraversalOrder] as Iterable<int>) {
        visit(child);
      }
    }

    visit(0);
    for (final node in updates) {
      if (!reachable.contains(node[#id])) {
        errors.add(
          'Orphan ${node[#id]}: label=${node[#label]}, '
          'tooltip=${node[#tooltip]}, traversalParent=${node[#traversalParent]}',
        );
      }
    }
    nodes.removeWhere((id, _) => !reachable.contains(id));
  }
}

class _RecordingBuilder extends Fake implements ui.SemanticsUpdateBuilder {
  _RecordingBuilder(this.binding);
  final _RecordingBinding binding;
  final updates = <Map<Symbol, dynamic>>[];

  @override
  dynamic noSuchMethod(Invocation invocation) {
    if (invocation.memberName == #updateNode) {
      updates.add(invocation.namedArguments);
      return null;
    }
    if (invocation.memberName == #updateCustomAction) return null;
    return super.noSuchMethod(invocation);
  }

  @override
  ui.SemanticsUpdate build() {
    binding.record(updates);
    return ui.SemanticsUpdateBuilder().build();
  }
}

void main() {
  final binding = _RecordingBinding();

  testWidgets('diagnostics overlay menus keep serialized semantics connected', (
    tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    binding.nodes.clear();
    binding.errors.clear();
    final semantics = tester.ensureSemantics();
    final service = DiagnosticsService();
    try {
      await tester.pumpWidget(
        MaterialApp(
          builder: (_, child) =>
              DiagnosticsOverlay(service: service, child: child!),
          home: const Scaffold(body: Text('Player')),
        ),
      );
      service.record('INFO', 'test', 'Initial log');
      service.open();
      await tester.pumpAndSettle();
      expect(binding.errors, isEmpty, reason: 'Opening diagnostics');
      for (final label in ['All levels', 'All sources']) {
        for (var cycle = 0; cycle < 3; cycle++) {
          await tester.tap(find.text(label));
          await tester.pump();
          expect(
            binding.errors,
            isEmpty,
            reason: 'Opening $label cycle $cycle',
          );
          for (var frame = 0; frame < 16; frame++) {
            service.record('INFO', 'test', 'Incoming log $frame');
            await tester.pump(const Duration(milliseconds: 16));
            expect(
              binding.errors,
              isEmpty,
              reason: '$label refresh frame $frame',
            );
          }
          await tester.sendKeyEvent(LogicalKeyboardKey.escape);
          await tester.pumpAndSettle();
          expect(service.visible.value, isTrue);
          expect(binding.errors, isEmpty, reason: 'Closing $label');
        }
      }
      service.close();
      await tester.pumpAndSettle();
      expect(binding.errors, isEmpty, reason: 'Returning to player');
      await tester.pumpWidget(const SizedBox());
      await tester.pumpAndSettle();
    } finally {
      semantics.dispose();
      debugDefaultTargetPlatformOverride = null;
      await service.shutdown();
    }
  });

  testWidgets(
    'select menu keeps serialized semantics connected on every frame',
    (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.windows;
      binding.nodes.clear();
      binding.errors.clear();
      final semantics = tester.ensureSemantics();
      try {
        String? selected = 'All levels';
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: StatefulBuilder(
                builder: (context, setState) => Center(
                  child: AppSelect<String>(
                    width: 180,
                    value: selected,
                    items: [
                      for (final label in [
                        'All levels',
                        'TRACE',
                        'DEBUG',
                        'INFO',
                        'WARN',
                        'ERROR',
                      ])
                        DropdownMenuItem(value: label, child: Text(label)),
                    ],
                    onChanged: (value) => setState(() => selected = value),
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
        await mouse.addPointer(location: const Offset(1, 1));
        for (var cycle = 0; cycle < 3; cycle++) {
          await tester.tap(find.byType(OutlinedButton));
          await tester.pump();
          expect(
            binding.nodes.values.any(
              (node) => (node[#label] as String).contains('WARN'),
            ),
            isTrue,
            reason: 'Menu choices must be accessible from the first frame',
          );
          for (var frame = 0; frame < 36; frame++) {
            await tester.pump(const Duration(milliseconds: 16));
            expect(
              binding.errors,
              isEmpty,
              reason: 'Opening $cycle frame $frame',
            );
          }
          await mouse.moveTo(tester.getCenter(find.text('ERROR').last));
          await tester.pump(const Duration(milliseconds: 100));
          await tester.tap(find.text('ERROR').last);
          for (var frame = 0; frame < 20; frame++) {
            await tester.pump(const Duration(milliseconds: 16));
            expect(
              binding.errors,
              isEmpty,
              reason: 'Selecting $cycle frame $frame',
            );
          }
          expect(selected, 'ERROR');
        }
        await tester.tap(find.byType(OutlinedButton));
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(binding.errors, isEmpty);
        await mouse.removePointer();
        await tester.pumpWidget(const SizedBox());
        await tester.pumpAndSettle();
        expect(binding.errors, isEmpty);
      } finally {
        semantics.dispose();
        debugDefaultTargetPlatformOverride = null;
      }
    },
  );

  testWidgets(
    'volume hover popup keeps serialized semantics connected during animation',
    (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.windows;
      binding.nodes.clear();
      binding.errors.clear();
      final semantics = tester.ensureSemantics();
      final changes = <double>[];
      try {
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: Align(
                alignment: Alignment.bottomCenter,
                child: VolumePopupButton(
                  volume: .6,
                  enableHover: true,
                  onChanged: changes.add,
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
        await mouse.addPointer(location: const Offset(1, 1));
        for (var cycle = 0; cycle < 3; cycle++) {
          await mouse.moveTo(tester.getCenter(find.byType(VolumePopupButton)));
          await tester.pump();
          for (var frame = 0; frame < 16; frame++) {
            await tester.pump(const Duration(milliseconds: 16));
            expect(
              binding.errors,
              isEmpty,
              reason: 'Opening cycle $cycle frame $frame',
            );
          }
          expect(find.byType(Slider), findsOneWidget);
          for (final action in [
            ui.SemanticsAction.increase,
            ui.SemanticsAction.decrease,
          ]) {
            expect(
              binding.nodes.values.any(
                (node) => ((node[#actions] as int? ?? 0) & action.index) != 0,
              ),
              isTrue,
              reason: 'The visible volume slider must expose $action',
            );
          }
          await mouse.moveTo(tester.getCenter(find.byType(Slider)));
          await tester.pump();
          await mouse.down(tester.getCenter(find.byType(Slider)));
          await mouse.moveBy(const Offset(0, -20));
          await tester.pump(const Duration(milliseconds: 16));
          await mouse.up();
          await tester.pump(const Duration(milliseconds: 200));
          expect(changes, isNotEmpty);
          expect(binding.errors, isEmpty);
          await mouse.moveTo(const Offset(1, 1));
          for (var frame = 0; frame < 24; frame++) {
            await tester.pump(const Duration(milliseconds: 16));
            expect(
              binding.errors,
              isEmpty,
              reason: 'Closing cycle $cycle frame $frame',
            );
          }
          expect(find.byType(Slider), findsNothing);
        }
        // Reverse a dismissal, then dispose while the popup is still attached.
        final anchor = tester.getCenter(find.byType(VolumePopupButton));
        await mouse.moveTo(anchor);
        await tester.pump(const Duration(milliseconds: 80));
        await mouse.moveTo(const Offset(1, 1));
        await tester.pump(const Duration(milliseconds: 150));
        await mouse.moveTo(anchor);
        await tester.pumpAndSettle();
        expect(find.byType(Slider), findsOneWidget);
        expect(binding.errors, isEmpty);
        await mouse.removePointer();
        await tester.pumpWidget(const SizedBox());
        await tester.pump(const Duration(milliseconds: 400));
        expect(binding.errors, isEmpty);
      } finally {
        semantics.dispose();
        debugDefaultTargetPlatformOverride = null;
      }
    },
  );

  Future<void> hoverAll(WidgetTester tester, Widget child) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    binding.nodes.clear();
    binding.errors.clear();
    final semantics = tester.ensureSemantics();
    try {
      await tester.pumpWidget(
        MaterialApp(
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(body: child),
        ),
      );
      await tester.pumpAndSettle();
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(1, 1));
      final tooltips = find.byType(Tooltip);
      final count = tooltips.evaluate().length;
      expect(count, greaterThan(1));
      for (var i = 0; i < count; i++) {
        await mouse.moveTo(tester.getCenter(tooltips.at(i)));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 80));
        await tester.pump(const Duration(milliseconds: 800));
        await tester.pump(const Duration(milliseconds: 200));
        final message = tester.widget<Tooltip>(tooltips.at(i)).message;
        if (message != null) expect(find.text(message), findsWidgets);
        expect(binding.errors, isEmpty, reason: 'Hovering tooltip $i');
      }
      await mouse.moveTo(const Offset(1, 1));
      await tester.pumpAndSettle();
      await mouse.removePointer();
      expect(binding.errors, isEmpty);
    } finally {
      semantics.dispose();
      debugDefaultTargetPlatformOverride = null;
    }
  }

  testWidgets('track tooltips keep serialized semantics connected', (
    tester,
  ) async {
    await hoverAll(
      tester,
      Builder(
        builder: (context) => ListView.builder(
          itemCount: 3,
          itemBuilder: (context, index) => TrackListTile(
            l10n: AppLocalizations.of(context)!,
            index: index,
            track: TrackLite(
              id: index + 1,
              path: 'test-$index.mp3',
              title: 'Track $index',
            ),
            coverDir: 'missing-test-covers',
            deferHeavy: false,
            selectionMode: false,
            selected: false,
            pressed: false,
            isLiked: false,
            isBlocked: true,
            blockedReason: 'Unavailable track $index',
            isDesktopPlatform: true,
            onPressedDown: () {},
            onPressedUp: () {},
            onPressedCancel: () {},
            onToggleLike: () {},
            onToggleSelected: () {},
            onTapTrack: () {},
            onTrackAction: (_) async {},
            buildTrackActionMenuItems: (_) => [],
          ),
        ),
      ),
    );
  });

  testWidgets('folder tooltips keep serialized semantics connected', (
    tester,
  ) async {
    await hoverAll(
      tester,
      FolderTree(
        roots: const ['D:/Music'],
        folders: const ['D:/Music', 'D:/Music/Album'],
        selectedFolder: '',
        isEditing: true,
        onSelectAll: () {},
        onSelectFolder: (_) {},
        onDeleteFolder: (_) {},
      ),
    );
  });

  for (final reorder in [false, true]) {
    testWidgets('full track list hovering (reorder=$reorder)', (tester) async {
      await hoverAll(
        tester,
        TrackList(
          coverDir: 'missing-test-covers',
          items: List.generate(
            5,
            (i) => TrackLite(id: i + 1, path: 'test-$i.mp3', title: 'Track $i'),
          ),
          likedTrackIds: const {},
          playlists: const [],
          currentPlaylistId: reorder ? 1 : null,
          onActivate: (_, _) async {},
          onEnqueue: (_) async {},
          onSetLiked: (_, _) async {},
          onAddToPlaylist: (_, _) async {},
          onRemoveFromPlaylist: (_, _) async {},
          onMoveInCurrentPlaylist: reorder ? (_, _) async {} : null,
        ),
      );
    });
  }

  testWidgets('title bar hovering', (tester) async {
    await hoverAll(
      tester,
      Row(
        children: [
          WindowButton(
            icon: Icons.minimize,
            onPressed: () {},
            color: Colors.black,
            tooltip: 'Minimize',
          ),
          WindowButton(
            icon: Icons.close,
            onPressed: () {},
            color: Colors.black,
            tooltip: 'Close',
          ),
        ],
      ),
    );
  });

  // Opt-in upstream reproducer: currently fails on Flutter 3.47.2. Kept outside
  // normal CI so SDK behavior can be compared without blaming an app widget.
  // FIXME(flutter-a11y): On SDK upgrades run this file with
  // --dart-define=RUN_FLUTTER_TOOLTIP_REPRO=true. Once the minimal Tooltip case
  // passes and Windows hover is clean, enable it in normal CI. Its result alone
  // does not justify removing the separate menu/volume workarounds above.
  if (const bool.fromEnvironment('RUN_FLUTTER_TOOLTIP_REPRO')) {
    testWidgets('upstream adjacent tooltip reproduction', (tester) async {
      await hoverAll(
        tester,
        ListView(
          children: [
            Row(
              children: [
                Tooltip(
                  message: 'Tooltip A',
                  child: Container(width: 100, height: 100, color: Colors.red),
                ),
                Tooltip(
                  message: 'Tooltip B',
                  child: Container(width: 100, height: 100, color: Colors.blue),
                ),
              ],
            ),
          ],
        ),
      );
    });
  }
}
