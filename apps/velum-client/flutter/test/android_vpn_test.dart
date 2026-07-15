import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/android_vpn.dart';

void main() {
  test('TUN defaults serialize to the Android channel contract', () {
    final options = TunOptions();

    expect(options.toMethodArguments(), {
      'address': '172.19.0.1',
      'prefixLength': 30,
      'ipv6Address': 'fd00:19::1',
      'ipv6PrefixLength': 126,
      'mtu': 1500,
      'dnsServers': ['8.8.8.8', '2001:4860:4860::8888'],
      'routes': [
        {'address': '0.0.0.0', 'prefixLength': 0},
        {'address': '::', 'prefixLength': 0},
      ],
    });
  });

  test('TUN option collections are immutable', () {
    final dns = ['1.1.1.1'];
    final routes = [const TunRoute('10.0.0.0', 8)];
    final options = TunOptions(dnsServers: dns, routes: routes);
    dns.add('8.8.8.8');
    routes.add(const TunRoute('192.168.0.0', 16));

    expect(options.dnsServers, ['1.1.1.1']);
    expect(options.routes, hasLength(1));
    expect(() => options.dnsServers.add('9.9.9.9'), throwsUnsupportedError);
  });

  test('TUN options reject values Android VpnService cannot accept', () {
    expect(() => TunOptions(prefixLength: 33), throwsArgumentError);
    expect(() => TunOptions(ipv6PrefixLength: 129), throwsArgumentError);
    expect(() => TunOptions(mtu: 500), throwsArgumentError);
    expect(
      () => TunOptions(routes: const [TunRoute('10.0.0.0', -1)]),
      throwsArgumentError,
    );
  });
}
