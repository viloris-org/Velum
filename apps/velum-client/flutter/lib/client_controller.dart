import 'dart:async';

import 'package:flutter/foundation.dart';

import 'native_client.dart';
import 'traffic_runtime.dart';

typedef ClientRuntimeFactory = ClientRuntimeBridge Function(String libraryPath);
typedef ClientReconnectConfiguration =
    FutureOr<ClientRuntimeConfiguration> Function();

enum ClientReconnectPhase { inactive, waiting, reconnecting, exhausted }

final class ClientReconnectStatus {
  const ClientReconnectStatus({
    required this.phase,
    this.attempt = 0,
    this.maxAttempts = 0,
  });

  const ClientReconnectStatus.inactive()
    : phase = ClientReconnectPhase.inactive,
      attempt = 0,
      maxAttempts = 0;

  final ClientReconnectPhase phase;
  final int attempt;
  final int maxAttempts;

  @override
  bool operator ==(Object other) =>
      other is ClientReconnectStatus &&
      phase == other.phase &&
      attempt == other.attempt &&
      maxAttempts == other.maxAttempts;

  @override
  int get hashCode => Object.hash(phase, attempt, maxAttempts);
}

/// Owns the native runtime handle and exposes only authoritative snapshots.
class ClientController extends ChangeNotifier implements TrafficRuntime {
  ClientController({
    ClientRuntimeFactory? runtimeFactory,
    this.pollInterval = const Duration(milliseconds: 200),
    this.reconnectDelay = const Duration(seconds: 1),
    this.maxReconnectAttempts = 3,
  }) : _runtimeFactory = runtimeFactory ?? NativeClientRuntime.open;

  final ClientRuntimeFactory _runtimeFactory;
  final Duration pollInterval;
  final Duration reconnectDelay;
  final int maxReconnectAttempts;

  ClientRuntimeBridge? _runtime;
  String? _libraryPath;
  Timer? _pollTimer;
  Timer? _reconnectTimer;
  ClientRuntimeSnapshot _snapshot = const ClientRuntimeSnapshot.stopped();
  int _acceptedRevision = -1;
  int _minimumGeneration = 0;
  bool _disposed = false;
  Object? _pollingError;
  ClientReconnectConfiguration? _reconnectConfiguration;
  ClientReconnectStatus _reconnectStatus =
      const ClientReconnectStatus.inactive();
  int _reconnectEpoch = 0;

  @override
  ClientRuntimeSnapshot get snapshot => _snapshot;

  Object? get pollingError => _pollingError;

  ClientReconnectStatus get reconnectStatus => _reconnectStatus;

  int start(
    ClientRuntimeConfiguration configuration, {
    ClientReconnectConfiguration? reconnectConfiguration,
  }) {
    _ensureActive();
    _cancelReconnect();
    _reconnectConfiguration = reconnectConfiguration;
    return _start(configuration);
  }

  int _start(ClientRuntimeConfiguration configuration) {
    final runtime = _runtimeFor(configuration.libraryPath);
    final generation = runtime.start(configuration);
    if (generation > _minimumGeneration) _minimumGeneration = generation;
    refresh();
    _syncPolling();
    return generation;
  }

  int? stop() {
    _ensureActive();
    _cancelReconnect();
    _reconnectConfiguration = null;
    final runtime = _runtime;
    if (runtime == null) return null;
    final generation = runtime.stop();
    if (generation > _minimumGeneration) _minimumGeneration = generation;
    refresh();
    _syncPolling();
    return generation;
  }

  @override
  int startLoopbackProxy({
    int requestedPort = 0,
    String routingRules = 'MATCH,PROXY',
  }) {
    _ensureActive();
    final runtime = _runtime;
    if (runtime == null || runtime is! ClientProxyBridge) {
      throw const ClientControlException(ClientControlStatus.configuration);
    }
    return (runtime as ClientProxyBridge).startLoopbackProxy(
      requestedPort: requestedPort,
      routingRules: routingRules,
    );
  }

  @override
  void stopLoopbackProxy() {
    if (_runtime case final ClientProxyBridge runtime) {
      runtime.stopLoopbackProxy();
    }
  }

  @override
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
    _syncReconnect(next);
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
    _cancelReconnect();
    _reconnectConfiguration = null;
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

  void _syncReconnect(ClientRuntimeSnapshot next) {
    if (next.phase == ClientRuntimePhase.online) {
      _setReconnectStatus(const ClientReconnectStatus.inactive());
      return;
    }
    if (next.phase != ClientRuntimePhase.failed ||
        !_isRetryable(next.failure)) {
      return;
    }
    if (_reconnectConfiguration == null || _reconnectTimer != null) return;

    final nextAttempt = _reconnectStatus.attempt + 1;
    if (nextAttempt > maxReconnectAttempts) {
      _setReconnectStatus(
        ClientReconnectStatus(
          phase: ClientReconnectPhase.exhausted,
          attempt: _reconnectStatus.attempt,
          maxAttempts: maxReconnectAttempts,
        ),
      );
      return;
    }
    _setReconnectStatus(
      ClientReconnectStatus(
        phase: ClientReconnectPhase.waiting,
        attempt: nextAttempt,
        maxAttempts: maxReconnectAttempts,
      ),
    );
    final epoch = ++_reconnectEpoch;
    _reconnectTimer = Timer(_retryDelay(nextAttempt), () {
      _reconnectTimer = null;
      unawaited(_retry(epoch));
    });
  }

  Future<void> _retry(int epoch) async {
    if (_disposed || epoch != _reconnectEpoch) return;
    final configuration = _reconnectConfiguration;
    if (configuration == null || _snapshot.phase != ClientRuntimePhase.failed) {
      return;
    }
    _setReconnectStatus(
      ClientReconnectStatus(
        phase: ClientReconnectPhase.reconnecting,
        attempt: _reconnectStatus.attempt,
        maxAttempts: maxReconnectAttempts,
      ),
    );
    try {
      final resolved = await configuration();
      if (_disposed ||
          epoch != _reconnectEpoch ||
          _snapshot.phase != ClientRuntimePhase.failed ||
          _reconnectConfiguration == null) {
        return;
      }
      _start(resolved);
    } on Object {
      if (!_disposed && epoch == _reconnectEpoch) _syncReconnect(_snapshot);
    }
  }

  Duration _retryDelay(int attempt) {
    final milliseconds = reconnectDelay.inMilliseconds * (1 << (attempt - 1));
    return Duration(milliseconds: milliseconds > 30000 ? 30000 : milliseconds);
  }

  void _cancelReconnect() {
    _reconnectEpoch += 1;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    _setReconnectStatus(const ClientReconnectStatus.inactive());
  }

  void _setReconnectStatus(ClientReconnectStatus status) {
    if (_reconnectStatus == status) return;
    _reconnectStatus = status;
    if (!_disposed) notifyListeners();
  }

  bool _isRetryable(ClientRuntimeFailure failure) => switch (failure) {
    ClientRuntimeFailure.connectTimeout ||
    ClientRuntimeFailure.connection ||
    ClientRuntimeFailure.transport => true,
    ClientRuntimeFailure.none ||
    ClientRuntimeFailure.certificate ||
    ClientRuntimeFailure.controlTooLarge ||
    ClientRuntimeFailure.datagramTooLarge ||
    ClientRuntimeFailure.datagramUnavailable ||
    ClientRuntimeFailure.protocol => false,
  };

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
