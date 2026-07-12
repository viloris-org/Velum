class BridgeResult {
  const BridgeResult({
    required this.success,
    required this.output,
    this.exitCode = 0,
  });

  final bool success;
  final String output;
  final int exitCode;
}

abstract class VelumBridge {
  bool get supportsLocalCommands;
  String get adapterName;

  Future<BridgeResult> run({
    required String binaryPath,
    required List<String> arguments,
  });
}
