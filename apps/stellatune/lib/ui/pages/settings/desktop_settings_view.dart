import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:flutter/material.dart';

class SettingsPanel {
  const SettingsPanel({
    required this.id,
    required this.keywords,
    required this.child,
  });
  final String id, keywords;
  final Widget child;
}

class DesktopSettingsView extends StatefulWidget {
  const DesktopSettingsView({super.key, required this.panels});
  final List<SettingsPanel> panels;
  @override
  State<DesktopSettingsView> createState() => _DesktopSettingsViewState();
}

class _DesktopSettingsViewState extends State<DesktopSettingsView> {
  String query = '';
  @override
  Widget build(BuildContext context) {
    final visible = widget.panels
        .where(
          (panel) =>
              query.trim().isEmpty ||
              query
                  .trim()
                  .toLowerCase()
                  .split(RegExp(r'\s+'))
                  .every((word) => panel.keywords.toLowerCase().contains(word)),
        )
        .toList();
    final palette = ArtworkPalette.of(context);
    return Theme(
      data: palette
          .applyTo(Theme.of(context))
          .copyWith(
            switchTheme: SwitchThemeData(
              thumbColor: WidgetStateProperty.resolveWith(
                (states) => states.contains(WidgetState.selected)
                    ? palette.onAccent
                    : palette.onSurfaceVariant,
              ),
              trackOutlineColor: const WidgetStatePropertyAll(
                Colors.transparent,
              ),
              trackColor: WidgetStateProperty.resolveWith(
                (states) => states.contains(WidgetState.selected)
                    ? ArtworkPalette.of(context).accent
                    : palette.controlSurface,
              ),
            ),
          ),
      child: LayoutBuilder(
        builder: (context, size) {
          final twoColumns = size.maxWidth >= 1000;
          return SingleChildScrollView(
            key: const ValueKey('settings-scroll'),
            padding: const EdgeInsets.fromLTRB(28, 12, 28, 24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            '设置',
                            style: TextStyle(
                              fontSize: 34,
                              fontWeight: FontWeight.w600,
                              color: ArtworkPalette.of(context).onBackdrop,
                            ),
                          ),
                          SizedBox(height: 4),
                          Text(
                            '定制属于你的聆听体验',
                            style: TextStyle(
                              fontSize: 15,
                              color: ArtworkPalette.of(context).onBackdrop
                                  .withValues(alpha: .7),
                            ),
                          ),
                        ],
                      ),
                    ),
                    SizedBox(
                      width: size.maxWidth < 800 ? 240 : 320,
                      child: TextField(
                        key: const ValueKey('settings-search'),
                        onChanged: (value) => setState(() => query = value),
                        style: TextStyle(fontSize: 13),
                        decoration: InputDecoration(
                          hintText: '搜索设置项…',
                          prefixIcon: const Icon(Icons.search, size: 21),
                          contentPadding: const EdgeInsets.symmetric(
                            horizontal: 16,
                            vertical: 12,
                          ),
                          enabledBorder: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(28),
                            borderSide: BorderSide(
                              color: Colors.white.withValues(alpha: .55),
                            ),
                          ),
                          border: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(28),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 24),
                if (visible.isEmpty)
                  Padding(
                    padding: EdgeInsets.all(36),
                    child: Text(
                      '没有找到匹配的设置',
                      textAlign: TextAlign.center,
                      style: TextStyle(
                        color: ArtworkPalette.of(context).onBackdrop
                            .withValues(alpha: .7),
                      ),
                    ),
                  ),
                for (var i = 0; i < visible.length; i += twoColumns ? 2 : 1)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 14),
                    child: twoColumns
                        ? Table(
                            columnWidths: const {
                              0: FlexColumnWidth(),
                              1: FixedColumnWidth(14),
                              2: FlexColumnWidth(),
                            },
                            // Measures each card at its actual column width,
                            // then stretches both cells to the taller card.
                            defaultVerticalAlignment:
                                TableCellVerticalAlignment.intrinsicHeight,
                            children: [
                              TableRow(
                                children: [
                                  KeyedSubtree(
                                    key: ValueKey(visible[i].id),
                                    child: visible[i].child,
                                  ),
                                  const SizedBox.shrink(),
                                  i + 1 < visible.length
                                      ? KeyedSubtree(
                                          key: ValueKey(visible[i + 1].id),
                                          child: visible[i + 1].child,
                                        )
                                      : const SizedBox.shrink(),
                                ],
                              ),
                            ],
                          )
                        : KeyedSubtree(
                            key: ValueKey(visible[i].id),
                            child: visible[i].child,
                          ),
                  ),
              ],
            ),
          );
        },
      ),
    );
  }
}
