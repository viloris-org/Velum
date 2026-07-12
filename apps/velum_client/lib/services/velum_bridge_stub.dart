import 'bridge_contract.dart';

VelumBridge createBridge() => _BrowserBridge();

class _BrowserBridge implements VelumBridge {
  @override
  bool get supportsLocalCommands => false;

  @override
  String get adapterName => '浏览器演示适配器';

  @override
  Future<BridgeResult> run({
    required String binaryPath,
    required List<String> arguments,
  }) async {
    await Future<void>.delayed(const Duration(milliseconds: 450));
    return const BridgeResult(
      success: false,
      output: '浏览器无法直接启动本机进程。请使用 Windows 客户端执行 Velum CLI，或保持演示模式。',
      exitCode: -1,
    );
  }
}
