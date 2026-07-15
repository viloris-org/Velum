import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/routing_rule.dart';
import 'package:velum_client/traffic_configuration.dart';

void main() {
  test('builds validated proxy and TUN options from the draft', () {
    final draft = TrafficConfigurationDraft();
    addTearDown(draft.dispose);

    expect(draft.systemProxyOptions().requestedPort, 0);
    expect(draft.systemProxyOptions().bypassHosts, contains('localhost'));
    expect(draft.tunOptions().address, '172.19.0.1');
    expect(draft.tunOptions().routes.single.prefixLength, 0);
    expect(draft.validate(), isNull);
  });

  test('routing modes produce explicit fallback policies', () {
    final draft = TrafficConfigurationDraft();
    addTearDown(draft.dispose);

    draft.routingMode = RoutingMode.global;
    expect(
      draft.routingRules().match(const RoutingRequest()),
      RoutingAction.proxy,
    );
    draft.routingMode = RoutingMode.direct;
    expect(
      draft.routingRules().match(const RoutingRequest()),
      RoutingAction.direct,
    );
  });

  test('reports invalid adapter values before activation', () {
    final draft = TrafficConfigurationDraft();
    addTearDown(draft.dispose);

    draft.tunRoutes.text = '10.0.0.1/8';
    expect(draft.validate(), contains('host bits'));
    draft.tunRoutes.text = '10.0.0.0/8';
    draft.tunDnsServers.text = 'not-an-address';
    expect(draft.validate(), contains('IPv4 address'));
  });

  test('validates desktop proxy and TUN settings independently', () {
    final draft = TrafficConfigurationDraft();
    addTearDown(draft.dispose);

    draft.rules.text = 'INVALID';
    expect(draft.validateSystemProxy(), isNotNull);
    expect(draft.validateTun(), isNull);
  });
}
