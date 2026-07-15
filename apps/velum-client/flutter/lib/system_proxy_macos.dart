import 'system_proxy_contract.dart';

final class MacosProxyBackend extends CommandProxyBackend {
  MacosProxyBackend({super.run});

  @override
  String get id => 'macos';

  @override
  Future<ProxySnapshot> capture() async {
    final services = <String, Object?>{};
    for (final service in await _services()) {
      services[service] = <String, Object?>{
        'http': await _proxyState(service, 'webproxy'),
        'https': await _proxyState(service, 'securewebproxy'),
        'socks': await _proxyState(service, 'socksfirewallproxy'),
        'pac': await _proxyState(service, 'autoproxyurl'),
        'bypass': await _bypassDomains(service),
      };
    }
    return ProxySnapshot(backend: id, values: {'services': services});
  }

  @override
  Future<void> enable(int port, {required List<String> bypassHosts}) async {
    for (final service in await _services()) {
      await checked('/usr/sbin/networksetup', [
        '-setautoproxystate',
        service,
        'off',
      ]);
      for (final kind in ['webproxy', 'securewebproxy', 'socksfirewallproxy']) {
        await checked('/usr/sbin/networksetup', [
          '-set$kind',
          service,
          '127.0.0.1',
          '$port',
        ]);
        await checked('/usr/sbin/networksetup', [
          '-set${kind}state',
          service,
          'on',
        ]);
      }
      await checked('/usr/sbin/networksetup', [
        '-setproxybypassdomains',
        service,
        if (bypassHosts.isEmpty) 'Empty' else ...bypassHosts,
      ]);
    }
  }

  @override
  Future<void> restore(ProxySnapshot snapshot) async {
    final services = snapshot.values['services'] as Map<String, Object?>;
    for (final serviceEntry in services.entries) {
      final service = serviceEntry.key;
      final values = serviceEntry.value as Map<String, Object?>;
      for (final (key, kind) in [
        ('http', 'webproxy'),
        ('https', 'securewebproxy'),
        ('socks', 'socksfirewallproxy'),
      ]) {
        final state = values[key] as Map<String, Object?>;
        final server = state['server'] as String?;
        final port = state['port'] as int?;
        if (server != null && port != null) {
          await checked('/usr/sbin/networksetup', [
            '-set$kind',
            service,
            server,
            '$port',
          ]);
        }
        await checked('/usr/sbin/networksetup', [
          '-set${kind}state',
          service,
          state['enabled'] == true ? 'on' : 'off',
        ]);
      }
      final pac = values['pac'] as Map<String, Object?>;
      final pacUrl = pac['server'] as String?;
      if (pacUrl != null && pacUrl.isNotEmpty) {
        await checked('/usr/sbin/networksetup', [
          '-setautoproxyurl',
          service,
          pacUrl,
        ]);
      }
      await checked('/usr/sbin/networksetup', [
        '-setautoproxystate',
        service,
        pac['enabled'] == true ? 'on' : 'off',
      ]);
      final bypass = (values['bypass'] as List<Object?>).cast<String>();
      await checked('/usr/sbin/networksetup', [
        '-setproxybypassdomains',
        service,
        if (bypass.isEmpty) 'Empty' else ...bypass,
      ]);
    }
  }

  Future<List<String>> _services() async {
    final output = await checked('/usr/sbin/networksetup', [
      '-listallnetworkservices',
    ]);
    return output.stdout
        .toString()
        .split('\n')
        .map((line) => line.trim())
        .where((line) => line.isNotEmpty)
        .where((line) => !line.startsWith('*'))
        .where((line) => !line.startsWith('An asterisk'))
        .toList();
  }

  Future<Map<String, Object?>> _proxyState(String service, String kind) async {
    final output = await checked('/usr/sbin/networksetup', [
      '-get$kind',
      service,
    ]);
    final fields = <String, String>{};
    for (final line in output.stdout.toString().split('\n')) {
      final separator = line.indexOf(':');
      if (separator > 0) {
        fields[line.substring(0, separator).trim().toLowerCase()] = line
            .substring(separator + 1)
            .trim();
      }
    }
    return {
      'enabled': fields['enabled']?.toLowerCase() == 'yes',
      'server': switch (fields['server'] ?? fields['url']) {
        null || '' => null,
        final v => v,
      },
      'port': int.tryParse(fields['port'] ?? ''),
    };
  }

  Future<List<String>> _bypassDomains(String service) async {
    final output = await checked('/usr/sbin/networksetup', [
      '-getproxybypassdomains',
      service,
    ]);
    final lines = output.stdout
        .toString()
        .split('\n')
        .map((line) => line.trim())
        .where((line) => line.isNotEmpty)
        .toList();
    if (lines.length == 1 && lines.single.startsWith("There aren't any")) {
      return const [];
    }
    return lines;
  }
}
