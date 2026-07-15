import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/native_client.dart';

void main() {
  final library = Platform.environment['VELUM_CLIENT_LIBRARY'];
  test(
    'Dart profile ABI normalizes aliases to stable node ids',
    () {
      final codec = NativeProfileCodec.open(library!);
      final normalized = codec.normalize(_profile);

      expect(normalized, contains('default-node: node-sg'));
      expect(normalized, contains('target: node-sg'));
    },
    skip: library == null
        ? 'Set VELUM_CLIENT_LIBRARY to a built velum-client-ffi library.'
        : false,
  );
}

const _profile = '''
version: 1
profile:
  id: personal
  name: Personal
  default-node: singapore
nodes:
  - id: node-sg
    alias: singapore
    relay-address: 203.0.113.10:4433
    server-name: relay.example
    credential-ref: secret://velum/personal/node-sg
    trust: {mode: system}
traffic:
  preferred-adapter: tun
  system-proxy: {port: 0, bypass: [localhost]}
  tun:
    mtu: 1500
    ipv4: 172.19.0.1/30
    ipv6: fd00:19::1/126
    dns: [1.1.1.1]
routing:
  mode: rule
  rules:
    - match: {type: match}
      action: {type: node, target: singapore}
''';
