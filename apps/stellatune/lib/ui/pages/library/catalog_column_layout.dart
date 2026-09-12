import 'dart:math' as math;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/library/catalog_track_sort.dart';

const _textWeights = {
  CatalogTrackColumn.title: 5.0,
  CatalogTrackColumn.artist: 3.0,
  CatalogTrackColumn.album: 3.0,
};

final catalogColumnWidthsProvider =
    NotifierProvider<CatalogColumnWidthsController, Map<String, double>>(
      CatalogColumnWidthsController.new,
    );

class CatalogColumnWidthsController extends Notifier<Map<String, double>> {
  CatalogColumnWidthsController([this.store]);
  final SettingsStore? store;

  @override
  Map<String, double> build() => store?.catalogColumnWidths ?? const {};

  void resize(CatalogColumnLayout layout, int boundary, double delta) {
    final left = layout.columns[boundary];
    final right = layout.columns[boundary + 1];
    final change = delta.clamp(
      layout.minimums[left]! - layout.widths[left]!,
      layout.widths[right]! - layout.minimums[right]!,
    );
    final widths = {...layout.widths};
    widths[left] = widths[left]! + change;
    widths[right] = widths[right]! - change;
    final text = layout.columns.where(_textWeights.containsKey);
    final totalWidth = text.fold(
      0.0,
      (sum, c) => sum + widths[c]! / layout.visibility[c]!,
    );
    final totalWeight = text.fold(
      0.0,
      (sum, c) => sum + (state[c.name] ?? _textWeights[c]!),
    );
    state = Map.unmodifiable({
      ...state,
      for (final c in layout.columns)
        c.name: _textWeights.containsKey(c)
            ? widths[c]! / layout.visibility[c]! / totalWidth * totalWeight
            : widths[c]! / layout.visibility[c]!,
    });
  }

  Future<void> save() async {
    try {
      await store?.setCatalogColumnWidths(state);
    } catch (error, stack) {
      DiagnosticsService.instance.report(
        error,
        stack: stack,
        operation: 'settings',
      );
    }
  }

  Future<void> reset() async {
    state = const {};
    await save();
  }
}

/// Shared geometry for headers and rows; text columns share the remaining space.
class CatalogColumnLayout {
  CatalogColumnLayout(
    double width,
    double numberWidth,
    Map<String, double> saved,
  ) {
    // Follow the available width directly, including every frame of the folder
    // pane animation. No independent row animations or breakpoint-sized jumps.
    double reveal(double lower, double upper) {
      final t = ((width - lower) / (upper - lower)).clamp(0.0, 1.0);
      return t * t * (3 - 2 * t);
    }

    final medium = reveal(410, 490);
    final wide = reveal(630, 730);
    final format = reveal(730, 830);
    visibility.addAll({
      CatalogTrackColumn.original: medium,
      CatalogTrackColumn.title: 1,
      CatalogTrackColumn.artist: wide,
      CatalogTrackColumn.album: medium,
      CatalogTrackColumn.duration: 1,
      CatalogTrackColumn.format: format,
    });
    actionsWidth = 36 + 32 * medium;
    columns = [
      if (medium > 0) CatalogTrackColumn.original,
      CatalogTrackColumn.title,
      if (wide > 0) CatalogTrackColumn.artist,
      if (medium > 0) CatalogTrackColumn.album,
      CatalogTrackColumn.duration,
      if (format > 0) CatalogTrackColumn.format,
    ];
    for (var i = 0; i < columns.length; i++) {
      final c = columns[i];
      gaps[c] = switch (c) {
        CatalogTrackColumn.original => 12 * medium,
        CatalogTrackColumn.title => 12 + 4 * medium,
        CatalogTrackColumn.artist => 16 * wide,
        CatalogTrackColumn.album => 12 * medium,
        CatalogTrackColumn.duration => 12 * format,
        CatalogTrackColumn.format => 0,
      };
      minimums[c] =
          (switch (c) {
            CatalogTrackColumn.original => numberWidth,
            CatalogTrackColumn.title => 80,
            CatalogTrackColumn.artist || CatalogTrackColumn.album => 64,
            CatalogTrackColumn.duration => 44,
            CatalogTrackColumn.format => 84,
          }) *
          visibility[c]!;
    }
    var available = math.max(
      0.0,
      width - 24 - actionsWidth - gaps.values.fold(0.0, (a, b) => a + b),
    );
    final minimumTotal = minimums.values.fold(0.0, (a, b) => a + b);
    // Very small embedded previews still fit; normal desktop widths retain minima.
    if (available < minimumTotal) {
      for (final c in columns) {
        minimums[c] = minimums[c]! * available / minimumTotal;
      }
    }
    for (final c in columns.where((c) => !_textWeights.containsKey(c))) {
      final otherMinimum = minimums.entries
          .where((e) => e.key != c && !widths.containsKey(e.key))
          .fold(0.0, (sum, e) => sum + e.value);
      widths[c] =
          (saved[c.name] == null
                  ? minimums[c]!
                  : saved[c.name]! * visibility[c]!)
              .clamp(
                minimums[c]!,
                math.max(minimums[c]!, available - otherMinimum),
              );
      available -= widths[c]!;
    }
    final pending = columns.where(_textWeights.containsKey).toList();
    while (pending.isNotEmpty) {
      final weights = pending.fold(
        0.0,
        (sum, c) => sum + (saved[c.name] ?? _textWeights[c]!) * visibility[c]!,
      );
      final constrained = pending
          .where(
            (c) =>
                available *
                    (saved[c.name] ?? _textWeights[c]!) *
                    visibility[c]! /
                    weights <
                minimums[c]!,
          )
          .toList();
      if (constrained.isEmpty) {
        for (final c in pending) {
          widths[c] =
              available *
              (saved[c.name] ?? _textWeights[c]!) *
              visibility[c]! /
              weights;
        }
        break;
      }
      for (final c in constrained) {
        widths[c] = minimums[c]!;
        available -= widths[c]!;
        pending.remove(c);
      }
    }
  }

  late final List<CatalogTrackColumn> columns;
  late final double actionsWidth;
  final widths = <CatalogTrackColumn, double>{};
  final minimums = <CatalogTrackColumn, double>{};
  final gaps = <CatalogTrackColumn, double>{};
  final visibility = <CatalogTrackColumn, double>{};

  double boundaryOffset(int index) {
    var offset = 12.0;
    for (var i = 0; i <= index; i++) {
      final c = columns[i];
      offset += widths[c]! + (i == index ? gaps[c]! / 2 : gaps[c]!);
    }
    return offset;
  }
}
