import 'dart:async';

import 'package:flutter/material.dart';
import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'diagnostics_page_transition.dart';

/// Overlay listeners never rebuild the application/page passed as child.
class DiagnosticsOverlay extends StatefulWidget {
  const DiagnosticsOverlay({super.key, required this.child, this.service});
  final Widget child;
  final DiagnosticsService? service;
  @override
  State<DiagnosticsOverlay> createState() => _DiagnosticsOverlayState();
}

class _DiagnosticsOverlayState extends State<DiagnosticsOverlay> {
  DiagnosticsService get service =>
      widget.service ?? DiagnosticsService.instance;
  Timer? _dismiss;
  @override
  void initState() {
    super.initState();
    service.notice.addListener(_onNotice);
  }

  void _onNotice() {
    _dismiss?.cancel();
    _dismiss = Timer(const Duration(seconds: 6), () {
      service.notice.value = null;
    });
  }

  @override
  void dispose() {
    _dismiss?.cancel();
    service.notice.removeListener(_onNotice);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    service.chinese = Localizations.localeOf(context).languageCode == 'zh';
    return Overlay.wrap(
      child: Stack(
        children: [
          ValueListenableBuilder<bool>(
            valueListenable: service.visible,
            child: widget.child,
            builder: (context, visible, child) => Positioned.fill(
              child: DiagnosticsPageTransition(
                service: service,
                visible: visible,
                child: child!,
              ),
            ),
          ),
          ValueListenableBuilder<ErrorNotice?>(
            valueListenable: service.notice,
            builder: (context, notice, _) => notice == null
                ? const SizedBox.shrink()
                : Positioned(
                    right: 20,
                    top: 68,
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        maxWidth: (MediaQuery.sizeOf(context).width - 40).clamp(
                          200,
                          420,
                        ),
                      ),
                      child: Material(
                        elevation: 6,
                        borderRadius: BorderRadius.circular(12),
                        color: Theme.of(context)
                            .colorScheme
                            .surfaceContainerHigh,
                        child: Padding(
                          padding: const EdgeInsets.all(12),
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              const Icon(Icons.error_outline, size: 20),
                              const SizedBox(width: 10),
                              Flexible(child: Text(notice.message)),
                              TextButton(
                                onPressed: () {
                                  service.notice.value = null;
                                  service.open(notice.logId);
                                },
                                child: Text(
                                  service.chinese ? '查看日志' : 'View logs',
                                ),
                              ),
                              IconButton(
                                tooltip: service.chinese ? '关闭' : 'Dismiss',
                                onPressed: () => service.notice.value = null,
                                icon: const Icon(Icons.close, size: 18),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class DiagnosticsButton extends StatelessWidget {
  const DiagnosticsButton({super.key});
  @override
  Widget build(BuildContext context) {
    final service = DiagnosticsService.instance;
    return ValueListenableBuilder<int>(
      valueListenable: service.unread,
      builder: (context, unread, _) => IconButton(
        tooltip: service.chinese ? '查看日志' : 'View logs',
        onPressed: service.open,
        icon: Badge(
          isLabelVisible: unread > 0,
          label: Text(unread > 99 ? '99+' : '$unread'),
          child: const Icon(Icons.receipt_long_outlined, size: 20),
        ),
      ),
    );
  }
}
