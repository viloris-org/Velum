import 'package:flutter/material.dart';

import 'client_theme.dart';

class TrafficSample {
  const TrafficSample({
    required this.downloadBytesPerSecond,
    required this.uploadBytesPerSecond,
  });

  final double downloadBytesPerSecond;
  final double uploadBytesPerSecond;
}

class TrafficChart extends StatelessWidget {
  const TrafficChart({
    required this.samples,
    this.height = 180,
    this.showLegend = true,
    this.showAxisLabels = true,
    super.key,
  });

  final List<TrafficSample> samples;
  final double height;
  final bool showLegend;
  final bool showAxisLabels;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          height: height,
          width: double.infinity,
          child: CustomPaint(
            painter: _TrafficChartPainter(
              samples,
              showAxisLabels: showAxisLabels,
            ),
            child: const SizedBox.expand(),
          ),
        ),
        if (showLegend) ...[
          const SizedBox(height: 10),
          const Center(
            child: Wrap(
              spacing: 20,
              children: [
                _TrafficLegend(
                  color: ClientTheme.trafficDownload,
                  label: 'Download',
                ),
                _TrafficLegend(
                  color: ClientTheme.trafficUpload,
                  label: 'Upload',
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }
}

class _TrafficLegend extends StatelessWidget {
  const _TrafficLegend({required this.color, required this.label});

  final Color color;
  final String label;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      Container(width: 14, height: 2, color: color),
      const SizedBox(width: 6),
      Text(
        label,
        style: const TextStyle(color: ClientTheme.muted, fontSize: 11),
      ),
    ],
  );
}

class _TrafficChartPainter extends CustomPainter {
  const _TrafficChartPainter(this.samples, {required this.showAxisLabels});

  final List<TrafficSample> samples;
  final bool showAxisLabels;
  static const _leftInset = 38.0;
  static const _bottomInset = 22.0;
  static const _topInset = 4.0;
  static const _rightInset = 4.0;
  static const _maxBytesPerSecond = 60 * 1024.0;

  @override
  void paint(Canvas canvas, Size size) {
    final chart = Rect.fromLTRB(
      showAxisLabels ? _leftInset : 0,
      _topInset,
      size.width - _rightInset,
      size.height - (showAxisLabels ? _bottomInset : 0),
    );
    _paintGrid(canvas, chart);
    if (samples.isEmpty) {
      _paintZeroTraffic(canvas, chart);
      return;
    }
    _paintSeries(
      canvas,
      chart,
      (sample) => sample.downloadBytesPerSecond,
      ClientTheme.trafficDownload,
    );
    _paintSeries(
      canvas,
      chart,
      (sample) => sample.uploadBytesPerSecond,
      ClientTheme.trafficUpload,
    );
  }

  void _paintGrid(Canvas canvas, Rect chart) {
    final gridPaint = Paint()..color = ClientTheme.trafficGrid;
    final labelPainter = TextPainter(textDirection: TextDirection.ltr);
    const labels = ['60 kB', '40 kB', '20 kB', '0 B'];
    for (var index = 0; index < labels.length; index++) {
      final y = chart.top + chart.height * index / (labels.length - 1);
      canvas.drawLine(Offset(chart.left, y), Offset(chart.right, y), gridPaint);
      if (showAxisLabels) {
        labelPainter.text = TextSpan(
          text: labels[index],
          style: const TextStyle(color: ClientTheme.muted, fontSize: 11),
        );
        labelPainter.layout();
        labelPainter.paint(
          canvas,
          Offset(
            chart.left - labelPainter.width - 12,
            y - labelPainter.height / 2,
          ),
        );
      }
    }
  }

  void _paintSeries(
    Canvas canvas,
    Rect chart,
    double Function(TrafficSample) valueOf,
    Color color,
  ) {
    if (samples.length < 2) return;
    final line = Path();
    final fill = Path();
    for (var index = 0; index < samples.length; index++) {
      final x = chart.left + chart.width * index / (samples.length - 1);
      final normalized = (valueOf(samples[index]) / _maxBytesPerSecond).clamp(
        0.0,
        1.0,
      );
      final y = chart.bottom - chart.height * normalized;
      if (index == 0) {
        line.moveTo(x, y);
        fill.moveTo(x, chart.bottom);
        fill.lineTo(x, y);
      } else {
        line.lineTo(x, y);
        fill.lineTo(x, y);
      }
    }
    fill.lineTo(chart.right, chart.bottom);
    fill.close();
    canvas.drawPath(fill, Paint()..color = color.withValues(alpha: .12));
    canvas.drawPath(
      line,
      Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round,
    );
  }

  void _paintZeroTraffic(Canvas canvas, Rect chart) {
    final paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round;
    paint.color = ClientTheme.trafficDownload;
    canvas.drawLine(
      Offset(chart.left, chart.bottom),
      Offset(chart.right, chart.bottom),
      paint,
    );
    paint.color = ClientTheme.trafficUpload;
    canvas.drawLine(
      Offset(chart.left, chart.bottom - 1),
      Offset(chart.right, chart.bottom - 1),
      paint,
    );
  }

  @override
  bool shouldRepaint(covariant _TrafficChartPainter oldDelegate) =>
      oldDelegate.samples != samples ||
      oldDelegate.showAxisLabels != showAxisLabels;
}
