import 'dart:ffi';
import 'dart:io';

import 'system_proxy_contract.dart';

final class WindowsProxyBackend extends CommandProxyBackend {
  WindowsProxyBackend({super.run, void Function()? refresh})
    : _refresh = refresh ?? _refreshWinInet;

  static const _key =
      r'HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings';
  final void Function() _refresh;

  @override
  String get id => 'windows';

  @override
  Future<ProxySnapshot> capture() async {
    final values = <String, Object?>{};
    for (final name in ['ProxyEnable', 'ProxyServer', 'ProxyOverride']) {
      final result = await run('reg.exe', ['QUERY', _key, '/v', name]);
      values[name] = result.exitCode == 0
          ? _parseRegistryValue(result.stdout.toString(), name)
          : null;
    }
    return ProxySnapshot(backend: id, values: values);
  }

  @override
  Future<void> enable(int port) async {
    await _set(
      'ProxyServer',
      'REG_SZ',
      'https=127.0.0.1:$port;socks=127.0.0.1:$port',
    );
    await _set('ProxyOverride', 'REG_SZ', '<local>;127.0.0.1;localhost');
    await _set('ProxyEnable', 'REG_DWORD', '1');
    _refresh();
  }

  @override
  Future<void> restore(ProxySnapshot snapshot) async {
    for (final name in ['ProxyServer', 'ProxyOverride', 'ProxyEnable']) {
      final value = snapshot.values[name] as Map<String, Object?>?;
      if (value == null) {
        final result = await run('reg.exe', ['DELETE', _key, '/v', name, '/f']);
        if (result.exitCode != 0 &&
            !result.stderr.toString().contains('unable to find')) {
          throw ProcessException(
            'reg.exe',
            ['DELETE', _key, '/v', name, '/f'],
            result.stderr.toString(),
            result.exitCode,
          );
        }
      } else {
        await _set(name, value['type'] as String, value['data'] as String);
      }
    }
    _refresh();
  }

  Future<void> _set(String name, String type, String data) => checked(
    'reg.exe',
    ['ADD', _key, '/v', name, '/t', type, '/d', data, '/f'],
  ).then((_) {});

  static Map<String, Object?> _parseRegistryValue(String output, String name) {
    for (final line in output.split('\n')) {
      final match = RegExp(
        '^\\s*${RegExp.escape(name)}\\s+(REG_\\w+)\\s+(.*)\\s*\$',
        caseSensitive: false,
      ).firstMatch(line);
      if (match != null) {
        return {'type': match.group(1)!, 'data': match.group(2)!.trim()};
      }
    }
    throw const FormatException('Registry query did not contain its value.');
  }

  static void _refreshWinInet() {
    final library = DynamicLibrary.open('wininet.dll');
    final internetSetOption = library
        .lookupFunction<
          Int32 Function(IntPtr, Uint32, Pointer<Void>, Uint32),
          int Function(int, int, Pointer<Void>, int)
        >('InternetSetOptionW');
    final changed = internetSetOption(0, 39, nullptr, 0);
    final refreshed = internetSetOption(0, 37, nullptr, 0);
    if (changed == 0 || refreshed == 0) {
      throw StateError(
        'Windows did not accept the proxy refresh notification.',
      );
    }
  }
}
