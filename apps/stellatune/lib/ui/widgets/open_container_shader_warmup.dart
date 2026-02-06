import 'dart:async';
import 'dart:io';

import 'package:animations/animations.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:palette_generator/palette_generator.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/ui/widgets/dynamic_background.dart';

class OpenContainerShaderWarmup extends ConsumerStatefulWidget {
  const OpenContainerShaderWarmup({
    super.key,
    this.enabled = true,
    this.useFullScreenPreview = false,
    this.tinyOverlayAlignment = Alignment.topRight,
    this.tinyOverlayPadding = const EdgeInsets.only(top: 2, right: 2),
    this.cycles = 5,
    this.transitionDuration = const Duration(milliseconds: 220),
    this.timeout = const Duration(milliseconds: 1000),
    this.closedSizeTiny = 1.0,
    this.openSizeTiny = 3.0,
    this.closedSizeFullScreen = 120.0,
    this.paletteResolveAttempts = 6,
    this.paletteRetryDelay = const Duration(milliseconds: 120),
  });

  final bool enabled;
  final bool useFullScreenPreview;
  final Alignment tinyOverlayAlignment;
  final EdgeInsets tinyOverlayPadding;
  final int cycles;
  final Duration transitionDuration;
  final Duration timeout;
  final double closedSizeTiny;
  final double openSizeTiny;
  final double closedSizeFullScreen;
  final int paletteResolveAttempts;
  final Duration paletteRetryDelay;

  @override
  ConsumerState<OpenContainerShaderWarmup> createState() =>
      _OpenContainerShaderWarmupState();
}

class _OpenContainerShaderWarmupState
    extends ConsumerState<OpenContainerShaderWarmup> {
  bool _queued = false;

  bool get _isDesktop =>
      Platform.isWindows || Platform.isLinux || Platform.isMacOS;

  double get _closedSize => widget.useFullScreenPreview
      ? widget.closedSizeFullScreen
      : widget.closedSizeTiny;

  double? get _openSize =>
      widget.useFullScreenPreview ? null : widget.openSizeTiny;

  @override
  void initState() {
    super.initState();
    if (!widget.enabled) return;
    unawaited(ShaderBackground.preloadProgram());
    if (!_isDesktop) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _queued) return;
      _queued = true;
      unawaited(_runWarmup());
    });
  }

  Future<void> _runWarmup() async {
    final logger = ref.read(loggerProvider);
    final stopwatch = Stopwatch()..start();
    final colors = await _resolveWarmupColors();
    if (!mounted) return;
    final overlay = Overlay.maybeOf(context, rootOverlay: true);
    if (overlay == null) return;

    for (int i = 0; i < widget.cycles; i++) {
      if (!mounted) break;
      final done = Completer<void>();
      final entry = OverlayEntry(
        builder: (_) => IgnorePointer(
          child: widget.useFullScreenPreview
              ? SizedBox.expand(
                  child: Navigator(
                    onGenerateRoute: (_) => PageRouteBuilder<void>(
                      transitionDuration: Duration.zero,
                      reverseTransitionDuration: Duration.zero,
                      pageBuilder: (context, animation, secondaryAnimation) =>
                          RepaintBoundary(
                            child: _WarmupProbe(
                              colors: colors,
                              fullscreen: true,
                              closedSize: _closedSize,
                              openSize: _openSize,
                              transitionDuration: widget.transitionDuration,
                              onFinished: () {
                                if (!done.isCompleted) done.complete();
                              },
                            ),
                          ),
                    ),
                  ),
                )
              : Align(
                  alignment: widget.tinyOverlayAlignment,
                  child: Padding(
                    padding: widget.tinyOverlayPadding,
                    child: SizedBox(
                      width: _openSize!,
                      height: _openSize,
                      child: ClipRect(
                        child: Navigator(
                          onGenerateRoute: (_) => PageRouteBuilder<void>(
                            transitionDuration: Duration.zero,
                            reverseTransitionDuration: Duration.zero,
                            pageBuilder:
                                (context, animation, secondaryAnimation) =>
                                    RepaintBoundary(
                                      child: _WarmupProbe(
                                        colors: colors,
                                        fullscreen: false,
                                        closedSize: _closedSize,
                                        openSize: _openSize,
                                        transitionDuration:
                                            widget.transitionDuration,
                                        onFinished: () {
                                          if (!done.isCompleted) {
                                            done.complete();
                                          }
                                        },
                                      ),
                                    ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
        ),
      );

      overlay.insert(entry);
      await done.future.timeout(
        widget.timeout,
        onTimeout: () {
          logger.d(
            '[Warmup] OpenContainer+Shader warmup timeout at '
            'cycle ${i + 1}/${widget.cycles}',
          );
        },
      );
      if (entry.mounted) {
        entry.remove();
      }
    }

    stopwatch.stop();
    logger.d(
      '[Warmup] OpenContainer+Shader warmup completed: '
      'cycles=${widget.cycles}, elapsed=${stopwatch.elapsedMilliseconds}ms, '
      'fullscreen=${widget.useFullScreenPreview}',
    );
  }

  Future<List<Color>> _resolveWarmupColors() async {
    for (int attempt = 0; attempt < widget.paletteResolveAttempts; attempt++) {
      final colors = await _buildWarmupColorsFromCurrentTrack();
      if (colors != null) return colors;
      await Future<void>.delayed(widget.paletteRetryDelay);
      if (!mounted) break;
    }
    return _buildFallbackWarmupColors(Theme.of(context).colorScheme);
  }

  Future<List<Color>?> _buildWarmupColorsFromCurrentTrack() async {
    final queue = ref.read(queueControllerProvider);
    final trackId = queue.currentItem?.id;
    if (trackId == null) return null;

    final coverDir = ref.read(coverDirProvider);
    if (coverDir.isEmpty) return null;

    final coverPath = '$coverDir${Platform.pathSeparator}$trackId';
    final file = File(coverPath);
    if (!await file.exists()) return null;

    try {
      final imageProvider = ResizeImage(
        FileImage(file),
        width: 100,
        height: 100,
      );
      final palette = await PaletteGenerator.fromImageProvider(
        imageProvider,
        maximumColorCount: 24,
      );

      final dominantColor = palette.dominantColor?.color ?? Colors.blueGrey;
      final sortedSwatches = List<PaletteColor>.from(palette.paletteColors)
        ..sort((a, b) => b.population.compareTo(a.population));

      final weightedColors = <Color>[];
      for (int i = 0; i < 4; i++) {
        if (i < sortedSwatches.length) {
          weightedColors.add(sortedSwatches[i].color);
        } else {
          weightedColors.add(i == 0 ? dominantColor : Colors.black);
        }
      }
      return weightedColors;
    } catch (_) {
      return null;
    }
  }

  List<Color> _buildFallbackWarmupColors(ColorScheme scheme) {
    final base = HSLColor.fromColor(scheme.primary);
    return <Color>[
      base
          .withSaturation((base.saturation + 0.22).clamp(0.0, 1.0))
          .withLightness(0.56)
          .toColor(),
      base
          .withHue((base.hue + 80.0) % 360.0)
          .withSaturation((base.saturation + 0.12).clamp(0.0, 1.0))
          .withLightness(0.52)
          .toColor(),
      HSLColor.fromColor(
        scheme.secondary,
      ).withSaturation(0.82).withLightness(0.50).toColor(),
      HSLColor.fromColor(
        scheme.tertiary,
      ).withSaturation(0.88).withLightness(0.54).toColor(),
    ];
  }

  @override
  Widget build(BuildContext context) => const SizedBox.shrink();
}

class _WarmupProbe extends StatefulWidget {
  const _WarmupProbe({
    required this.colors,
    required this.fullscreen,
    required this.closedSize,
    required this.openSize,
    required this.transitionDuration,
    required this.onFinished,
  });

  final List<Color> colors;
  final bool fullscreen;
  final double closedSize;
  final double? openSize;
  final Duration transitionDuration;
  final VoidCallback onFinished;

  @override
  State<_WarmupProbe> createState() => _WarmupProbeState();
}

class _WarmupProbeState extends State<_WarmupProbe> {
  bool _opened = false;
  bool _finishing = false;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: widget.fullscreen ? Alignment.center : Alignment.topLeft,
      child: SizedBox(
        width: widget.closedSize,
        height: widget.closedSize,
        child: OpenContainer(
          closedElevation: 0,
          openElevation: 0,
          closedColor: Colors.black,
          middleColor: Colors.black,
          openColor: Colors.black,
          onClosed: (_) => _finish(),
          useRootNavigator: false,
          closedShape: const RoundedRectangleBorder(
            borderRadius: BorderRadius.all(Radius.circular(8)),
          ),
          openShape: const RoundedRectangleBorder(
            borderRadius: BorderRadius.zero,
          ),
          transitionDuration: widget.transitionDuration,
          transitionType: ContainerTransitionType.fade,
          openBuilder: (context, closeContainer) => _WarmupShaderDetail(
            colors: widget.colors,
            size: widget.openSize,
            closeContainer: closeContainer,
          ),
          closedBuilder: (context, openContainer) {
            if (!_opened) {
              _opened = true;
              WidgetsBinding.instance.addPostFrameCallback((_) {
                if (!mounted) return;
                openContainer();
              });
            }
            return Container(
              decoration: BoxDecoration(
                color: widget.colors.first,
                borderRadius: BorderRadius.circular(8),
              ),
            );
          },
        ),
      ),
    );
  }

  void _finish() {
    if (_finishing) return;
    _finishing = true;
    widget.onFinished();
  }
}

class _WarmupShaderDetail extends StatefulWidget {
  const _WarmupShaderDetail({
    required this.colors,
    required this.size,
    required this.closeContainer,
  });

  final List<Color> colors;
  final double? size;
  final VoidCallback closeContainer;

  @override
  State<_WarmupShaderDetail> createState() => _WarmupShaderDetailState();
}

class _WarmupShaderDetailState extends State<_WarmupShaderDetail> {
  Animation<double>? _routeAnimation;
  bool _closeRequested = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final animation = ModalRoute.of(context)?.animation;
    if (_routeAnimation == animation) {
      _tryCloseAfterOpenCompleted();
      return;
    }
    _routeAnimation?.removeStatusListener(_onRouteStatusChanged);
    _routeAnimation = animation;
    _routeAnimation?.addStatusListener(_onRouteStatusChanged);
    _tryCloseAfterOpenCompleted();
  }

  void _onRouteStatusChanged(AnimationStatus status) {
    if (status == AnimationStatus.completed) {
      _tryCloseAfterOpenCompleted();
    }
  }

  void _tryCloseAfterOpenCompleted() {
    if (!mounted || _closeRequested) return;
    if (_routeAnimation?.status != AnimationStatus.completed) return;
    _closeRequested = true;
    widget.closeContainer();
  }

  @override
  void dispose() {
    _routeAnimation?.removeStatusListener(_onRouteStatusChanged);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final shaderChild = ShaderBackground(
      colors: widget.colors,
      child: const SizedBox.expand(),
    );

    return Material(
      type: MaterialType.transparency,
      child: widget.size == null
          ? shaderChild
          : Center(
              child: SizedBox(
                width: widget.size,
                height: widget.size,
                child: shaderChild,
              ),
            ),
    );
  }
}
