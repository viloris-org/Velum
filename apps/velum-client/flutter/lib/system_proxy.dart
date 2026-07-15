import 'dart:io';

/// Desktop system-proxy writer. All mutations are per-user and fail closed.
class SystemProxy {
  Future<void> enable(int port) async {
    if (port < 1 || port > 65535) throw ArgumentError.value(port, 'port');
    if (Platform.isLinux) return _enableLinux(port);
    if (Platform.isMacOS) return _enableMacos(port);
    if (Platform.isWindows) return _enableWindows(port);
    throw UnsupportedError('System proxy is not supported on this platform.');
  }

  Future<void> disable() async {
    if (Platform.isLinux) return _disableLinux();
    if (Platform.isMacOS) return _disableMacos();
    if (Platform.isWindows) return _disableWindows();
  }

  Future<void> _enableLinux(int port) async {
    if (await _isKde()) return _enableKde(port);
    await _run('gsettings', [
      'set',
      'org.gnome.system.proxy',
      'mode',
      'manual',
    ]);
    for (final scheme in ['https', 'socks']) {
      await _run('gsettings', [
        'set',
        'org.gnome.system.proxy.$scheme',
        'host',
        '127.0.0.1',
      ]);
      await _run('gsettings', [
        'set',
        'org.gnome.system.proxy.$scheme',
        'port',
        '$port',
      ]);
    }
    await _run('gsettings', [
      'set',
      'org.gnome.system.proxy',
      'ignore-hosts',
      "['localhost', '127.0.0.0/8', '::1']",
    ]);
  }

  Future<void> _disableLinux() =>
      _isKde().then((kde) => kde ? _disableKde() : _disableGnome());

  Future<void> _disableGnome() =>
      _run('gsettings', ['set', 'org.gnome.system.proxy', 'mode', 'none']);

  Future<bool> _isKde() async =>
      (Platform.environment['XDG_CURRENT_DESKTOP'] ?? '')
          .toUpperCase()
          .contains('KDE');

  Future<void> _enableKde(int port) async {
    final executable = await _kdeConfigWriter();
    final config = '${Platform.environment['HOME']}/.config/kioslaverc';
    await _run(executable, [
      '--file',
      config,
      '--group',
      'Proxy Settings',
      '--key',
      'ProxyType',
      '1',
    ]);
    await _run(executable, [
      '--file',
      config,
      '--group',
      'Proxy Settings',
      '--key',
      'NoProxyFor',
      'localhost,127.0.0.1,::1',
    ]);
    for (final scheme in ['https', 'socks']) {
      await _run(executable, [
        '--file',
        config,
        '--group',
        'Proxy Settings',
        '--key',
        '${scheme}Proxy',
        '$scheme://127.0.0.1:$port',
      ]);
    }
  }

  Future<void> _disableKde() async {
    final executable = await _kdeConfigWriter();
    await _run(executable, [
      '--file',
      '${Platform.environment['HOME']}/.config/kioslaverc',
      '--group',
      'Proxy Settings',
      '--key',
      'ProxyType',
      '0',
    ]);
  }

  Future<String> _kdeConfigWriter() async {
    for (final executable in ['kwriteconfig6', 'kwriteconfig5']) {
      if ((await Process.run('which', [executable])).exitCode == 0) {
        return executable;
      }
    }
    throw UnsupportedError('KDE proxy configuration writer is unavailable.');
  }

  Future<void> _enableMacos(int port) async {
    for (final service in await _macosServices()) {
      for (final kind in ['securewebproxy', 'socksfirewallproxy']) {
        await _run('/usr/sbin/networksetup', [
          '-set$kind',
          service,
          '127.0.0.1',
          '$port',
        ]);
        await _run('/usr/sbin/networksetup', [
          '-set${kind}state',
          service,
          'on',
        ]);
      }
      await _run('/usr/sbin/networksetup', [
        '-setproxybypassdomains',
        service,
        'localhost',
        '127.0.0.1',
        '::1',
      ]);
    }
  }

  Future<void> _disableMacos() async {
    for (final service in await _macosServices()) {
      for (final kind in ['securewebproxy', 'socksfirewallproxy']) {
        await _run('/usr/sbin/networksetup', [
          '-set${kind}state',
          service,
          'off',
        ]);
      }
    }
  }

  Future<List<String>> _macosServices() async {
    final output = await Process.run('/usr/sbin/networksetup', [
      '-listallnetworkservices',
    ]);
    if (output.exitCode != 0) {
      throw ProcessException(
        '/usr/sbin/networksetup',
        ['-listallnetworkservices'],
        output.stderr.toString(),
        output.exitCode,
      );
    }
    return output.stdout
        .toString()
        .split('\n')
        .map((line) => line.trim())
        .where(
          (line) =>
              line.isNotEmpty &&
              !line.startsWith('*') &&
              !line.startsWith('An asterisk'),
        )
        .toList();
  }

  Future<void> _enableWindows(int port) async {
    await _run('reg.exe', [
      'ADD',
      r'HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings',
      '/v',
      'ProxyServer',
      '/t',
      'REG_SZ',
      '/d',
      'https=127.0.0.1:$port;socks=127.0.0.1:$port',
      '/f',
    ]);
    await _run('reg.exe', [
      'ADD',
      r'HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings',
      '/v',
      'ProxyOverride',
      '/t',
      'REG_SZ',
      '/d',
      '<local>;127.0.0.1;localhost',
      '/f',
    ]);
    await _run('reg.exe', [
      'ADD',
      r'HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings',
      '/v',
      'ProxyEnable',
      '/t',
      'REG_DWORD',
      '/d',
      '1',
      '/f',
    ]);
    await _refreshWindows();
  }

  Future<void> _disableWindows() async {
    await _run('reg.exe', [
      'ADD',
      r'HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings',
      '/v',
      'ProxyEnable',
      '/t',
      'REG_DWORD',
      '/d',
      '0',
      '/f',
    ]);
    await _refreshWindows();
  }

  Future<void> _refreshWindows() =>
      _run('RUNDLL32.EXE', ['user32.dll,UpdatePerUserSystemParameters']);

  Future<void> _run(String executable, List<String> arguments) async {
    final result = await Process.run(executable, arguments);
    if (result.exitCode != 0) {
      throw ProcessException(
        executable,
        arguments,
        result.stderr.toString(),
        result.exitCode,
      );
    }
  }
}
