import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/client_configuration.dart';
import 'package:velum_client/client_engine_activation.dart';
import 'package:velum_client/client_profile.dart';
import 'package:velum_client/native_client.dart';

void main() {
  test(
    'resolves file-backed node material and clears it after activation',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'velum-engine-test',
      );
      final credential = File('${directory.path}/credential.hex');
      final certificate = File('${directory.path}/ca.pem');
      await credential.writeAsString('07' * 32);
      await certificate.writeAsString(
        '-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----\n',
      );
      final node = RelayNodeDraft(
        id: 'node-one',
        name: 'primary',
        relayAddress: '192.0.2.1:443',
        serverName: 'relay.example',
        credentialPath: credential.path,
        certificatePath: certificate.path,
        trustMode: ClientTrustMode.customCa,
      );
      final resolver = ClientEngineActivationResolver(ClientSecretStore());
      try {
        final activation = await resolver.resolve(
          libraryPath: 'test-library',
          nodes: [node],
          defaultNode: 'node-one',
        );

        expect(activation.defaultNode, 'node-one');
        expect(activation.nodes, hasLength(1));
        expect(activation.nodes.single.credential, hasLength(32));
        expect(activation.nodes.single.certificatePem, isNotEmpty);

        activation.clearSecrets();
        expect(activation.nodes.single.credential, everyElement(0));
        expect(activation.nodes.single.certificatePem, everyElement(0));
      } finally {
        node.dispose();
        await directory.delete(recursive: true);
      }
    },
  );
}
