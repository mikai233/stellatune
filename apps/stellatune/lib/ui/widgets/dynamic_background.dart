import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';

class ShaderBackground extends StatefulWidget {
  const ShaderBackground({
    super.key,
    required this.colors,
    required this.child,
    this.animate = true, // Added animate toggle
  });

  /// 4 colors to be used in the shader.
  /// Usually extracted from the album cover.
  final List<Color> colors;
  final Widget child;
  final bool animate;

  @override
  State<ShaderBackground> createState() => _ShaderBackgroundState();
}

class _ShaderBackgroundState extends State<ShaderBackground>
    with TickerProviderStateMixin {
  FragmentShader? _shader;
  late Ticker _ticker;
  final ValueNotifier<double> _timeNotifier = ValueNotifier(0.0);

  // Animation controllers for color smoothing
  late AnimationController _colorController;
  late List<ColorTween> _colorTweens;
  List<Color> _currentColors = [];

  @override
  void initState() {
    super.initState();
    _currentColors = widget.colors;

    _colorController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 800),
    );

    _setupTweens(widget.colors);

    _loadShader();
    _ticker = createTicker((elapsed) {
      _timeNotifier.value = elapsed.inMilliseconds / 1000.0;
    });
    if (widget.animate) {
      _ticker.start();
    }
  }

  void _setupTweens(List<Color> newColors) {
    _colorTweens = List.generate(4, (i) {
      return ColorTween(
        begin: _currentColors.length > i ? _currentColors[i] : newColors[i],
        end: newColors[i],
      );
    });
    _colorController.forward(from: 0.0);
  }

  @override
  void didUpdateWidget(ShaderBackground oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.colors != oldWidget.colors) {
      _currentColors = _colorTweens
          .map((t) => t.evaluate(_colorController) ?? t.end!)
          .toList();
      _setupTweens(widget.colors);
    }
    if (widget.animate != oldWidget.animate) {
      if (widget.animate) {
        _ticker.start();
      } else {
        _ticker.stop();
      }
    }
  }

  Future<void> _loadShader() async {
    try {
      final program = await FragmentProgram.fromAsset(
        'assets/shaders/background.frag',
      );
      if (mounted) {
        setState(() {
          _shader = program.fragmentShader();
        });
      }
    } catch (e) {
      debugPrint('Error loading shader: $e');
    }
  }

  @override
  void dispose() {
    _ticker.dispose();
    _colorController.dispose();
    _timeNotifier.dispose();
    _shader?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_shader == null) {
      return Container(color: Colors.black, child: widget.child);
    }

    return RepaintBoundary(
      child: AnimatedBuilder(
        animation: Listenable.merge([_colorController, _timeNotifier]),
        builder: (context, child) {
          final animatedColors = _colorTweens
              .map((t) => t.evaluate(_colorController) ?? t.end!)
              .toList();

          return CustomPaint(
            painter: _ShaderPainter(
              shader: _shader!,
              time: _timeNotifier.value,
              colors: animatedColors,
            ),
            child: child,
          );
        },
        child: widget.child,
      ),
    );
  }
}

class _ShaderPainter extends CustomPainter {
  _ShaderPainter({
    required this.shader,
    required this.time,
    required this.colors,
  });

  final FragmentShader shader;
  final double time;
  final List<Color> colors;

  @override
  void paint(Canvas canvas, Size size) {
    shader.setFloat(0, size.width);
    shader.setFloat(1, size.height);
    shader.setFloat(2, time);

    // Set 4 colors
    for (int i = 0; i < 4; i++) {
      final color = colors[i];
      shader.setFloat(3 + i * 4, color.r);
      shader.setFloat(4 + i * 4, color.g);
      shader.setFloat(5 + i * 4, color.b);
      shader.setFloat(6 + i * 4, 1.0); // Alpha
    }

    final paint = Paint()..shader = shader;
    canvas.drawRect(Offset.zero & size, paint);
  }

  @override
  bool shouldRepaint(_ShaderPainter oldDelegate) => true;
}
