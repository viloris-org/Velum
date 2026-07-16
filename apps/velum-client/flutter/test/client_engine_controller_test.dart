import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/client_engine_activation.dart';
import 'package:velum_client/client_engine_controller.dart';
import 'package:velum_client/native_client.dart';
import 'package:velum_client/native_engine.dart';
import 'package:velum_client/traffic_runtime.dart';

void main() {
  test('activates, polls the default node, and controls the engine proxy', () {
    final engine = _FakeEngine();
    final controller = ClientEngineController(engineFactory: (_) => engine);
    final activation = ClientEngineActivation(
      libraryPath: 'fake',
      defaultNode: 'one',
      nodes: [
        ClientEngineNodeConfiguration(
          id: 'one',
          alias: 'primary',
          relayAddress: '192.0.2.1:443',
          serverName: 'relay.example',
          credential: Uint8List.fromList(List.filled(32, 7)),
          trustMode: ClientTrustMode.system,
          certificatePem: Uint8List(0),
        ),
      ],
    );

    expect(controller.activate(activation), 1);
    expect(activation.nodes.single.credential, everyElement(0));
    expect(controller.snapshot.phase, ClientRuntimePhase.connecting);

    engine.online();
    controller.refresh();
    expect(controller.online, isTrue);
    expect(controller.startLoopbackProxy(routingRules: 'MATCH,NODE:one'), 1080);
    expect(engine.events, ['activate:one', 'proxy:MATCH,NODE:one']);

    controller.stop();
    controller.dispose();
    expect(engine.events, contains('stop'));
    expect(engine.events, contains('destroy'));
  });

  test('does not expose a single runtime handle for TUN', () {
    final controller = ClientEngineController(
      engineFactory: (_) => _FakeEngine(),
    );
    final runtime = controller as TrafficRuntime;

    expect(runtime.runtimeHandleForTun, throwsUnsupportedError);
    controller.dispose();
  });
}

final class _FakeEngine implements ClientEngineBridge {
  final events = <String>[];
  ClientRuntimePhase phase = ClientRuntimePhase.connecting;
  int revision = 1;

  @override
  int activate(
    List<ClientEngineNodeConfiguration> nodes, {
    required String defaultNode,
  }) {
    events.add('activate:$defaultNode');
    return 1;
  }

  void online() {
    phase = ClientRuntimePhase.online;
    revision += 1;
  }

  @override
  ClientEngineNodeSnapshot nodeSnapshot(String reference) =>
      ClientEngineNodeSnapshot(
        profileGeneration: 1,
        isDefault: true,
        runtime: ClientRuntimeSnapshot(
          revision: revision,
          generation: 1,
          phase: phase,
          failure: ClientRuntimeFailure.none,
        ),
      );

  @override
  int startLoopbackProxy({
    int requestedPort = 0,
    String routingRules = 'MATCH,PROXY',
  }) {
    events.add('proxy:$routingRules');
    return 1080;
  }

  @override
  void stopLoopbackProxy() => events.add('proxy-stop');

  @override
  int stop() {
    events.add('stop');
    phase = ClientRuntimePhase.stopped;
    return 2;
  }

  @override
  void destroy() => events.add('destroy');
}
