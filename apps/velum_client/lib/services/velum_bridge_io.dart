import 'dart:io';

import 'bridge_contract.dart';

VelumBridge createBridge() => _NativeBridge();

class _NativeBridge implements VelumBridge {
  @override
  bool get supportsLocalCommands => true;

  @override
  String get adapterName => '本机 CLI 适配器';

  @override
  Future<BridgeResult> run({
    required String binaryPath,
    required List<String> arguments,
  }) async {
    try {
      final result = await Process.run(
        binaryPath,
        arguments,
        runInShell: true,
      ).timeout(const Duration(seconds: 12));
      final output = [result.stdout, result.stderr]
          .map((value) => value.toString().trim())
          .where((value) => value.isNotEmpty)
          .join('\n');
      return BridgeResult(
        success: result.exitCode == 0,
        output: output.isEmpty ? '命令已完成。' : output,
        exitCode: result.exitCode,
      );
    } on ProcessException catch (error) {
      return BridgeResult(
        success: false,
        output: '无法启动 $binaryPath：${error.message}',
        exitCode: -1,
      );
    } on Exception catch (error) {
      return BridgeResult(
        success: false,
        output: '命令执行失败：$error',
        exitCode: -1,
      );
    }
  }
}
