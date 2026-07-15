enum RoutingRuleType { domain, domainSuffix, ipCidr, match }

enum RoutingAction { direct, proxy, reject }

final class RoutingRequest {
  const RoutingRequest({this.domain, this.ipAddress});

  final String? domain;
  final String? ipAddress;
}

final class RoutingRule {
  const RoutingRule._({
    required this.type,
    required this.action,
    this.value,
    _Ipv4Network? network,
  }) : _network = network;

  factory RoutingRule.parse(String source) {
    final fields = source.split(',').map((field) => field.trim()).toList();
    if (fields.any((field) => field.isEmpty)) {
      throw FormatException('Routing rule fields cannot be empty.', source);
    }

    final type = _parseType(fields.first, source);
    final expectedFields = type == RoutingRuleType.match ? 2 : 3;
    if (fields.length != expectedFields) {
      throw FormatException(
        '${fields.first} requires $expectedFields fields.',
        source,
      );
    }

    final action = _parseAction(fields.last, source);
    if (type == RoutingRuleType.match) {
      return RoutingRule._(type: type, action: action);
    }

    final value = fields[1];
    switch (type) {
      case RoutingRuleType.domain:
      case RoutingRuleType.domainSuffix:
        final domain = _normalizeDomain(value, source);
        return RoutingRule._(type: type, value: domain, action: action);
      case RoutingRuleType.ipCidr:
        final network = _Ipv4Network.parse(value, source);
        return RoutingRule._(
          type: type,
          value: network.serialize(),
          action: action,
          network: network,
        );
      case RoutingRuleType.match:
        throw StateError('MATCH was handled before value parsing.');
    }
  }

  final RoutingRuleType type;
  final String? value;
  final RoutingAction action;
  final _Ipv4Network? _network;

  bool matches(RoutingRequest request) {
    switch (type) {
      case RoutingRuleType.domain:
        final domain = _requestDomain(request.domain);
        return domain != null && domain == value;
      case RoutingRuleType.domainSuffix:
        final domain = _requestDomain(request.domain);
        return domain != null &&
            (domain == value || domain.endsWith('.$value'));
      case RoutingRuleType.ipCidr:
        final address = _tryParseIpv4(request.ipAddress);
        return address != null && _network!.contains(address);
      case RoutingRuleType.match:
        return true;
    }
  }

  String serialize() {
    final typeName = switch (type) {
      RoutingRuleType.domain => 'DOMAIN',
      RoutingRuleType.domainSuffix => 'DOMAIN-SUFFIX',
      RoutingRuleType.ipCidr => 'IP-CIDR',
      RoutingRuleType.match => 'MATCH',
    };
    final actionName = switch (action) {
      RoutingAction.direct => 'DIRECT',
      RoutingAction.proxy => 'PROXY',
      RoutingAction.reject => 'REJECT',
    };
    return value == null
        ? '$typeName,$actionName'
        : '$typeName,$value,$actionName';
  }

  @override
  String toString() => serialize();

  static RoutingRuleType _parseType(String field, String source) =>
      switch (field) {
        'DOMAIN' => RoutingRuleType.domain,
        'DOMAIN-SUFFIX' => RoutingRuleType.domainSuffix,
        'IP-CIDR' => RoutingRuleType.ipCidr,
        'MATCH' => RoutingRuleType.match,
        _ => throw FormatException('Unknown routing rule type: $field', source),
      };

  static RoutingAction _parseAction(String field, String source) =>
      switch (field) {
        'DIRECT' => RoutingAction.direct,
        'PROXY' => RoutingAction.proxy,
        'REJECT' => RoutingAction.reject,
        _ => throw FormatException('Unknown routing action: $field', source),
      };
}

final class RoutingRuleSet {
  RoutingRuleSet(Iterable<RoutingRule> rules)
    : rules = List.unmodifiable(rules) {
    final fallback = this.rules.indexWhere(
      (rule) => rule.type == RoutingRuleType.match,
    );
    if (fallback >= 0 && fallback != this.rules.length - 1) {
      throw const FormatException('MATCH must be the final routing rule.');
    }
  }

  factory RoutingRuleSet.parse(String source) {
    final rules = <RoutingRule>[];
    for (final entry in source.split('\n').indexed) {
      final lineNumber = entry.$1 + 1;
      final line = entry.$2.trim();
      if (line.isEmpty) continue;
      try {
        rules.add(RoutingRule.parse(line));
      } on FormatException catch (error) {
        throw FormatException(
          'Invalid routing rule on line $lineNumber: ${error.message}',
          error.source,
          error.offset,
        );
      }
    }
    return RoutingRuleSet(rules);
  }

  final List<RoutingRule> rules;

  RoutingAction? match(RoutingRequest request) {
    for (final rule in rules) {
      if (rule.matches(request)) return rule.action;
    }
    return null;
  }

  String serialize() => rules.map((rule) => rule.serialize()).join('\n');
}

String _normalizeDomain(String value, String source) {
  var domain = value.toLowerCase();
  if (domain.endsWith('.')) domain = domain.substring(0, domain.length - 1);
  if (domain.isEmpty || domain.length > 253 || domain.contains('..')) {
    throw FormatException('Invalid domain: $value', source);
  }
  final labels = domain.split('.');
  for (final label in labels) {
    if (label.isEmpty ||
        label.length > 63 ||
        label.startsWith('-') ||
        label.endsWith('-') ||
        !RegExp(r'^[a-z0-9-]+$').hasMatch(label)) {
      throw FormatException('Invalid domain: $value', source);
    }
  }
  return domain;
}

String? _requestDomain(String? value) {
  if (value == null) return null;
  try {
    return _normalizeDomain(value.trim(), value);
  } on FormatException {
    return null;
  }
}

int? _tryParseIpv4(String? value) {
  if (value == null) return null;
  try {
    return _parseIpv4(value.trim(), value);
  } on FormatException {
    return null;
  }
}

int _parseIpv4(String value, String source) {
  final octets = value.split('.');
  if (octets.length != 4) {
    throw FormatException('Invalid IPv4 address: $value', source);
  }
  var address = 0;
  for (final octet in octets) {
    if (!RegExp(r'^(0|[1-9][0-9]{0,2})$').hasMatch(octet)) {
      throw FormatException('Invalid IPv4 address: $value', source);
    }
    final number = int.parse(octet);
    if (number > 255) {
      throw FormatException('Invalid IPv4 address: $value', source);
    }
    address = (address << 8) | number;
  }
  return address;
}

final class _Ipv4Network {
  const _Ipv4Network(this.address, this.prefixLength, this.mask);

  factory _Ipv4Network.parse(String value, String source) {
    final fields = value.split('/');
    if (fields.length != 2 ||
        !RegExp(r'^(0|[1-9][0-9]?)$').hasMatch(fields[1])) {
      throw FormatException('Invalid IPv4 CIDR: $value', source);
    }
    final prefixLength = int.parse(fields[1]);
    if (prefixLength > 32) {
      throw FormatException('Invalid IPv4 CIDR: $value', source);
    }
    final address = _parseIpv4(fields[0], source);
    final mask = prefixLength == 0
        ? 0
        : (0xffffffff << (32 - prefixLength)) & 0xffffffff;
    if ((address & mask) != address) {
      throw FormatException('IPv4 CIDR has host bits set: $value', source);
    }
    return _Ipv4Network(address, prefixLength, mask);
  }

  final int address;
  final int prefixLength;
  final int mask;

  bool contains(int candidate) => (candidate & mask) == address;

  String serialize() => '${_serializeIpv4(address)}/$prefixLength';
}

String _serializeIpv4(int address) => [
  (address >> 24) & 0xff,
  (address >> 16) & 0xff,
  (address >> 8) & 0xff,
  address & 0xff,
].join('.');
