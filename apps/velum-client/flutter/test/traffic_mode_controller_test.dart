import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/android_vpn.dart';
import 'package:velum_client/desktop_tun.dart';
import 'package:velum_client/native_client.dart';
import 'package:velum_client/system_proxy.dart';
import 'package:velum_client/traffic_mode_controller.dart';
import 'package:velum_client/traffic_runtime.dart';

void main() {
  test(
    'selection stays armed offline and activates when runtime is online',
    () async {
      final runtime = _FakeRuntime();
      final adapter = _FakeAdapter(TrafficMode.systemProxy);
      final controller = TrafficModeController(
        runtime: runtime,
        adapters: [adapter],
      );

      await controller.select(TrafficMode.systemProxy);
      expect(controller.selectedMode, TrafficMode.systemProxy);
      expect(controller.phase, TrafficModePhase.inactive);
      expect(adapter.events, isEmpty);

      runtime.setPhase(ClientRuntimePhase.online);
      await _flushAsync();

      expect(adapter.events, ['activate']);
      expect(controller.activeMode, TrafficMode.systemProxy);
      expect(controller.phase, TrafficModePhase.active);
      controller.dispose();
    },
  );

  test(
    'runtime loss removes integration but preserves selected mode',
    () async {
      final runtime = _FakeRuntime()..setPhase(ClientRuntimePhase.online);
      final adapter = _FakeAdapter(TrafficMode.tun);
      final controller = TrafficModeController(
        runtime: runtime,
        adapters: [adapter],
      );

      await controller.select(TrafficMode.tun);
      runtime.setPhase(ClientRuntimePhase.failed);
      await _flushAsync();

      expect(adapter.events, ['activate', 'deactivate']);
      expect(controller.selectedMode, TrafficMode.tun);
      expect(controller.activeMode, TrafficMode.off);
      expect(controller.phase, TrafficModePhase.inactive);
      controller.dispose();
    },
  );

  test('suspend deactivates before runtime stop and keeps intent', () async {
    final runtime = _FakeRuntime()..setPhase(ClientRuntimePhase.online);
    final adapter = _FakeAdapter(TrafficMode.systemProxy);
    final controller = TrafficModeController(
      runtime: runtime,
      adapters: [adapter],
    );

    await controller.select(TrafficMode.systemProxy);
    await controller.suspend();

    expect(adapter.events, ['activate', 'deactivate']);
    expect(controller.selectedMode, TrafficMode.systemProxy);
    expect(controller.phase, TrafficModePhase.inactive);
    controller.dispose();
  });

  test('activation failure is exposed as authoritative failed state', () async {
    final runtime = _FakeRuntime()..setPhase(ClientRuntimePhase.online);
    final adapter = _FakeAdapter(TrafficMode.tun)..failActivation = true;
    final controller = TrafficModeController(
      runtime: runtime,
      adapters: [adapter],
    );

    await expectLater(controller.select(TrafficMode.tun), throwsStateError);

    expect(controller.activeMode, TrafficMode.off);
    expect(controller.phase, TrafficModePhase.failed);
    expect(controller.error, 'permission denied');
    controller.dispose();
  });

  test('unexpected adapter exit clears active state', () async {
    final runtime = _FakeRuntime()..setPhase(ClientRuntimePhase.online);
    final adapter = _FakeAdapter(TrafficMode.tun);
    final controller = TrafficModeController(
      runtime: runtime,
      adapters: [adapter],
    );

    await controller.select(TrafficMode.tun);
    adapter.exit();
    await _flushAsync();

    expect(controller.activeMode, TrafficMode.off);
    expect(controller.phase, TrafficModePhase.failed);
    expect(controller.error, 'The TUN VPN stopped unexpectedly.');
    controller.dispose();
  });

  test('selecting off retries failed startup recovery', () async {
    final runtime = _FakeRuntime();
    final adapter = _FakeAdapter(TrafficMode.systemProxy)..failRecovery = true;
    final controller = TrafficModeController(
      runtime: runtime,
      adapters: [adapter],
    );
    await _flushAsync();
    expect(controller.phase, TrafficModePhase.failed);

    adapter.failRecovery = false;
    await controller.select(TrafficMode.off);

    expect(adapter.recoveryCount, 2);
    expect(controller.phase, TrafficModePhase.inactive);
    expect(controller.error, isNull);
    controller.dispose();
  });

  test('system proxy adapter reads options when it activates', () async {
    final runtime = _FakeRuntime();
    final backend = _ProxyBackend();
    final adapter = DesktopSystemProxyAdapter(
      runtime,
      SystemProxy(backend: backend, store: _ProxyStore()),
      options: () => SystemProxyOptions(
        requestedPort: 9080,
        bypassHosts: const ['localhost', '.velum.test'],
      ),
    );

    await adapter.activate();

    expect(runtime.requestedPorts, [9080]);
    expect(backend.port, 1080);
    expect(backend.bypassHosts, ['localhost', '.velum.test']);
    await adapter.deactivate();
    expect(backend.restored, isTrue);
  });

  test('desktop TUN adapter passes generation and validated options', () async {
    final runtime = _FakeRuntime()..setPhase(ClientRuntimePhase.online);
    final host = _DesktopHost();
    final adapter = DesktopTunAdapter(
      runtime,
      host,
      options: () => TunOptions(mtu: 1280),
    );

    await adapter.recover();
    await adapter.activate();
    await adapter.deactivate();

    expect(host.events, ['recover', 'start:1:1:1280', 'stop']);
  });
}

Future<void> _flushAsync() async {
  await Future<void>.delayed(Duration.zero);
  await Future<void>.delayed(Duration.zero);
}

final class _FakeRuntime extends ChangeNotifier implements TrafficRuntime {
  ClientRuntimeSnapshot _snapshot = const ClientRuntimeSnapshot.stopped();
  final requestedPorts = <int>[];

  @override
  ClientRuntimeSnapshot get snapshot => _snapshot;

  void setPhase(ClientRuntimePhase phase) {
    _snapshot = ClientRuntimeSnapshot(
      revision: _snapshot.revision + 1,
      generation: 1,
      phase: phase,
      failure: ClientRuntimeFailure.none,
    );
    notifyListeners();
  }

  @override
  int runtimeHandleForTun() => 1;

  @override
  int startLoopbackProxy({
    int requestedPort = 0,
    String routingRules = 'MATCH,PROXY',
  }) {
    requestedPorts.add(requestedPort);
    return 1080;
  }

  @override
  void stopLoopbackProxy() {}
}

final class _ProxyBackend implements ProxyBackend {
  int? port;
  List<String>? bypassHosts;
  bool restored = false;

  @override
  String get id => 'test';

  @override
  Future<ProxySnapshot> capture() async =>
      const ProxySnapshot(backend: 'test', values: {});

  @override
  Future<void> enable(int port, {required List<String> bypassHosts}) async {
    this.port = port;
    this.bypassHosts = bypassHosts;
  }

  @override
  Future<void> restore(ProxySnapshot snapshot) async => restored = true;
}

final class _ProxyStore implements ProxyBackupStore {
  ProxySnapshot? snapshot;

  @override
  Future<void> clear() async => snapshot = null;

  @override
  Future<ProxySnapshot?> read() async => snapshot;

  @override
  Future<void> write(ProxySnapshot snapshot) async => this.snapshot = snapshot;
}

final class _FakeAdapter implements TrafficAdapter {
  _FakeAdapter(this.mode);

  @override
  final TrafficMode mode;
  final events = <String>[];
  bool failActivation = false;
  bool failRecovery = false;
  int recoveryCount = 0;
  Completer<void>? _completion;

  @override
  Future<void>? get completion => _completion?.future;

  @override
  Future<void> recover() async {
    recoveryCount += 1;
    if (failRecovery) throw StateError('restore failed');
  }

  @override
  Future<void> activate() async {
    events.add('activate');
    if (failActivation) throw StateError('permission denied');
    _completion = Completer<void>();
  }

  @override
  Future<void> deactivate() async {
    events.add('deactivate');
    if (_completion case final completion? when !completion.isCompleted) {
      completion.complete();
    }
  }

  void exit() => _completion?.complete();
}

final class _DesktopHost implements DesktopTunControl {
  final events = <String>[];

  @override
  Future<void> recover() async => events.add('recover');

  @override
  Future<void> start({
    required int runtimeHandle,
    required int profileGeneration,
    required TunOptions options,
  }) async {
    events.add('start:$runtimeHandle:$profileGeneration:${options.mtu}');
  }

  @override
  Future<void> stop() async => events.add('stop');
}
