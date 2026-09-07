import 'package:flutter/material.dart';

class SettingsFormRow extends StatelessWidget {
  const SettingsFormRow({
    super.key,
    required this.title,
    this.subtitle,
    required this.control,
    this.switchControl = false,
  });
  final Widget title;
  final Widget? subtitle;
  final Widget control;
  final bool switchControl;
  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, size) {
      final label = Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          DefaultTextStyle.merge(
            style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
            child: title,
          ),
          if (subtitle != null) ...[
            const SizedBox(height: 4),
            DefaultTextStyle.merge(
              style: TextStyle(
                fontSize: 12,
                height: 1.45,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
              child: subtitle!,
            ),
          ],
        ],
      );
      final stack = !switchControl && size.maxWidth < 430;
      return Container(
        padding: const EdgeInsets.symmetric(vertical: 12, horizontal: 12),
        decoration: BoxDecoration(
          border: Border(
            bottom: BorderSide(
              color: Theme.of(context).colorScheme.onSurface
                  .withValues(alpha: .06),
            ),
          ),
        ),
        child: stack
            ? Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [label, const SizedBox(height: 10), control],
              )
            : Row(
                children: [
                  Expanded(child: label),
                  const SizedBox(width: 18),
                  SizedBox(width: switchControl ? 52 : 200, child: control),
                ],
              ),
      );
    },
  );
}

/// Uses the existing field inputs/callbacks, with the label outside the control.
class SettingsSelectField<T> extends StatelessWidget {
  const SettingsSelectField({
    super.key,
    required this.decoration,
    required this.initialValue,
    required this.items,
    required this.onChanged,
  });
  final InputDecoration decoration;
  final T? initialValue;
  final List<DropdownMenuItem<T>> items;
  final ValueChanged<T?>? onChanged;
  @override
  Widget build(BuildContext context) => SettingsFormRow(
    title: Text(decoration.labelText ?? ''),
    subtitle: decoration.helperText == null
        ? null
        : Text(decoration.helperText!),
    control: DropdownButtonFormField<T>(
      key: ValueKey(initialValue),
      initialValue: initialValue,
      isExpanded: true,
      dropdownColor: Theme.of(context).colorScheme.surface,
      style: TextStyle(
        fontFamily: 'NotoSansSC',
        fontSize: 12,
        color: Theme.of(context).colorScheme.onSurface,
      ),
      decoration: const InputDecoration(
        isDense: true,
        contentPadding: EdgeInsets.symmetric(horizontal: 12, vertical: 11),
      ),
      items: items,
      onChanged: onChanged,
      selectedItemBuilder: (_) => items
          .map(
            (item) => Align(
              alignment: Alignment.centerLeft,
              child: DefaultTextStyle.merge(
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                child: item.child,
              ),
            ),
          )
          .toList(),
    ),
  );
}

class SettingsToggleRow extends StatelessWidget {
  const SettingsToggleRow({
    super.key,
    required this.title,
    this.subtitle,
    required this.value,
    required this.onChanged,
    this.dense,
    this.contentPadding,
  });
  final Widget title;
  final Widget? subtitle;
  final bool value;
  final ValueChanged<bool>? onChanged;
  // Accepted so existing SwitchListTile call sites retain their callbacks.
  final bool? dense;
  final EdgeInsetsGeometry? contentPadding;
  @override
  Widget build(BuildContext context) => SettingsFormRow(
    title: title,
    subtitle: subtitle,
    switchControl: true,
    control: Switch(value: value, onChanged: onChanged),
  );
}
