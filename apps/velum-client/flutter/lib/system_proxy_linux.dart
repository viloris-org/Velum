import 'dart:io';

import 'system_proxy_contract.dart';

enum _LinuxDesktop { gnome, mate, kde }

final class LinuxProxyBackend extends CommandProxyBackend {
  LinuxProxyBackend({super.run, Map<String, String>? environment})
    : _environment = environment ?? Platform.environment;

  final Map<String, String> _environment;

  @override
  String get id => 'linux';

  @override
  Future<ProxySnapshot> capture() async {
    final desktop = await _desktop();
    return switch (desktop) {
      _LinuxDesktop.gnome => _captureGSettings('org.gnome.system.proxy'),
      _LinuxDesktop.mate => _captureGSettings('org.mate.system.proxy'),
      _LinuxDesktop.kde => _captureKde(),
    };
  }

  @override
  Future<void> enable(int port) async {
    switch (await _desktop()) {
      case _LinuxDesktop.gnome:
        await _enableGSettings('org.gnome.system.proxy', port);
      case _LinuxDesktop.mate:
        await _enableGSettings('org.mate.system.proxy', port);
      case _LinuxDesktop.kde:
        await _enableKde(port);
    }
  }

  @override
  Future<void> restore(ProxySnapshot snapshot) async {
    final kind = snapshot.values['kind'];
    if (kind == 'gsettings') {
      final settings = snapshot.values['settings'] as Map<String, Object?>;
      for (final entry in settings.entries) {
        final separator = entry.key.indexOf('|');
        await checked('gsettings', [
          'set',
          entry.key.substring(0, separator),
          entry.key.substring(separator + 1),
          entry.value as String,
        ]);
      }
      return;
    }
    if (kind != 'kde') throw const FormatException('Invalid Linux backup.');
    final executable = await _kdeWriter();
    final file = _kdeFile();
    final settings = snapshot.values['settings'] as Map<String, Object?>;
    for (final entry in settings.entries) {
      await checked(executable, [
        '--file',
        file,
        '--group',
        'Proxy Settings',
        '--key',
        entry.key,
        entry.value as String,
      ]);
    }
  }

  Future<ProxySnapshot> _captureGSettings(String schema) async {
    final keys = <String, String>{};
    for (final (subschema, key) in [
      (schema, 'mode'),
      (schema, 'ignore-hosts'),
      ('$schema.https', 'host'),
      ('$schema.https', 'port'),
      ('$schema.socks', 'host'),
      ('$schema.socks', 'port'),
    ]) {
      final result = await checked('gsettings', ['get', subschema, key]);
      keys['$subschema|$key'] = result.stdout.toString().trim();
    }
    return ProxySnapshot(
      backend: id,
      values: {'kind': 'gsettings', 'schema': schema, 'settings': keys},
    );
  }

  Future<void> _enableGSettings(String schema, int port) async {
    for (final (subschema, key, value) in [
      ('$schema.https', 'host', "'127.0.0.1'"),
      ('$schema.https', 'port', '$port'),
      ('$schema.socks', 'host', "'127.0.0.1'"),
      ('$schema.socks', 'port', '$port'),
      (schema, 'ignore-hosts', "['localhost', '127.0.0.0/8', '::1']"),
      (schema, 'mode', "'manual'"),
    ]) {
      await checked('gsettings', ['set', subschema, key, value]);
    }
  }

  Future<ProxySnapshot> _captureKde() async {
    final reader = await _kdeReader();
    final settings = <String, String>{};
    for (final key in ['ProxyType', 'NoProxyFor', 'httpsProxy', 'socksProxy']) {
      final result = await checked(reader, [
        '--file',
        _kdeFile(),
        '--group',
        'Proxy Settings',
        '--key',
        key,
        '--default',
        '',
      ]);
      settings[key] = result.stdout.toString().trim();
    }
    return ProxySnapshot(
      backend: id,
      values: {'kind': 'kde', 'settings': settings},
    );
  }

  Future<void> _enableKde(int port) async {
    final executable = await _kdeWriter();
    for (final (key, value) in [
      ('httpsProxy', 'https://127.0.0.1:$port'),
      ('socksProxy', 'socks://127.0.0.1:$port'),
      ('NoProxyFor', 'localhost,127.0.0.1,::1'),
      ('ProxyType', '1'),
    ]) {
      await checked(executable, [
        '--file',
        _kdeFile(),
        '--group',
        'Proxy Settings',
        '--key',
        key,
        value,
      ]);
    }
  }

  Future<_LinuxDesktop> _desktop() async {
    final value = (_environment['XDG_CURRENT_DESKTOP'] ?? '').toUpperCase();
    if (value.contains('KDE')) return _LinuxDesktop.kde;
    if (value.contains('MATE')) return _LinuxDesktop.mate;
    if (value.contains('GNOME') ||
        value.contains('CINNAMON') ||
        value.contains('BUDGIE') ||
        value.contains('UNITY')) {
      return _LinuxDesktop.gnome;
    }
    if ((await run('gsettings', ['list-schemas'])).exitCode == 0) {
      return _LinuxDesktop.gnome;
    }
    throw UnsupportedError('No supported Linux desktop proxy backend found.');
  }

  String _kdeFile() => '${_environment['HOME']}/.config/kioslaverc';

  Future<String> _kdeWriter() =>
      _firstExecutable(['kwriteconfig6', 'kwriteconfig5']);
  Future<String> _kdeReader() =>
      _firstExecutable(['kreadconfig6', 'kreadconfig5']);

  Future<String> _firstExecutable(List<String> candidates) async {
    for (final candidate in candidates) {
      if ((await run('which', [candidate])).exitCode == 0) return candidate;
    }
    throw UnsupportedError('${candidates.join('/')} is unavailable.');
  }
}
