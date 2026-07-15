import 'dart:async';

import 'package:flutter/foundation.dart';

import 'native_client.dart';

typedef ClientRuntimeFactory = ClientRuntimeBridge Function(String libraryPath);

/// Owns the native runtime handle and exposes only authoritative snapshots.
class ClientController extends ChangeNotifier {
  ClientController({
    ClientRuntimeFactory? runtimeFactory,
    this.pollInterval = const Duration(milliseconds: 200),
  }) : _runtimeFactory = runtimeFactory ?? NativeClientRuntime.open;

  final ClientRuntimeFactory _runtimeFactory;
  final Duration pollInterval;

  ClientRuntimeBridge? _runtime;
  String? _libraryPath;
  Timer? _pollTimer;
  ClientRuntimeSnapshot _snapshot = const ClientRuntimeSnapshot.stopped();
  int _acceptedRevision = -1;
  int _minimumGeneration = 0;
  bool _disposed = false;
  Object? _pollingError;

  ClientRuntimeSnapshot get snapshot => _snapshot;

  Object? get pollingError => _pollingError;

  int start(ClientRuntimeConfiguration configuration) {
    _ensureActive();
    final runtime = _runtimeFor(configuration.libraryPath);
    final generation = runtime.start(configuration);
    if (generation > _minimumGeneration) _minimumGeneration = generation;
    refresh();
    _syncPolling();
    return generation;
  }

  int? stop() {
    _ensureActive();
    final runtime = _runtime;
    if (runtime == null) return null;
    final generation = runtime.stop();
    if (generation > _minimumGeneration) _minimumGeneration = generation;
    refresh();
    _syncPolling();
    return generation;
  }

  int startLoopbackProxy({int requestedPort = 0}) {
    _ensureActive();
    final runtime = _runtime;
    if (runtime == null || runtime is! ClientProxyBridge) {
      throw const ClientControlException(ClientControlStatus.configuration);
    }
    return (runtime as ClientProxyBridge).startLoopbackProxy(
      requestedPort: requestedPort,
    );
  }

  void stopLoopbackProxy() {
    if (_runtime case final ClientProxyBridge runtime) {
      runtime.stopLoopbackProxy();
    }
  }

  int runtimeHandleForTun() {
    _ensureActive();
    final runtime = _runtime;
    if (runtime is! ClientTunBridge) {
      throw const ClientControlException(ClientControlStatus.configuration);
    }
    return (runtime as ClientTunBridge).runtimeHandle;
  }

  /// Polls the latest-value snapshot and rejects stale revisions or generations.
  bool refresh() {
    _ensureActive();
    final runtime = _runtime;
    if (runtime == null) return false;
    final next = runtime.snapshot();
    final recoveredFromPollingError = _pollingError != null;
    _pollingError = null;
    if (next.revision <= _acceptedRevision ||
        next.generation < _minimumGeneration ||
        next.generation < _snapshot.generation) {
      if (recoveredFromPollingError) notifyListeners();
      return false;
    }

    _acceptedRevision = next.revision;
    _snapshot = next;
    _syncPolling();
    notifyListeners();
    return true;
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _pollTimer?.cancel();
    _pollTimer = null;
    final runtime = _runtime;
    _runtime = null;
    if (runtime != null) {
      try {
        runtime.stop();
      } on Object {
        // Disposal must continue through handle destruction.
      }
      try {
        runtime.destroy();
      } on Object {
        // Widget disposal cannot surface native cleanup failures.
      }
    }
    super.dispose();
  }

  ClientRuntimeBridge _runtimeFor(String libraryPath) {
    final existing = _runtime;
    if (existing != null && _libraryPath == libraryPath) return existing;
    if (existing != null &&
        !const {
          ClientRuntimePhase.stopped,
          ClientRuntimePhase.failed,
        }.contains(_snapshot.phase)) {
      throw const ClientControlException(ClientControlStatus.busy);
    }

    if (existing != null) {
      try {
        existing.destroy();
      } on Object {
        // Replacing an inactive compatibility handle must still make progress.
      }
      _runtime = null;
      _libraryPath = null;
    }
    final runtime = _runtimeFactory(libraryPath);
    _runtime = runtime;
    _libraryPath = libraryPath;
    _acceptedRevision = -1;
    _minimumGeneration = 0;
    _snapshot = const ClientRuntimeSnapshot.stopped();
    _pollingError = null;
    return runtime;
  }

  void _syncPolling() {
    if (_disposed || _runtime == null) return;
    final shouldPoll = switch (_snapshot.phase) {
      ClientRuntimePhase.connecting ||
      ClientRuntimePhase.online ||
      ClientRuntimePhase.stopping => true,
      ClientRuntimePhase.stopped || ClientRuntimePhase.failed => false,
    };
    if (!shouldPoll) {
      _pollTimer?.cancel();
      _pollTimer = null;
      return;
    }
    _pollTimer ??= Timer.periodic(pollInterval, (_) => _pollSafely());
  }

  void _pollSafely() {
    if (_disposed) return;
    try {
      refresh();
    } on Object catch (error) {
      if (_disposed) return;
      if (_pollingError == null) {
        _pollingError = error;
        notifyListeners();
      }
    }
  }

  void _ensureActive() {
    if (_disposed) throw StateError('ClientController is disposed.');
  }
}
