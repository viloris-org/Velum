import 'dart:async';

import 'package:flutter/foundation.dart';

import 'client_engine_activation.dart';
import 'native_client.dart';
import 'native_engine.dart';
import 'traffic_runtime.dart';

typedef ClientEngineFactory = ClientEngineBridge Function(String libraryPath);

/// Owns the native engine handle and publishes the default-node snapshot.
final class ClientEngineController extends ChangeNotifier
    implements TrafficRuntime {
  ClientEngineController({
    ClientEngineFactory? engineFactory,
    this.pollInterval = const Duration(milliseconds: 200),
  }) : _engineFactory = engineFactory ?? NativeClientEngine.open;

  final ClientEngineFactory _engineFactory;
  final Duration pollInterval;
  ClientEngineBridge? _engine;
  String? _libraryPath;
  String? _defaultNode;
  Timer? _pollTimer;
  ClientRuntimeSnapshot _snapshot = const ClientRuntimeSnapshot.stopped();
  bool _disposed = false;

  @override
  ClientRuntimeSnapshot get snapshot => _snapshot;
  bool get online => _snapshot.phase == ClientRuntimePhase.online;

  int activate(ClientEngineActivation activation) {
    _ensureActive();
    final engine = _engineFor(activation.libraryPath);
    try {
      final generation = engine.activate(
        activation.nodes,
        defaultNode: activation.defaultNode,
      );
      _defaultNode = activation.defaultNode;
      refresh();
      _syncPolling();
      return generation;
    } finally {
      activation.clearSecrets();
    }
  }

  bool refresh() {
    _ensureActive();
    final engine = _engine;
    final defaultNode = _defaultNode;
    if (engine == null || defaultNode == null) return false;
    final next = engine.nodeSnapshot(defaultNode).runtime;
    if (next.revision < _snapshot.revision ||
        next.generation < _snapshot.generation) {
      return false;
    }
    _snapshot = next;
    _syncPolling();
    notifyListeners();
    return true;
  }

  @override
  int startLoopbackProxy({
    int requestedPort = 0,
    String routingRules = 'MATCH,PROXY',
  }) {
    _ensureActive();
    if (!online || _engine == null) {
      throw const ClientControlException(ClientControlStatus.configuration);
    }
    return _engine!.startLoopbackProxy(
      requestedPort: requestedPort,
      routingRules: routingRules,
    );
  }

  @override
  void stopLoopbackProxy() => _engine?.stopLoopbackProxy();

  @override
  int runtimeHandleForTun() {
    throw UnsupportedError(
      'The multi-node engine does not expose a single runtime handle for TUN.',
    );
  }

  int? stop() {
    _ensureActive();
    _pollTimer?.cancel();
    _pollTimer = null;
    final engine = _engine;
    if (engine == null) return null;
    final generation = engine.stop();
    _snapshot = const ClientRuntimeSnapshot.stopped();
    notifyListeners();
    return generation;
  }

  ClientEngineBridge _engineFor(String libraryPath) {
    final existing = _engine;
    if (existing != null && _libraryPath == libraryPath) return existing;
    existing?.destroy();
    final engine = _engineFactory(libraryPath);
    _engine = engine;
    _libraryPath = libraryPath;
    _defaultNode = null;
    _snapshot = const ClientRuntimeSnapshot.stopped();
    return engine;
  }

  void _syncPolling() {
    if (_disposed) return;
    if (const {
      ClientRuntimePhase.connecting,
      ClientRuntimePhase.online,
    }.contains(_snapshot.phase)) {
      _pollTimer ??= Timer.periodic(pollInterval, (_) {
        try {
          refresh();
        } on Object {
          // The next explicit action can surface a native-handle failure.
        }
      });
    } else {
      _pollTimer?.cancel();
      _pollTimer = null;
    }
  }

  void _ensureActive() {
    if (_disposed) throw StateError('ClientEngineController is disposed.');
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _pollTimer?.cancel();
    try {
      _engine?.stopLoopbackProxy();
      _engine?.stop();
      _engine?.destroy();
    } on Object {
      // Widget disposal cannot surface native cleanup errors.
    }
    super.dispose();
  }
}
