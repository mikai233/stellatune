import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:path/path.dart' as p;
import 'package:window_manager/window_manager.dart';
import 'package:url_launcher/url_launcher.dart';

import 'log_record_view.dart';

import 'package:stellatune/ui/widgets/app_select.dart';

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
  LogRecord? _detail;
  bool _follow = true;
  double _clearBefore = 0;
  Timer? _searchTimer;
  Timer? _detailRetry;
  String? _focused;
  String? _status;
  bool _selecting = false;
  int _focusRevision = -1;
  Object? _recordsKey;
  List<LogRecord> _filteredRecords = [];
  bool _followScheduled = false;
  DiagnosticsService get service => widget.service;
  String text(String zh, String en) => service.chinese ? zh : en;
  @override
  void initState() {
    super.initState();
    service.revision.addListener(_updated);
    _updated();
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
    setState(() {});
    _scheduleFollow();
  }

  void _scheduleFollow() {
    if (_follow && !_followScheduled) {
      _followScheduled = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _followScheduled = false;
        if (mounted && _follow && _scroll.hasClients) {
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
        await service.export(service.session ?? 'startup', file.path);
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

  Future<void> _openLogDirectory() async {
    final path = service.logDirectory;
    if (path == null) return;
    try {
      if (!await Directory(path).exists()) {
        throw FileSystemException('Log directory is unavailable', path);
      }
      if (Platform.isWindows) {
        // Explorer needs a native path even when Dart accepts mixed separators.
        final nativePath = p.windows.normalize(Directory(path).absolute.path);
        await Process.start('explorer.exe', [
          nativePath,
        ], mode: ProcessStartMode.detached);
      } else if (!await launchUrl(Uri.directory(path))) {
        throw StateError('Unable to open log directory');
      }
    } catch (_) {
      if (mounted) {
        setState(
          () => _status = text('无法打开日志文件夹', 'Unable to open log folder'),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final key = (
      service.recordsRevision,
      _level,
      _source,
      _search,
      _clearBefore,
    );
    if (_recordsKey != key) {
      _recordsKey = key;
      final query = _search.toLowerCase();
      _filteredRecords = service.records
          .where(
            (r) =>
                r.timestampMs > _clearBefore &&
                (_level.isEmpty || r.level == _level) &&
                (_source.isEmpty || r.source == _source) &&
                (_search.isEmpty ||
                    '${r.message} ${r.details} ${r.target}'
                        .toLowerCase()
                        .contains(query)),
          )
          .toList();
    }
    final records = _filteredRecords;
    final detail = _detail;
    final nativeDesktop =
        Platform.isWindows || Platform.isLinux || Platform.isMacOS;
    Widget select(
      String label,
      String value,
      Map<String, String> choices,
      double width,
      ValueChanged<String> changed,
    ) => AppSelect<String>(
      key: ValueKey(label),
      value: value,
      items: [
        for (final choice in choices.entries)
          DropdownMenuItem(value: choice.key, child: Text(choice.value)),
      ],
      width: width,
      onChanged: (v) {
        changed(v!);
      },
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
                              onPressed: () {
                                setState(() => _follow = !_follow);
                                _scheduleFollow();
                              },
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
                              tooltip: text('导出当前日志', 'Export current logs'),
                              onPressed: _export,
                              icon: const Icon(
                                Icons.file_download_outlined,
                                size: 20,
                              ),
                            ),
                            TextButton.icon(
                              onPressed: service.logDirectory == null
                                  ? null
                                  : _openLogDirectory,
                              icon: const Icon(
                                Icons.folder_open_outlined,
                                size: 20,
                              ),
                              label: Text(text('打开日志文件夹', 'Open log folder')),
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
              Expanded(
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    final list = records.isEmpty
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
                            itemExtent: LogRecordTile.extent(context),
                            itemCount: records.length,
                            itemBuilder: (context, index) {
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
