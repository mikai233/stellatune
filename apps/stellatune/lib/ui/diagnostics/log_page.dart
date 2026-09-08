import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:window_manager/window_manager.dart';

import 'log_record_view.dart';

import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

class DiagnosticsPage extends StatefulWidget {
  const DiagnosticsPage({super.key, required this.service});
  final DiagnosticsService service;
  @override
  State<DiagnosticsPage> createState() => _DiagnosticsPageState();
}

class _DiagnosticsPageState extends State<DiagnosticsPage> {
  final _scroll = ScrollController();
  String _level = '', _source = '', _search = '';
  String? _session;
  List<String> _sessions = [];
  List<LogRecord> _history = [];
  LogRecord? _detail;
  int? _next = 0;
  int _request = 0;
  bool _loading = false, _follow = true;
  double _clearBefore = 0;
  Timer? _searchTimer;
  Timer? _detailRetry;
  String? _focused;
  String? _status;
  bool _selecting = false;
  int _focusRevision = -1;
  DateTime _lastSearchRefresh = DateTime.fromMillisecondsSinceEpoch(0);
  bool get _searchingLive =>
      _session == null && service.connected && _search.isNotEmpty;
  bool get _usingHistory => _session != null || _searchingLive;
  DiagnosticsService get service => widget.service;
  String text(String zh, String en) => service.chinese ? zh : en;
  @override
  void initState() {
    super.initState();
    service.revision.addListener(_updated);
    _loadSessions();
    _updated();
  }

  Future<void> _loadSessions() async {
    try {
      final sessions = await service.sessions();
      if (mounted) setState(() => _sessions = sessions);
    } catch (_) {
      if (mounted) {
        setState(() => _status = text('历史日志暂不可用', 'History unavailable'));
      }
    }
  }

  void _updated() {
    if (!mounted) return;
    if (_focusRevision != service.focusRevision &&
        service.focusedId != null &&
        !_selecting) {
      _focusRevision = service.focusRevision;
      _focused = service.focusedId;
      unawaited(_selectId(_focused!));
    }
    if (_searchingLive &&
        !_loading &&
        DateTime.now().difference(_lastSearchRefresh) >
            const Duration(seconds: 1)) {
      unawaited(_loadHistory());
    }
    setState(() {});
    if (_follow && _session == null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && _scroll.hasClients) {
          _scroll.jumpTo(_scroll.position.maxScrollExtent);
        }
      });
    }
  }

  Future<void> _selectId(String id, {bool retry = true}) async {
    _detailRetry?.cancel();
    _selecting = true;
    final detail = await service.detail(id);
    _selecting = false;
    if (mounted && _focused == id) {
      // A notification can precede its batch. Retry once, never on every log:
      // a missing diagnostic lookup can itself produce another log record.
      if (detail == null && retry) {
        _detailRetry = Timer(const Duration(milliseconds: 200), () {
          if (mounted && _focused == id) unawaited(_selectId(id, retry: false));
        });
      }
      setState(() {
        _detail = detail;
        _status = detail == null
            ? text('此记录已过期或未写入文件', 'Record expired or was not persisted')
            : null;
      });
    }
  }

  Future<void> _loadHistory({bool reset = true}) async {
    if (!_usingHistory) {
      _request++;
      setState(() {
        _history = [];
        _loading = false;
      });
      return;
    }
    final request = ++_request;
    _lastSearchRefresh = DateTime.now();
    setState(() {
      _loading = true;
      if (reset) {
        _history = [];
        _next = 0;
      }
    });
    try {
      final page = await service.query(
        _session ?? service.session!,
        _next ?? 0,
        _level,
        _source,
        _search,
      );
      if (mounted && request == _request) {
        setState(() {
          _history = [..._history, ...page.records];
          _next = page.nextOffset;
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() => _status = text('无法读取历史日志', 'Unable to read history'));
      }
    } finally {
      if (mounted && request == _request) setState(() => _loading = false);
    }
  }

  @override
  void dispose() {
    _detailRetry?.cancel();
    _searchTimer?.cancel();
    service.revision.removeListener(_updated);
    _scroll.dispose();
    super.dispose();
  }

  Future<void> _export() async {
    try {
      final directory = await Directory.systemTemp.createTemp(
        'stellatune-log-export-',
      );
      try {
        final file = File('${directory.path}/logs.txt');
        await service.export(
          _session ?? service.session ?? 'startup',
          file.path,
        );
        final path = await FilePicker.saveFile(
          dialogTitle: text('导出日志', 'Export logs'),
          fileName: 'stellatune-logs.txt',
          bytes: await file.readAsBytes(),
        );
        if (path == null) return;
      } finally {
        await directory.delete(recursive: true);
      }
      if (mounted) setState(() => _status = text('日志已导出', 'Logs exported'));
    } catch (_) {
      if (mounted) {
        setState(
          () => _status = text(
            '导出失败，请检查保存位置',
            'Export failed. Check the destination',
          ),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final records = (_usingHistory ? _history : service.records)
        .where(
          (r) =>
              r.timestampMs > _clearBefore &&
              (_level.isEmpty || r.level == _level) &&
              (_source.isEmpty || r.source == _source) &&
              (_usingHistory ||
                  _search.isEmpty ||
                  '${r.message} ${r.details} ${r.target}'
                      .toLowerCase()
                      .contains(_search.toLowerCase())),
        )
        .toList();
    final detail = _detail;
    final nativeDesktop =
        Platform.isWindows || Platform.isLinux || Platform.isMacOS;
    Widget select(
      String label,
      String value,
      Map<String, String> choices,
      double width,
      ValueChanged<String> changed,
    ) => SizedBox(
      width: width,
      height: 44,
      child: DropdownButtonFormField<String>(
        key: ValueKey((label, value)),
        initialValue: value,
        isExpanded: true,
        decoration: InputDecoration(
          contentPadding: const EdgeInsets.symmetric(
            horizontal: 12,
            vertical: 10,
          ),
          isDense: true,
        ),
        items: choices.entries
            .map(
              (e) => DropdownMenuItem(
                value: e.key,
                child: Text(
                  e.value,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontSize: 13),
                ),
              ),
            )
            .toList(),
        onChanged: (v) {
          if (v != null) {
            changed(v);
            _loadHistory();
          }
        },
      ),
    );
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): service.close,
      },
      child: Focus(
        autofocus: true,
        child: Material(
          key: const ValueKey('diagnostics-page'),
          color: scheme.surface,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(
                height: 68,
                child: Row(
                  children: [
                    const SizedBox(width: 12),
                    IconButton(
                      tooltip: text('返回播放器', 'Back to player'),
                      onPressed: service.close,
                      icon: const Icon(Icons.arrow_back),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: GestureDetector(
                        behavior: HitTestBehavior.opaque,
                        onPanStart: nativeDesktop
                            ? (_) => windowManager.startDragging()
                            : null,
                        child: Align(
                          alignment: Alignment.centerLeft,
                          child: Text(
                            text('应用日志', 'Application logs'),
                            style: Theme.of(context).textTheme.titleLarge
                                ?.copyWith(fontWeight: FontWeight.w600),
                          ),
                        ),
                      ),
                    ),
                    if (nativeDesktop) const SizedBox(width: 126),
                    const SizedBox(width: 12),
                  ],
                ),
              ),
              const Divider(height: 1),
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 16, 20, 14),
                child: Column(
                  children: [
                    TextField(
                      decoration: InputDecoration(
                        isDense: true,
                        prefixIcon: const Icon(Icons.search, size: 20),
                        hintText: text(
                          '搜索消息、模块、错误详情…',
                          'Search messages, modules, details…',
                        ),
                      ),
                      onChanged: (v) {
                        _searchTimer?.cancel();
                        _searchTimer = Timer(
                          const Duration(milliseconds: 200),
                          () {
                            if (mounted) {
                              setState(() => _search = v);
                              _loadHistory();
                            }
                          },
                        );
                      },
                    ),
                    const SizedBox(height: 12),
                    LayoutBuilder(
                      builder: (context, constraints) => Align(
                        alignment: Alignment.centerLeft,
                        child: Wrap(
                          spacing: 10,
                          runSpacing: 10,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: [
                            select(
                              'session',
                              _session ?? '',
                              {
                                '': text('当前会话 · 实时', 'Current session · Live'),
                                for (final s in _sessions) s: s,
                              },
                              constraints.maxWidth < 620
                                  ? constraints.maxWidth
                                  : 250,
                              (v) => setState(() {
                                _session = v.isEmpty ? null : v;
                                _clearBefore = 0;
                              }),
                            ),
                            select(
                              'level',
                              _level,
                              {
                                '': text('所有级别', 'All levels'),
                                for (final l in [
                                  'TRACE',
                                  'DEBUG',
                                  'INFO',
                                  'WARN',
                                  'ERROR',
                                ])
                                  l: l,
                              },
                              132,
                              (v) => setState(() => _level = v),
                            ),
                            select(
                              'source',
                              _source,
                              {
                                '': text('所有来源', 'All sources'),
                                'flutter': 'Flutter',
                                'rust': 'Rust',
                                'plugin': text('插件', 'Plugins'),
                              },
                              132,
                              (v) => setState(() => _source = v),
                            ),
                            IconButton(
                              tooltip: text('跟随最新日志', 'Follow latest'),
                              isSelected: _follow,
                              onPressed: () =>
                                  setState(() => _follow = !_follow),
                              icon: const Icon(
                                Icons.vertical_align_bottom,
                                size: 20,
                              ),
                            ),
                            IconButton(
                              tooltip: text('清空当前视图', 'Clear view'),
                              onPressed: () => setState(() {
                                _clearBefore = DateTime.now()
                                    .millisecondsSinceEpoch
                                    .toDouble();
                                _detail = null;
                              }),
                              icon: const Icon(Icons.clear_all, size: 20),
                            ),
                            IconButton(
                              tooltip: text('导出会话', 'Export session'),
                              onPressed: _export,
                              icon: const Icon(
                                Icons.file_download_outlined,
                                size: 20,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 20,
                  vertical: 10,
                ),
                color: scheme.surfaceContainerLow,
                child: Row(
                  children: [
                    Icon(
                      Icons.circle,
                      size: 7,
                      color: _follow ? scheme.primary : scheme.outline,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        _status ??
                            text(
                              '${records.length} 条记录',
                              '${records.length} records',
                            ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontSize: 12,
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ),
                    Text(
                      text(
                        _follow ? '自动跟随' : '已暂停跟随',
                        _follow ? 'Following' : 'Follow paused',
                      ),
                      style: TextStyle(
                        fontSize: 12,
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              if (_loading) const LinearProgressIndicator(minHeight: 2),
              Expanded(
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    final list = records.isEmpty && !_loading
                        ? Center(
                            child: Column(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                Icon(
                                  Icons.receipt_long_outlined,
                                  size: 32,
                                  color: scheme.outline,
                                ),
                                const SizedBox(height: 12),
                                Text(
                                  text('暂无符合条件的日志', 'No matching logs'),
                                  style: TextStyle(
                                    color: scheme.onSurfaceVariant,
                                  ),
                                ),
                              ],
                            ),
                          )
                        : ListView.builder(
                            controller: _scroll,
                            itemCount:
                                records.length +
                                (_usingHistory && _next != null ? 1 : 0),
                            itemBuilder: (context, index) {
                              if (index == records.length) {
                                return TextButton(
                                  onPressed: _loading
                                      ? null
                                      : () => _loadHistory(reset: false),
                                  child: Text(text('加载更多', 'Load more')),
                                );
                              }
                              final r = records[index];
                              return LogRecordTile(
                                record: r,
                                selected: detail?.id == r.id,
                                onTap: () {
                                  _focused = r.id;
                                  _selectId(r.id);
                                },
                              );
                            },
                          );
                    if (detail == null) return list;
                    final details = LogRecordDetails(
                      record: detail,
                      chinese: service.chinese,
                      onClose: () => setState(() {
                        _detail = null;
                      }),
                    );
                    return constraints.maxWidth >= 960
                        ? Row(
                            children: [
                              Expanded(flex: 5, child: list),
                              const VerticalDivider(width: 1),
                              Expanded(flex: 5, child: details),
                            ],
                          )
                        : details;
                  },
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
