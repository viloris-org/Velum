import 'dart:async';

import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';

import 'client_theme.dart';

final class EnrollmentScannerPage extends StatefulWidget {
  const EnrollmentScannerPage({super.key});

  @override
  State<EnrollmentScannerPage> createState() => _EnrollmentScannerPageState();
}

class _EnrollmentScannerPageState extends State<EnrollmentScannerPage> {
  final _controller = MobileScannerController(
    detectionSpeed: DetectionSpeed.noDuplicates,
    formats: const [BarcodeFormat.qrCode],
  );
  var _handled = false;

  void _handleCapture(BarcodeCapture capture) {
    if (_handled || capture.barcodes.isEmpty) return;
    final value = capture.barcodes.first.rawValue;
    if (value == null || value.isEmpty) return;
    _handled = true;
    unawaited(_controller.stop());
    Navigator.of(context).pop(value);
  }

  @override
  void dispose() {
    unawaited(_controller.dispose());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    backgroundColor: Colors.black,
    appBar: AppBar(
      backgroundColor: Colors.black,
      foregroundColor: Colors.white,
      title: const Text('Scan enrollment'),
      actions: [
        ValueListenableBuilder(
          valueListenable: _controller,
          builder: (context, state, _) => IconButton(
            tooltip: state.torchState == TorchState.on
                ? 'Turn flashlight off'
                : 'Turn flashlight on',
            onPressed: state.isInitialized
                ? () => unawaited(_controller.toggleTorch())
                : null,
            icon: Icon(
              state.torchState == TorchState.on
                  ? Icons.flash_off_rounded
                  : Icons.flash_on_rounded,
            ),
          ),
        ),
      ],
    ),
    body: Stack(
      fit: StackFit.expand,
      children: [
        MobileScanner(
          controller: _controller,
          onDetect: _handleCapture,
          errorBuilder: (context, error) => Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(
                  Icons.no_photography_rounded,
                  color: Colors.white,
                  size: 44,
                ),
                const SizedBox(height: 12),
                const Text(
                  'Camera unavailable',
                  style: TextStyle(color: Colors.white),
                ),
                const SizedBox(height: 12),
                TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: const Text('Close'),
                ),
              ],
            ),
          ),
        ),
        IgnorePointer(
          child: Center(
            child: Semantics(
              label: 'Place the enrollment QR code inside the frame',
              child: Container(
                width: 260,
                height: 260,
                decoration: BoxDecoration(
                  border: Border.all(color: ClientTheme.accent, width: 3),
                  borderRadius: BorderRadius.circular(8),
                ),
              ),
            ),
          ),
        ),
      ],
    ),
  );
}
