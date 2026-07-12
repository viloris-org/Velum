import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../../theme/velum_theme.dart';

class ContinuityVisual extends StatefulWidget {
  const ContinuityVisual({super.key});

  @override
  State<ContinuityVisual> createState() => _ContinuityVisualState();
}

class _ContinuityVisualState extends State<ContinuityVisual>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(seconds: 5),
    )..repeat();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final reduceMotion = MediaQuery.disableAnimationsOf(context);
    return RepaintBoundary(
      child: CustomPaint(
        painter: _ContinuityPainter(
          animation: reduceMotion
              ? const AlwaysStoppedAnimation<double>(0.34)
              : _controller,
        ),
        child: const SizedBox.expand(),
      ),
    );
  }
}

class _ContinuityPainter extends CustomPainter {
  _ContinuityPainter({required this.animation}) : super(repaint: animation);

  final Animation<double> animation;

  @override
  void paint(Canvas canvas, Size size) {
    final progress = animation.value;
    final center = Offset(size.width * 0.5, size.height * 0.51);
    final minSide = math.min(size.width, size.height);
    final outer = minSide * 0.37;
    final inner = minSide * 0.23;
    final gridPaint = Paint()
      ..color = VelumColors.line.withValues(alpha: 0.55)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1;

    for (var i = 1; i <= 4; i++) {
      canvas.drawCircle(center, outer * i / 4, gridPaint);
    }
    canvas.drawLine(
      Offset(center.dx - outer - 18, center.dy),
      Offset(center.dx + outer + 18, center.dy),
      gridPaint,
    );
    canvas.drawLine(
      Offset(center.dx, center.dy - outer - 18),
      Offset(center.dx, center.dy + outer + 18),
      gridPaint,
    );

    final quicPath = Path()
      ..moveTo(center.dx - outer, center.dy + 10)
      ..cubicTo(
        center.dx - inner,
        center.dy - outer,
        center.dx + inner,
        center.dy + outer,
        center.dx + outer,
        center.dy - 14,
      );
    final tlsPath = Path()
      ..moveTo(center.dx - outer, center.dy - 10)
      ..cubicTo(
        center.dx - inner,
        center.dy + outer,
        center.dx + inner,
        center.dy - outer,
        center.dx + outer,
        center.dy + 14,
      );

    final pathPaint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2.2
      ..strokeCap = StrokeCap.round;
    pathPaint.color = VelumColors.aqua.withValues(alpha: 0.75);
    canvas.drawPath(quicPath, pathPaint);
    pathPaint.color = VelumColors.amber.withValues(alpha: 0.65);
    canvas.drawPath(tlsPath, pathPaint);

    _drawMovingDot(canvas, quicPath, progress, VelumColors.aqua);
    _drawMovingDot(canvas, tlsPath, (progress + 0.48) % 1, VelumColors.amber);

    canvas.drawCircle(
      center,
      inner * 0.45,
      Paint()..color = VelumColors.ink.withValues(alpha: 0.88),
    );
    canvas.drawCircle(
      center,
      inner * 0.45,
      Paint()
        ..color = VelumColors.aqua.withValues(alpha: 0.34)
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5,
    );
    canvas.drawCircle(center, 5, Paint()..color = VelumColors.aqua);

    _text(
      canvas,
      'SESSION',
      center.translate(0, -23),
      VelumColors.muted,
      9,
      true,
    );
    _text(
      canvas,
      'CONTINUOUS',
      center.translate(0, 23),
      VelumColors.mist,
      11,
      true,
    );
    _label(canvas, 'QUIC / UDP', Offset(14, 14), VelumColors.aqua);
    _label(
      canvas,
      'TLS / TCP',
      Offset(size.width - 98, size.height - 30),
      VelumColors.amber,
    );
  }

  void _drawMovingDot(Canvas canvas, Path path, double value, Color color) {
    final metric = path.computeMetrics().first;
    final tangent = metric.getTangentForOffset(metric.length * value);
    if (tangent == null) return;
    canvas.drawCircle(
      tangent.position,
      10,
      Paint()..color = color.withValues(alpha: 0.11),
    );
    canvas.drawCircle(tangent.position, 3.5, Paint()..color = color);
  }

  void _label(Canvas canvas, String text, Offset offset, Color color) {
    final painter = TextPainter(
      text: TextSpan(
        text: text,
        style: TextStyle(
          color: color,
          fontFamily: 'Consolas',
          fontSize: 9,
          fontWeight: FontWeight.w700,
          letterSpacing: 0.8,
        ),
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    final rect = RRect.fromRectAndRadius(
      Rect.fromLTWH(offset.dx, offset.dy, painter.width + 16, 24),
      const Radius.circular(7),
    );
    canvas.drawRRect(rect, Paint()..color = color.withValues(alpha: 0.09));
    canvas.drawRRect(
      rect,
      Paint()
        ..color = color.withValues(alpha: 0.28)
        ..style = PaintingStyle.stroke,
    );
    painter.paint(canvas, offset.translate(8, 6));
  }

  void _text(
    Canvas canvas,
    String text,
    Offset center,
    Color color,
    double size,
    bool bold,
  ) {
    final painter = TextPainter(
      text: TextSpan(
        text: text,
        style: TextStyle(
          color: color,
          fontFamily: 'Consolas',
          fontSize: size,
          fontWeight: bold ? FontWeight.w700 : FontWeight.w400,
          letterSpacing: 1,
        ),
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    painter.paint(
      canvas,
      center - Offset(painter.width / 2, painter.height / 2),
    );
  }

  @override
  bool shouldRepaint(covariant _ContinuityPainter oldDelegate) =>
      oldDelegate.animation != animation;
}
