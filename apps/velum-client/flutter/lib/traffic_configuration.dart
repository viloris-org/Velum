import 'dart:io';

import 'package:flutter/material.dart';

import 'android_vpn.dart';
import 'routing_rule.dart';
import 'system_proxy_contract.dart';

enum RoutingMode { rule, global, direct }

final class TrafficConfigurationDraft {
  TrafficConfigurationDraft()
    : proxyPort = TextEditingController(text: '0'),
      proxyBypass = TextEditingController(text: 'localhost\n127.0.0.1\n::1'),
      tunAddress = TextEditingController(text: '172.19.0.1'),
      tunPrefixLength = TextEditingController(text: '30'),
      tunMtu = TextEditingController(text: '1500'),
      tunDnsServers = TextEditingController(text: '8.8.8.8'),
      tunRoutes = TextEditingController(text: '0.0.0.0/0'),
      rules = TextEditingController(
        text:
            'DOMAIN-SUFFIX,local,DIRECT\n'
            'IP-CIDR,10.0.0.0/8,DIRECT\n'
            'IP-CIDR,172.16.0.0/12,DIRECT\n'
            'IP-CIDR,192.168.0.0/16,DIRECT\n'
            'MATCH,PROXY',
      );

  final TextEditingController proxyPort;
  final TextEditingController proxyBypass;
  final TextEditingController tunAddress;
  final TextEditingController tunPrefixLength;
  final TextEditingController tunMtu;
  final TextEditingController tunDnsServers;
  final TextEditingController tunRoutes;
  final TextEditingController rules;
  RoutingMode routingMode = RoutingMode.rule;

  SystemProxyOptions systemProxyOptions() => SystemProxyOptions(
    requestedPort: _integer(proxyPort.text, 'Proxy port', min: 0, max: 65535),
    bypassHosts: _lines(proxyBypass.text),
  );

  TunOptions tunOptions() => TunOptions(
    address: _ipv4(tunAddress.text, 'TUN address'),
    prefixLength: _integer(
      tunPrefixLength.text,
      'TUN prefix length',
      min: 0,
      max: 32,
    ),
    mtu: _integer(tunMtu.text, 'TUN MTU', min: 576, max: 65535),
    dnsServers: _lines(
      tunDnsServers.text,
    ).map((value) => _ipv4(value, 'TUN DNS server')),
    routes: _lines(tunRoutes.text).map(_parseRoute),
  );

  RoutingRuleSet routingRules() => switch (routingMode) {
    RoutingMode.rule => RoutingRuleSet.parse(rules.text),
    RoutingMode.global => RoutingRuleSet.parse('MATCH,PROXY'),
    RoutingMode.direct => RoutingRuleSet.parse('MATCH,DIRECT'),
  };

  String? validate() {
    return validateSystemProxy() ?? validateTun();
  }

  String? validateSystemProxy() => _validate(() {
    systemProxyOptions();
    final parsedRules = routingRules();
    if (routingMode == RoutingMode.rule && parsedRules.rules.isEmpty) {
      throw const FormatException(
        'Rule mode requires at least one routing rule.',
      );
    }
  });

  String? validateTun() => _validate(tunOptions);

  String? _validate(void Function() validation) {
    try {
      validation();
      return null;
    } on FormatException catch (error) {
      return error.message.toString();
    } on ArgumentError catch (error) {
      return error.message?.toString() ?? 'Invalid traffic configuration.';
    }
  }

  void dispose() {
    proxyPort.dispose();
    proxyBypass.dispose();
    tunAddress.dispose();
    tunPrefixLength.dispose();
    tunMtu.dispose();
    tunDnsServers.dispose();
    tunRoutes.dispose();
    rules.dispose();
  }
}

int _integer(
  String source,
  String label, {
  required int min,
  required int max,
}) {
  final value = int.tryParse(source.trim());
  if (value == null || value < min || value > max) {
    throw FormatException('$label must be between $min and $max.');
  }
  return value;
}

List<String> _lines(String source) => source
    .split(RegExp(r'[,\n]'))
    .map((value) => value.trim())
    .where((value) => value.isNotEmpty)
    .toList(growable: false);

TunRoute _parseRoute(String value) {
  final separator = value.lastIndexOf('/');
  if (separator <= 0) {
    throw FormatException('Invalid TUN route: $value');
  }
  final rule = RoutingRule.parse('IP-CIDR,$value,PROXY');
  return TunRoute(
    rule.value!.substring(0, rule.value!.lastIndexOf('/')),
    _integer(
      value.substring(separator + 1),
      'TUN route prefix',
      min: 0,
      max: 32,
    ),
  );
}

String _ipv4(String source, String label) {
  final value = source.trim();
  final address = InternetAddress.tryParse(value);
  if (address == null || address.type != InternetAddressType.IPv4) {
    throw FormatException('$label must be an IPv4 address.');
  }
  return value;
}
