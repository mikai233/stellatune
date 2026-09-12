import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:stellatune/ui/theme/app_menu_style.dart';

/// Controlled selector shared by settings, catalog and diagnostics.
/// Keeps nullable values and disabled entries supplied by existing callers.
class AppSelect<T> extends StatefulWidget {
  const AppSelect({
    super.key,
    required this.value,
    required this.items,
    required this.onChanged,
    this.width,
    this.menuWidth,
    this.height = 44,
    this.filled = true,
    this.foregroundColor,
    this.hint,
    this.semanticLabel,
  });

  final T? value;
  final List<DropdownMenuItem<T>> items;
  final ValueChanged<T?>? onChanged;
  final double? width;
  final double? menuWidth;
  final double height;
  final bool filled;
  final Color? foregroundColor;
  final String? hint;
  final String? semanticLabel;

  @override
  State<AppSelect<T>> createState() => _AppSelectState<T>();
}

class _AppSelectState<T> extends State<AppSelect<T>> {
  final _focusNode = FocusNode();
  final _menuController = MenuController();

  void _releasePointerFocus() {
    if (!_focusNode.hasFocus && !_menuController.isOpen) return;
    // Menu dismissal restores trigger focus. Wait until that has finished, and
    // never clear focus that has already moved to another control. Keyboard
    // dismissal does not take this path, so its focus restoration is retained.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && !_menuController.isOpen && _focusNode.hasFocus) {
        _focusNode.unfocus();
      }
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final selected = widget.items
        .where((item) => item.value == widget.value)
        .firstOrNull;
    final enabled = widget.onChanged != null && widget.items.isNotEmpty;
    final textStyle = AppMenuStyle.text(theme);
    return LayoutBuilder(
      builder: (context, constraints) {
        final width = math.min(
          widget.width ??
              (constraints.hasBoundedWidth ? constraints.maxWidth : 200),
          constraints.maxWidth,
        );
        final menuWidth = math.min(
          widget.menuWidth ?? math.max(180, width),
          MediaQuery.sizeOf(context).width - 24,
        );
        return TapRegion(
          onTapOutside: (_) => _releasePointerFocus(),
          child: Listener(
            onPointerDown: (_) => _releasePointerFocus(),
            child: Semantics(
              // FIXME(flutter-a11y): Revisit this boundary after upgrading Flutter.
              // On 3.47.2, sibling MenuAnchor/OverlayPortal traversal parents merge
              // in DiagnosticsOverlay; opening a filter emits orphan AXTree nodes.
              // The exact upstream issue/fix is not confirmed. Remove only when
              // 'diagnostics overlay menus keep serialized semantics connected' in
              // test/semantics_hover_test.dart passes WITHOUT this boundary, and
              // Windows opening/closing all filters during live logging stays clean.
              container: true,
              child: MenuAnchor(
                controller: _menuController,
                // Show menu text fully opaque from its first frame. This is a visual
                // choice, not the AXTree workaround; disabling animation did not fix it.
                animated: false,
                childFocusNode: _focusNode,
                alignmentOffset: const Offset(0, 6),
                style: AppMenuStyle.menu(theme),
                menuChildren: [
                  for (final item in widget.items)
                    Semantics(
                      selected: item.value == widget.value,
                      child: MenuItemButton(
                        onPressed: enabled && item.enabled
                            ? () {
                                item.onTap?.call();
                                widget.onChanged!(item.value);
                              }
                            : null,
                        style: ButtonStyle(
                          minimumSize: const WidgetStatePropertyAll(
                            Size(0, 38),
                          ),
                          padding: const WidgetStatePropertyAll(
                            EdgeInsets.symmetric(horizontal: 10),
                          ),
                          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                          shape: WidgetStatePropertyAll(
                            RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(6),
                            ),
                          ),
                          backgroundColor: WidgetStatePropertyAll(
                            item.value == widget.value
                                ? scheme.primary.withValues(alpha: 0.09)
                                : Colors.transparent,
                          ),
                          overlayColor: WidgetStatePropertyAll(
                            scheme.primary.withValues(alpha: 0.06),
                          ),
                          foregroundColor: WidgetStateProperty.resolveWith(
                            (states) => states.contains(WidgetState.disabled)
                                ? theme.disabledColor
                                : item.value == widget.value
                                ? scheme.primary
                                : scheme.onSurface,
                          ),
                          textStyle: WidgetStatePropertyAll(textStyle),
                        ),
                        child: SizedBox(
                          width: math.max(0, menuWidth - 32),
                          child: Row(
                            children: [
                              Expanded(
                                child: DefaultTextStyle.merge(
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  child: item.child,
                                ),
                              ),
                              const SizedBox(width: 8),
                              SizedBox(
                                width: 16,
                                child: item.value == widget.value
                                    ? const Icon(Icons.check_rounded, size: 16)
                                    : null,
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                ],
                builder: (context, controller, child) => Semantics(
                  label: widget.semanticLabel,
                  child: SizedBox(
                    width: width,
                    height: widget.height,
                    child: OutlinedButton(
                      focusNode: _focusNode,
                      onPressed: enabled
                          ? () => controller.isOpen
                                ? controller.close()
                                : controller.open()
                          : null,
                      style: OutlinedButton.styleFrom(
                        padding: EdgeInsets.symmetric(
                          horizontal: widget.filled ? 14 : 8,
                        ),
                        backgroundColor: widget.filled
                            ? scheme.surfaceContainerLow
                            : Colors.transparent,
                        foregroundColor:
                            widget.foregroundColor ?? scheme.onSurfaceVariant,
                        overlayColor: scheme.primary.withValues(alpha: 0.06),
                        side: widget.filled
                            ? BorderSide(
                                color: controller.isOpen
                                    ? scheme.primary.withValues(alpha: 0.5)
                                    : scheme.outlineVariant.withValues(
                                        alpha: 0.65,
                                      ),
                              )
                            : BorderSide.none,
                        shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(10),
                        ),
                        textStyle: textStyle,
                      ),
                      child: Row(
                        children: [
                          Expanded(
                            child: DefaultTextStyle.merge(
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              child: selected?.child ?? Text(widget.hint ?? ''),
                            ),
                          ),
                          const SizedBox(width: 8),
                          AnimatedRotation(
                            turns: controller.isOpen ? 0.5 : 0,
                            duration: const Duration(milliseconds: 160),
                            curve: Curves.easeOutCubic,
                            child: const Icon(
                              Icons.expand_more_rounded,
                              size: 18,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}
