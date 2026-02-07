import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_models.dart';

class SourcesPage extends ConsumerStatefulWidget {
  const SourcesPage({super.key});

  @override
  ConsumerState<SourcesPage> createState() => _SourcesPageState();
}

class _SourcesPageState extends ConsumerState<SourcesPage> {
  final TextEditingController _configController = TextEditingController();
  final TextEditingController _requestController = TextEditingController(
    text: '{}',
  );

  List<SourceCatalogTypeDescriptor> _types = const [];
  SourceCatalogTypeDescriptor? _selectedType;
  List<_SourceItem> _items = const [];
  bool _loadingTypes = false;
  bool _loadingItems = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    unawaited(_loadTypes());
  }

  @override
  void dispose() {
    _configController.dispose();
    _requestController.dispose();
    super.dispose();
  }

  Future<void> _loadTypes() async {
    setState(() {
      _loadingTypes = true;
      _error = null;
    });

    try {
      final types = await ref.read(playerBridgeProvider).sourceListTypes();
      types.sort((a, b) {
        final ap = '${a.pluginName}/${a.displayName}'.toLowerCase();
        final bp = '${b.pluginName}/${b.displayName}'.toLowerCase();
        return ap.compareTo(bp);
      });

      SourceCatalogTypeDescriptor? nextSelected;
      if (_selectedType != null) {
        for (final t in types) {
          if (t.pluginId == _selectedType!.pluginId &&
              t.typeId == _selectedType!.typeId) {
            nextSelected = t;
            break;
          }
        }
      }
      nextSelected ??= types.isEmpty ? null : types.first;

      setState(() {
        _types = types;
        _selectedType = nextSelected;
        _items = const [];
        if (nextSelected != null) {
          _configController.text = nextSelected.defaultConfigJson;
        }
      });
    } catch (e) {
      setState(() => _error = e.toString());
    } finally {
      if (mounted) {
        setState(() => _loadingTypes = false);
      }
    }
  }

  Future<void> _loadItems() async {
    final type = _selectedType;
    if (type == null) return;

    final configJson = _configController.text.trim().isEmpty
        ? '{}'
        : _configController.text.trim();
    final requestJson = _requestController.text.trim().isEmpty
        ? '{}'
        : _requestController.text.trim();

    setState(() {
      _loadingItems = true;
      _error = null;
    });

    try {
      final out = await ref
          .read(playerBridgeProvider)
          .sourceListItemsJson(
            pluginId: type.pluginId,
            typeId: type.typeId,
            configJson: configJson,
            requestJson: requestJson,
          );
      final parsed = _parseSourceItems(
        outputJson: out,
        type: type,
        defaultConfigJson: configJson,
      );
      setState(() => _items = parsed);
    } catch (e) {
      setState(() => _error = e.toString());
    } finally {
      if (mounted) {
        setState(() => _loadingItems = false);
      }
    }
  }

  Future<void> _playAt(int index) async {
    if (index < 0 || index >= _items.length) return;
    final queueItems = _items.map((e) => e.queueItem).toList(growable: false);
    await ref
        .read(playbackControllerProvider.notifier)
        .setQueueAndPlayItems(queueItems, startIndex: index);
  }

  Future<void> _enqueue(_SourceItem item) async {
    await ref.read(playbackControllerProvider.notifier).enqueueItems(
      <QueueItem>[item.queueItem],
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final selected = _selectedType;

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.sourcesTitle),
        actions: [
          IconButton(
            tooltip: l10n.sourcesRefreshTypes,
            onPressed: _loadingTypes ? null : _loadTypes,
            icon: const Icon(Icons.refresh),
          ),
        ],
      ),
      body: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
        child: Column(
          children: [
            Card(
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Expanded(
                          child:
                              DropdownButtonFormField<
                                SourceCatalogTypeDescriptor
                              >(
                                key: ValueKey<String?>(
                                  selected == null
                                      ? null
                                      : '${selected.pluginId}:${selected.typeId}',
                                ),
                                initialValue: selected,
                                decoration: InputDecoration(
                                  labelText: l10n.sourcesTypeLabel,
                                  border: const OutlineInputBorder(),
                                  isDense: true,
                                ),
                                items: _types
                                    .map(
                                      (t) => DropdownMenuItem(
                                        value: t,
                                        child: Text(
                                          '${t.pluginName} / ${t.displayName}',
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                        ),
                                      ),
                                    )
                                    .toList(),
                                onChanged: _loadingTypes
                                    ? null
                                    : (next) {
                                        setState(() {
                                          _selectedType = next;
                                          _items = const [];
                                          _error = null;
                                          if (next != null) {
                                            _configController.text =
                                                next.defaultConfigJson;
                                          }
                                        });
                                      },
                              ),
                        ),
                        if (_loadingTypes) ...[
                          const SizedBox(width: 12),
                          const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                        ],
                      ],
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      controller: _configController,
                      minLines: 2,
                      maxLines: 6,
                      decoration: InputDecoration(
                        labelText: l10n.sourcesConfigJsonLabel,
                        border: const OutlineInputBorder(),
                      ),
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      controller: _requestController,
                      minLines: 2,
                      maxLines: 6,
                      decoration: InputDecoration(
                        labelText: l10n.sourcesRequestJsonLabel,
                        border: const OutlineInputBorder(),
                      ),
                    ),
                    const SizedBox(height: 12),
                    Row(
                      children: [
                        FilledButton.icon(
                          onPressed: selected == null || _loadingItems
                              ? null
                              : _loadItems,
                          icon: const Icon(Icons.playlist_add_check),
                          label: Text(l10n.sourcesLoadItems),
                        ),
                        const SizedBox(width: 12),
                        if (_loadingItems)
                          const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                        if (!_loadingItems && _items.isNotEmpty)
                          Text(
                            l10n.sourcesItemsCount(_items.length),
                            style: theme.textTheme.bodySmall,
                          ),
                      ],
                    ),
                    if (_error != null) ...[
                      const SizedBox(height: 8),
                      Text(
                        _error!,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.error,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ),
            const SizedBox(height: 12),
            Expanded(
              child: _types.isEmpty
                  ? Center(child: Text(l10n.sourcesNoTypes))
                  : _items.isEmpty
                  ? Center(child: Text(l10n.sourcesNoItems))
                  : ListView.separated(
                      itemCount: _items.length,
                      separatorBuilder: (context, index) =>
                          const Divider(height: 1),
                      itemBuilder: (context, index) {
                        final item = _items[index];
                        final queueItem = item.queueItem;
                        return ListTile(
                          title: Text(
                            queueItem.displayTitle,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          subtitle: Text(
                            _subtitleFor(item),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                          onTap: () => _playAt(index),
                          trailing: PopupMenuButton<_SourceAction>(
                            onSelected: (action) async {
                              if (action == _SourceAction.play) {
                                await _playAt(index);
                              } else if (action == _SourceAction.enqueue) {
                                await _enqueue(item);
                              }
                            },
                            itemBuilder: (context) => [
                              PopupMenuItem(
                                value: _SourceAction.play,
                                child: Text(l10n.menuPlay),
                              ),
                              PopupMenuItem(
                                value: _SourceAction.enqueue,
                                child: Text(l10n.menuEnqueue),
                              ),
                            ],
                          ),
                        );
                      },
                    ),
            ),
          ],
        ),
      ),
    );
  }

  static String _subtitleFor(_SourceItem item) {
    final artist = item.queueItem.artist?.trim() ?? '';
    final album = item.queueItem.album?.trim() ?? '';
    final meta = [artist, album].where((s) => s.isNotEmpty).join(' • ');
    if (meta.isNotEmpty) return meta;
    return '${item.queueItem.track.sourceId} • ${item.queueItem.track.trackId}';
  }

  static List<_SourceItem> _parseSourceItems({
    required String outputJson,
    required SourceCatalogTypeDescriptor type,
    required String defaultConfigJson,
  }) {
    final decoded = jsonDecode(outputJson);
    final rawItems = _extractItems(decoded);
    final out = <_SourceItem>[];

    for (var i = 0; i < rawItems.length; i++) {
      final raw = rawItems[i];
      if (raw is! Map) continue;
      final map = raw.cast<String, dynamic>();

      final sourceId = _pickString(map, const ['source_id', 'sourceId']).trim();
      final trackId = _pickString(map, const [
        'track_id',
        'trackId',
        'id',
        'key',
        'uid',
        'path',
        'url',
      ]).trim();
      final title = _pickString(map, const [
        'title',
        'name',
        'track_title',
      ]).trim();
      final artist = _pickString(map, const ['artist', 'artists']).trim();
      final album = _pickString(map, const ['album']).trim();
      final durationMs = _pickInt(map, const [
        'duration_ms',
        'durationMs',
        'duration',
      ]);

      final trackPayload = map.containsKey('track_json')
          ? map['track_json']
          : (map.containsKey('track') ? map['track'] : map);
      final trackJson = _toJsonString(trackPayload);
      final configJsonValue = _pickString(map, const [
        'config_json',
        'configJson',
      ]).trim();
      final configJson = configJsonValue.isEmpty
          ? defaultConfigJson
          : configJsonValue;

      final effectiveSourceId = sourceId.isEmpty
          ? '${type.pluginId}:${type.typeId}'
          : sourceId;
      final effectiveTrackId = trackId.isEmpty
          ? (title.isNotEmpty ? title : 'item-$i')
          : trackId;

      final track = buildPluginSourceTrackRef(
        sourceId: effectiveSourceId,
        trackId: effectiveTrackId,
        pluginId: type.pluginId,
        typeId: type.typeId,
        configJson: configJson,
        trackJson: trackJson,
        extHint: _pickString(map, const ['ext_hint', 'extHint']).trim(),
        pathHint: _pickString(map, const ['path_hint', 'pathHint']).trim(),
        decoderPluginId: _nullableString(
          _pickString(map, const [
            'decoder_plugin_id',
            'decoderPluginId',
          ]).trim(),
        ),
        decoderTypeId: _nullableString(
          _pickString(map, const ['decoder_type_id', 'decoderTypeId']).trim(),
        ),
      );

      out.add(
        _SourceItem(
          queueItem: QueueItem(
            track: track,
            title: title.isEmpty ? null : title,
            artist: artist.isEmpty ? null : artist,
            album: album.isEmpty ? null : album,
            durationMs: durationMs,
          ),
        ),
      );
    }

    return out;
  }

  static List<dynamic> _extractItems(dynamic decoded) {
    if (decoded is List) return decoded;
    if (decoded is! Map) return const [];
    final map = decoded.cast<String, dynamic>();
    for (final key in const ['items', 'tracks', 'results', 'data', 'list']) {
      final v = map[key];
      if (v is List) return v;
    }
    return const [];
  }

  static String _pickString(Map<String, dynamic> map, List<String> keys) {
    for (final k in keys) {
      final v = map[k];
      if (v is String && v.isNotEmpty) return v;
    }
    return '';
  }

  static int? _pickInt(Map<String, dynamic> map, List<String> keys) {
    for (final k in keys) {
      final v = map[k];
      if (v is int) return v;
      if (v is num) return v.toInt();
      if (v is String) {
        final parsed = int.tryParse(v);
        if (parsed != null) return parsed;
      }
    }
    return null;
  }

  static String _toJsonString(dynamic value) {
    if (value is String) return value;
    try {
      return jsonEncode(value);
    } catch (_) {
      return '{}';
    }
  }

  static String? _nullableString(String value) => value.isEmpty ? null : value;
}

class _SourceItem {
  const _SourceItem({required this.queueItem});

  final QueueItem queueItem;
}

enum _SourceAction { play, enqueue }
