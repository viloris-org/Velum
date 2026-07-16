import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/native_client.dart';
import 'package:velum_client/native_engine.dart';

void main() {
  final library = Platform.environment['VELUM_CLIENT_LIBRARY'];

  test(
    'Dart engine ABI activates a resolved default node and reads its snapshot',
    () {
      final engine = NativeClientEngine.open(library!);
      final credential = Uint8List.fromList(List.filled(32, 7));
      try {
        final generation = engine.activate([
          ClientEngineNodeConfiguration(
            id: 'node-one',
            alias: 'primary',
            relayAddress: '192.0.2.1:443',
            serverName: 'relay.example',
            credential: credential,
            trustMode: ClientTrustMode.system,
            certificatePem: Uint8List(0),
          ),
        ], defaultNode: 'primary');

        credential.fillRange(0, credential.length, 0);
        final snapshot = engine.nodeSnapshot('node-one');
        expect(generation, greaterThan(0));
        expect(snapshot.profileGeneration, generation);
        expect(snapshot.isDefault, isTrue);
      } finally {
        engine.destroy();
      }
    },
    skip: library == null
        ? 'Set VELUM_CLIENT_LIBRARY to a built velum-client-ffi library.'
        : false,
  );

  test(
    'Dart engine proxy ABI rejects a pool without an online default node',
    () {
      final engine = NativeClientEngine.open(library!);
      try {
        expect(
          engine.startLoopbackProxy,
          throwsA(isA<ClientControlException>()),
        );
      } finally {
        engine.destroy();
      }
    },
    skip: library == null
        ? 'Set VELUM_CLIENT_LIBRARY to a built velum-client-ffi library.'
        : false,
  );
}
