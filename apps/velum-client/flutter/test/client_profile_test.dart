import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/client_profile.dart';

const _profile = '''
version: 1
profile:
  id: personal
  name: Personal
  default-node: node-sg
nodes:
  - id: node-sg
    alias: singapore
    relay-address: 203.0.113.10:4433
    server-name: relay.example
    credential-ref: secret://velum/personal/node-sg
    trust:
      mode: custom-ca
      ca-ref: secret://velum/personal/node-sg/ca
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
      action: {type: proxy}
''';

void main() {
  test('canonical profile projection retains only redacted node metadata', () {
    final profile = ManagedClientProfile.parseCanonical(_profile);

    expect(profile.defaultNode, 'node-sg');
    expect(profile.nodes.single.alias, 'singapore');
    expect(
      profile.nodes.single.credentialRef,
      'secret://velum/personal/node-sg',
    );
    expect(profile.nodes.single.caRef, 'secret://velum/personal/node-sg/ca');
  });

  test('managed import validates before replacing the active copy', () async {
    final directory = await Directory.systemTemp.createTemp(
      'velum-profile-test',
    );
    addTearDown(() => directory.delete(recursive: true));
    final source = File('${directory.path}/source.yaml');
    final managed = File('${directory.path}/managed/profile.yaml');
    await source.writeAsString(_profile);
    final repository = ManagedProfileRepository(managed);

    final imported = await repository.importFile(source.path, (value) => value);
    expect(imported.id, 'personal');
    expect(await managed.readAsString(), _profile);

    await source.writeAsString('invalid');
    expect(
      () => repository.importFile(
        source.path,
        (_) => throw const FormatException(),
      ),
      throwsFormatException,
    );
    expect(await managed.readAsString(), _profile);
  });
}
