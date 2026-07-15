import 'dart:io';

typedef ProxyProcessRunner =
    Future<ProcessResult> Function(String executable, List<String> arguments);

class ProxySnapshot {
  const ProxySnapshot({required this.backend, required this.values});

  factory ProxySnapshot.fromJson(Map<String, Object?> json) {
    final backend = json['backend'];
    final values = json['values'];
    if (backend is! String || values is! Map<String, Object?>) {
      throw const FormatException('Invalid system proxy backup fields.');
    }
    return ProxySnapshot(backend: backend, values: values);
  }

  final String backend;
  final Map<String, Object?> values;

  Map<String, Object?> toJson() => {'backend': backend, 'values': values};
}

abstract interface class ProxyBackend {
  String get id;
  Future<ProxySnapshot> capture();
  Future<void> enable(int port);
  Future<void> restore(ProxySnapshot snapshot);
}

abstract base class CommandProxyBackend implements ProxyBackend {
  CommandProxyBackend({ProxyProcessRunner? run}) : run = run ?? _runProcess;

  final ProxyProcessRunner run;

  Future<ProcessResult> checked(String executable, List<String> args) async {
    final result = await run(executable, args);
    if (result.exitCode != 0) {
      throw ProcessException(
        executable,
        args,
        result.stderr.toString(),
        result.exitCode,
      );
    }
    return result;
  }

  static Future<ProcessResult> _runProcess(
    String executable,
    List<String> arguments,
  ) => Process.run(executable, arguments);
}
