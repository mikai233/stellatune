import 'dart:convert';

import 'package:flutter/material.dart';

enum _SchemaFieldKind { string, number, integer, boolean, json }

class _SchemaField {
  const _SchemaField({
    required this.key,
    required this.label,
    required this.description,
    required this.kind,
    required this.required,
    required this.enumValues,
    required this.defaultValue,
    required this.order,
  });

  final String key;
  final String label;
  final String? description;
  final _SchemaFieldKind kind;
  final bool required;
  final List<String> enumValues;
  final Object? defaultValue;
  final int order;
}

class SchemaForm extends StatefulWidget {
  const SchemaForm({
    super.key,
    required this.schemaJson,
    required this.initialValueJson,
    required this.onChangedJson,
    this.fallbackLabel = 'Config JSON',
  });

  final String schemaJson;
  final String initialValueJson;
  final ValueChanged<String> onChangedJson;
  final String fallbackLabel;

  @override
  State<SchemaForm> createState() => _SchemaFormState();
}

class _SchemaFormState extends State<SchemaForm> {
  late final TextEditingController _fallbackController;
  List<_SchemaField> _fields = const [];
  final Map<String, dynamic> _values = <String, dynamic>{};
  final Map<String, TextEditingController> _controllers =
      <String, TextEditingController>{};
  final Map<String, String?> _fieldErrors = <String, String?>{};
  String? _schemaError;

  @override
  void initState() {
    super.initState();
    _fallbackController = TextEditingController(text: widget.initialValueJson);
    _rebuildFromSchema();
  }

  @override
  void didUpdateWidget(covariant SchemaForm oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.schemaJson != widget.schemaJson ||
        oldWidget.initialValueJson != widget.initialValueJson) {
      _fallbackController.text = widget.initialValueJson;
      _rebuildFromSchema();
    }
  }

  @override
  void dispose() {
    _fallbackController.dispose();
    for (final c in _controllers.values) {
      c.dispose();
    }
    super.dispose();
  }

  void _rebuildFromSchema() {
    for (final c in _controllers.values) {
      c.dispose();
    }
    _controllers.clear();
    _fieldErrors.clear();
    _values.clear();

    final parsedInitial =
        _decodeObject(widget.initialValueJson) ?? <String, dynamic>{};
    _values.addAll(parsedInitial);

    try {
      final fields = _parseSchema(widget.schemaJson);
      for (final f in fields) {
        if (!_values.containsKey(f.key) && f.defaultValue != null) {
          _values[f.key] = f.defaultValue;
        }
      }
      for (final f in fields) {
        if (_usesTextController(f.kind)) {
          _controllers[f.key] = TextEditingController(
            text: _valueToText(_values[f.key], f.kind),
          );
        }
      }
      _fields = fields;
      _schemaError = null;
      _emit();
    } catch (e) {
      _fields = const [];
      _schemaError = e.toString();
      widget.onChangedJson(_fallbackController.text);
    }
    if (mounted) {
      setState(() {});
    }
  }

  bool _usesTextController(_SchemaFieldKind kind) =>
      kind == _SchemaFieldKind.string ||
      kind == _SchemaFieldKind.number ||
      kind == _SchemaFieldKind.integer ||
      kind == _SchemaFieldKind.json;

  static Map<String, dynamic>? _decodeObject(String raw) {
    final text = raw.trim();
    if (text.isEmpty) return <String, dynamic>{};
    try {
      final decoded = jsonDecode(text);
      if (decoded is Map) {
        return decoded.cast<String, dynamic>();
      }
    } catch (_) {}
    return null;
  }

  static List<_SchemaField> _parseSchema(String schemaJson) {
    final root = jsonDecode(schemaJson);
    if (root is! Map) {
      throw const FormatException('schema root must be an object');
    }
    final map = root.cast<String, dynamic>();
    final propertiesRaw = map['properties'];
    if (propertiesRaw is! Map) {
      throw const FormatException('schema must contain object properties');
    }
    final properties = propertiesRaw.cast<String, dynamic>();
    final requiredSet = <String>{};
    final requiredRaw = map['required'];
    if (requiredRaw is List) {
      for (final item in requiredRaw) {
        if (item is String) {
          requiredSet.add(item);
        }
      }
    }

    final fields = <_SchemaField>[];
    properties.forEach((key, value) {
      if (value is! Map) return;
      final field = value.cast<String, dynamic>();
      final title = (field['title'] as String?)?.trim();
      final description = (field['description'] as String?)?.trim();
      final type = _readType(field['type']);

      final enumValues = <String>[];
      final enumRaw = field['enum'];
      if (enumRaw is List) {
        for (final v in enumRaw) {
          enumValues.add(v.toString());
        }
      }

      final order = _readOrder(field);

      _SchemaFieldKind kind;
      if (enumValues.isNotEmpty) {
        kind = _SchemaFieldKind.string;
      } else {
        switch (type) {
          case 'string':
            kind = _SchemaFieldKind.string;
            break;
          case 'number':
            kind = _SchemaFieldKind.number;
            break;
          case 'integer':
            kind = _SchemaFieldKind.integer;
            break;
          case 'boolean':
            kind = _SchemaFieldKind.boolean;
            break;
          case 'object':
          case 'array':
            kind = _SchemaFieldKind.json;
            break;
          default:
            kind = _SchemaFieldKind.json;
            break;
        }
      }

      fields.add(
        _SchemaField(
          key: key,
          label: title == null || title.isEmpty ? key : title,
          description: description == null || description.isEmpty
              ? null
              : description,
          kind: kind,
          required: requiredSet.contains(key),
          enumValues: enumValues,
          defaultValue: field.containsKey('default') ? field['default'] : null,
          order: order,
        ),
      );
    });

    fields.sort((a, b) {
      final c = a.order.compareTo(b.order);
      if (c != 0) return c;
      return a.key.compareTo(b.key);
    });
    return fields;
  }

  static int _readOrder(Map<String, dynamic> field) {
    final direct = field['x-order'];
    if (direct is int) return direct;
    if (direct is num) return direct.toInt();
    final ext = field['x-stellatune'];
    if (ext is Map) {
      final v = ext['order'];
      if (v is int) return v;
      if (v is num) return v.toInt();
    }
    return 0;
  }

  static String? _readType(dynamic rawType) {
    if (rawType is String) {
      final t = rawType.trim();
      return t.isEmpty ? null : t;
    }
    if (rawType is List) {
      String? fallback;
      for (final item in rawType) {
        if (item is! String) continue;
        final t = item.trim();
        if (t.isEmpty) continue;
        if (t != 'null') return t;
        fallback ??= t;
      }
      return fallback;
    }
    return null;
  }

  static String _valueToText(Object? value, _SchemaFieldKind kind) {
    if (value == null) return '';
    if (kind == _SchemaFieldKind.json) {
      try {
        return jsonEncode(value);
      } catch (_) {
        return value.toString();
      }
    }
    return value.toString();
  }

  void _emit() {
    widget.onChangedJson(jsonEncode(_values));
  }

  void _setFieldValue(_SchemaField f, dynamic value) {
    if (value == null) {
      _values.remove(f.key);
    } else {
      _values[f.key] = value;
    }
    _emit();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    if (_schemaError != null) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Schema parse failed, fallback to raw JSON editor.',
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.error,
            ),
          ),
          const SizedBox(height: 8),
          TextField(
            controller: _fallbackController,
            minLines: 2,
            maxLines: 8,
            onChanged: widget.onChangedJson,
            decoration: InputDecoration(
              labelText: widget.fallbackLabel,
              border: const OutlineInputBorder(),
            ),
          ),
        ],
      );
    }

    return Column(
      children: [
        for (final f in _fields) ...[
          _buildField(context, f),
          const SizedBox(height: 10),
        ],
      ],
    );
  }

  Widget _buildField(BuildContext context, _SchemaField f) {
    final label = f.required ? '${f.label} *' : f.label;

    if (f.kind == _SchemaFieldKind.boolean) {
      final current = _values[f.key];
      final value = current is bool ? current : false;
      return SwitchListTile.adaptive(
        value: value,
        title: Text(label),
        subtitle: f.description?.isEmpty ?? true ? null : Text(f.description!),
        contentPadding: EdgeInsets.zero,
        onChanged: (v) {
          setState(() => _setFieldValue(f, v));
        },
      );
    }

    if (f.enumValues.isNotEmpty) {
      final raw = _values[f.key];
      final current = raw?.toString();
      final normalized = f.enumValues.contains(current) ? current : null;
      return DropdownButtonFormField<String>(
        initialValue: normalized,
        decoration: InputDecoration(
          labelText: label,
          helperText: f.description,
          border: const OutlineInputBorder(),
          isDense: true,
        ),
        items: [
          for (final opt in f.enumValues)
            DropdownMenuItem(value: opt, child: Text(opt)),
        ],
        onChanged: (v) {
          setState(() => _setFieldValue(f, v));
        },
      );
    }

    final controller = _controllers[f.key];
    if (controller == null) {
      return const SizedBox.shrink();
    }

    final isJson = f.kind == _SchemaFieldKind.json;
    return TextField(
      controller: controller,
      minLines: isJson ? 2 : 1,
      maxLines: isJson ? 6 : 1,
      keyboardType:
          f.kind == _SchemaFieldKind.number ||
              f.kind == _SchemaFieldKind.integer
          ? const TextInputType.numberWithOptions(decimal: true, signed: true)
          : TextInputType.text,
      decoration: InputDecoration(
        labelText: label,
        helperText: f.description,
        errorText: _fieldErrors[f.key],
        border: const OutlineInputBorder(),
      ),
      onChanged: (text) {
        setState(() {
          _fieldErrors[f.key] = null;
          switch (f.kind) {
            case _SchemaFieldKind.string:
              _setFieldValue(f, text);
              break;
            case _SchemaFieldKind.number:
              final n = double.tryParse(text.trim());
              if (text.trim().isEmpty || n != null) {
                _setFieldValue(f, text.trim().isEmpty ? null : n);
              } else {
                _fieldErrors[f.key] = 'Invalid number';
              }
              break;
            case _SchemaFieldKind.integer:
              final n = int.tryParse(text.trim());
              if (text.trim().isEmpty || n != null) {
                _setFieldValue(f, text.trim().isEmpty ? null : n);
              } else {
                _fieldErrors[f.key] = 'Invalid integer';
              }
              break;
            case _SchemaFieldKind.json:
              final trimmed = text.trim();
              if (trimmed.isEmpty) {
                _setFieldValue(f, null);
                break;
              }
              try {
                _setFieldValue(f, jsonDecode(trimmed));
              } catch (_) {
                _fieldErrors[f.key] = 'Invalid JSON';
              }
              break;
            case _SchemaFieldKind.boolean:
              break;
          }
        });
      },
    );
  }
}
