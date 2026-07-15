import 'dart:typed_data';

import 'package:velum_client/native_client.dart';

class FakeClientRuntime implements ClientRuntimeBridge {
  ClientRuntimeSnapshot current = const ClientRuntimeSnapshot.stopped();
  int startCount = 0;
  int stopCount = 0;
  int destroyCount = 0;
  bool throwOnStop = false;
  bool throwOnDestroy = false;
  bool throwOnSnapshot = false;

  @override
  int start(ClientRuntimeConfiguration configuration) {
    startCount += 1;
    final generation = current.generation + 1;
    current = ClientRuntimeSnapshot(
      revision: current.revision + 1,
      generation: generation,
      phase: ClientRuntimePhase.connecting,
      failure: ClientRuntimeFailure.none,
    );
    return generation;
  }

  @override
  ClientRuntimeSnapshot snapshot() {
    if (throwOnSnapshot) {
      throw const ClientControlException(ClientControlStatus.internal);
    }
    return current;
  }

  @override
  int stop() {
    stopCount += 1;
    if (throwOnStop) {
      throw const ClientControlException(ClientControlStatus.internal);
    }
    final generation = current.generation + 1;
    current = ClientRuntimeSnapshot(
      revision: current.revision + 2,
      generation: generation,
      phase: ClientRuntimePhase.stopped,
      failure: ClientRuntimeFailure.none,
    );
    return generation;
  }

  @override
  void destroy() {
    destroyCount += 1;
    if (throwOnDestroy) {
      throw const ClientControlException(ClientControlStatus.internal);
    }
  }
}

ClientRuntimeConfiguration testRuntimeConfiguration() =>
    ClientRuntimeConfiguration(
      libraryPath: 'fake-runtime',
      relayAddress: '127.0.0.1:4433',
      serverName: 'localhost',
      credential: Uint8List.fromList([1, 2, 3]),
      trustMode: ClientTrustMode.system,
      certificatePem: Uint8List.fromList([4, 5, 6]),
    );
