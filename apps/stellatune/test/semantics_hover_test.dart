import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/track_list/widgets/track_list_tile.dart';
import 'package:stellatune/ui/widgets/folder_tree.dart';
import 'package:stellatune/ui/widgets/track_list.dart';
import 'package:stellatune/ui/widgets/custom_title_bar.dart';

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
