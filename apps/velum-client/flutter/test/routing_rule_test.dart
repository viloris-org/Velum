import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/routing_rule.dart';

void main() {
  group('RoutingRule parsing', () {
    test('parses and canonically serializes supported rules', () {
      expect(
        RoutingRule.parse(
          ' DOMAIN, Example.COM., direct '.toUpperCase(),
        ).serialize(),
        'DOMAIN,example.com,DIRECT',
      );
      expect(
        RoutingRule.parse('DOMAIN-SUFFIX,example.com,PROXY').serialize(),
        'DOMAIN-SUFFIX,example.com,PROXY',
      );
      expect(
        RoutingRule.parse('IP-CIDR,192.168.0.0/16,REJECT').serialize(),
        'IP-CIDR,192.168.0.0/16,REJECT',
      );
      expect(RoutingRule.parse('MATCH,DIRECT').serialize(), 'MATCH,DIRECT');
    });

    test('rejects unknown fields and malformed values', () {
      for (final source in [
        'GEOIP,CN,DIRECT',
        'DOMAIN,example.com,BLOCK',
        'DOMAIN,,DIRECT',
        'DOMAIN,not_a_domain,DIRECT',
        'MATCH,value,DIRECT',
        'IP-CIDR,192.168.1.1/24,DIRECT',
        'IP-CIDR,192.168.0.0/33,DIRECT',
        'IP-CIDR,01.2.3.4/32,DIRECT',
      ]) {
        expect(() => RoutingRule.parse(source), throwsFormatException);
      }
    });
  });

  group('RoutingRule matching', () {
    test('DOMAIN is case-insensitive and exact', () {
      final rule = RoutingRule.parse('DOMAIN,api.example.com,PROXY');

      expect(
        rule.matches(const RoutingRequest(domain: 'API.EXAMPLE.COM.')),
        isTrue,
      );
      expect(
        rule.matches(const RoutingRequest(domain: 'www.api.example.com')),
        isFalse,
      );
    });

    test('DOMAIN-SUFFIX matches the apex and label-bound subdomains', () {
      final rule = RoutingRule.parse('DOMAIN-SUFFIX,example.com,PROXY');

      expect(rule.matches(const RoutingRequest(domain: 'example.com')), isTrue);
      expect(
        rule.matches(const RoutingRequest(domain: 'www.example.com')),
        isTrue,
      );
      expect(
        rule.matches(const RoutingRequest(domain: 'notexample.com')),
        isFalse,
      );
    });

    test('IP-CIDR matches IPv4 addresses inside the network', () {
      final rule = RoutingRule.parse('IP-CIDR,10.20.0.0/16,DIRECT');

      expect(
        rule.matches(const RoutingRequest(ipAddress: '10.20.255.254')),
        isTrue,
      );
      expect(
        rule.matches(const RoutingRequest(ipAddress: '10.21.0.1')),
        isFalse,
      );
      expect(
        rule.matches(const RoutingRequest(ipAddress: 'not-an-ip')),
        isFalse,
      );
    });

    test('MATCH accepts every request', () {
      final rule = RoutingRule.parse('MATCH,REJECT');
      expect(rule.matches(const RoutingRequest()), isTrue);
    });
  });

  group('RoutingRuleSet', () {
    test('uses the first matching rule in source order', () {
      final rules = RoutingRuleSet.parse('''
DOMAIN,blocked.example,REJECT
DOMAIN-SUFFIX,example,PROXY
MATCH,DIRECT
''');

      expect(
        rules.match(const RoutingRequest(domain: 'blocked.example')),
        RoutingAction.reject,
      );
      expect(
        rules.match(const RoutingRequest(domain: 'other.example')),
        RoutingAction.proxy,
      );
      expect(
        rules.match(const RoutingRequest(domain: 'unrelated.test')),
        RoutingAction.direct,
      );
    });

    test('round trips canonical text and reports the invalid line', () {
      final rules = RoutingRuleSet.parse('''

 DOMAIN,EXAMPLE.COM,PROXY
 MATCH,DIRECT
''');
      expect(rules.serialize(), 'DOMAIN,example.com,PROXY\nMATCH,DIRECT');
      expect(
        () => RoutingRuleSet.parse('MATCH,DIRECT\nDOMAIN,bad_name,PROXY'),
        throwsA(
          isA<FormatException>().having(
            (error) => error.message,
            'message',
            contains('line 2'),
          ),
        ),
      );
      expect(
        () => RoutingRuleSet.parse('MATCH,DIRECT\nDOMAIN,example.com,PROXY'),
        throwsFormatException,
      );
    });
  });
}
