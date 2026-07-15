import 'dart:convert';
import 'dart:io';

import 'system_proxy_contract.dart';
import 'system_proxy_linux.dart';
import 'system_proxy_macos.dart';
import 'system_proxy_windows.dart';

export 'system_proxy_contract.dart'
    show ProxyBackend, ProxySnapshot, SystemProxyOptions;

/// Restorable desktop system-proxy lifecycle.
class SystemProxy {
  SystemProxy({ProxyBackend? backend, ProxyBackupStore? store})
    : _backend = backend ?? _platformBackend(),
      _store = store ?? FileProxyBackupStore.defaultForPlatform();

  final ProxyBackend _backend;
  final ProxyBackupStore _store;
  Future<void> _pending = Future.value();

  Future<void> enable(
    int port, {
    List<String> bypassHosts = const ['localhost', '127.0.0.1', '::1'],
  }) => _serialize(() => _enable(port, bypassHosts));

  Future<void> _enable(int port, List<String> bypassHosts) async {
    if (port < 1 || port > 65535) throw ArgumentError.value(port, 'port');
    if (bypassHosts.any((host) => host.trim().isEmpty)) {
      throw ArgumentError.value(bypassHosts, 'bypassHosts');
    }

    await _restorePending();
    final original = await _backend.capture();
    await _store.write(_recoverySnapshot(original));
    try {
      await _backend.enable(
        port,
        bypassHosts: List<String>.unmodifiable(bypassHosts),
      );
      final applied = await _backend.capture();
      await _store.write(_recoverySnapshot(original, applied: applied));
    } on Object {
      try {
        await _backend.restore(original);
        await _store.clear();
      } on Object {
        // Preserve the backup so the next launch can retry recovery.
      }
      rethrow;
    }
  }

  Future<void> disable() => _serialize(_restorePending);

  Future<void> _serialize(Future<void> Function() operation) {
    final result = _pending.then(
      (_) => operation(),
      onError: (_) => operation(),
    );
    _pending = result.then<void>((_) {}, onError: (_, _) {});
    return result;
  }

  Future<void> _restorePending() async {
    final snapshot = await _store.read();
    if (snapshot == null) return;
    if (snapshot.backend != _backend.id) {
      throw StateError('System proxy backup belongs to ${snapshot.backend}.');
    }
    final originalValues = snapshot.values['original'];
    final original = originalValues is Map<String, Object?>
        ? ProxySnapshot(backend: snapshot.backend, values: originalValues)
        : snapshot;
    final appliedValues = snapshot.values['applied'];
    if (appliedValues is Map<String, Object?>) {
      final current = await _backend.capture();
      if (!_deepEquals(current.values, appliedValues)) {
        throw StateError(
          'System proxy changed outside Velum; original settings were not overwritten.',
        );
      }
    }
    await _backend.restore(original);
    await _store.clear();
  }

  ProxySnapshot _recoverySnapshot(
    ProxySnapshot original, {
    ProxySnapshot? applied,
  }) => ProxySnapshot(
    backend: original.backend,
    values: {'original': original.values, 'applied': applied?.values},
  );

  static ProxyBackend _platformBackend() {
    if (Platform.isLinux) return LinuxProxyBackend();
    if (Platform.isMacOS) return MacosProxyBackend();
    if (Platform.isWindows) return WindowsProxyBackend();
    throw UnsupportedError('System proxy is not supported on this platform.');
  }
}

bool _deepEquals(Object? left, Object? right) {
  if (left is Map && right is Map) {
    if (left.length != right.length) return false;
    return left.entries.every(
      (entry) =>
          right.containsKey(entry.key) &&
          _deepEquals(entry.value, right[entry.key]),
    );
  }
  if (left is List && right is List) {
    if (left.length != right.length) return false;
    for (var index = 0; index < left.length; index++) {
      if (!_deepEquals(left[index], right[index])) return false;
    }
    return true;
  }
  return left == right;
}

abstract interface class ProxyBackupStore {
  Future<ProxySnapshot?> read();
  Future<void> write(ProxySnapshot snapshot);
  Future<void> clear();
}

class FileProxyBackupStore implements ProxyBackupStore {
  FileProxyBackupStore(this.file);

  factory FileProxyBackupStore.defaultForPlatform() {
    final home = Platform.environment['HOME'];
    final appData = Platform.environment['APPDATA'];
    final directory = Platform.isWindows && appData != null
        ? appData
        : home == null
        ? Directory.systemTemp.path
        : Platform.isMacOS
        ? '$home/Library/Application Support'
        : Platform.environment['XDG_STATE_HOME'] ?? '$home/.local/state';
    return FileProxyBackupStore(
      File('$directory/Velum/system-proxy-backup.json'),
    );
  }

  final File file;

  @override
  Future<ProxySnapshot?> read() async {
    if (!await file.exists()) return null;
    final value = jsonDecode(await file.readAsString());
    if (value is! Map<String, Object?>) {
      throw const FormatException('Invalid system proxy backup.');
    }
    return ProxySnapshot.fromJson(value);
  }

  @override
  Future<void> write(ProxySnapshot snapshot) async {
    await file.parent.create(recursive: true);
    final temporary = File('${file.path}.tmp');
    await temporary.writeAsString(jsonEncode(snapshot.toJson()), flush: true);
    if (await file.exists()) await file.delete();
    await temporary.rename(file.path);
  }

  @override
  Future<void> clear() async {
    if (await file.exists()) await file.delete();
  }
}
