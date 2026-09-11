import 'package:flutter/material.dart';

/// Shared surface and typography for selectors and action menus.
abstract final class AppMenuStyle {
  static TextStyle text(ThemeData theme) => theme.textTheme.bodyMedium!
      .copyWith(fontSize: 13, fontWeight: FontWeight.w500);

  static RoundedRectangleBorder shape(ColorScheme scheme) =>
      RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
        side: BorderSide(color: scheme.outlineVariant.withValues(alpha: 0.55)),
      );

  static MenuStyle menu(ThemeData theme) => MenuStyle(
    alignment: AlignmentDirectional.bottomStart,
    backgroundColor: WidgetStatePropertyAll(
      theme.colorScheme.surfaceContainerLow,
    ),
    surfaceTintColor: const WidgetStatePropertyAll(Colors.transparent),
    shadowColor: WidgetStatePropertyAll(
      theme.colorScheme.shadow.withValues(alpha: 0.16),
    ),
    elevation: const WidgetStatePropertyAll(4),
    padding: const WidgetStatePropertyAll(EdgeInsets.all(6)),
    shape: WidgetStatePropertyAll(shape(theme.colorScheme)),
  );

  static PopupMenuThemeData popup(ThemeData theme) => PopupMenuThemeData(
    color: theme.colorScheme.surfaceContainerLow,
    surfaceTintColor: Colors.transparent,
    shadowColor: theme.colorScheme.shadow.withValues(alpha: 0.16),
    elevation: 4,
    shape: shape(theme.colorScheme),
    menuPadding: const EdgeInsets.all(6),
    position: PopupMenuPosition.under,
    labelTextStyle: WidgetStateProperty.resolveWith(
      (states) => text(theme).copyWith(
        color: states.contains(WidgetState.disabled)
            ? theme.disabledColor
            : theme.colorScheme.onSurface,
      ),
    ),
  );
}
