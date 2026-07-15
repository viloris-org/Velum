import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';

import 'android_vpn.dart';
import 'client_controller.dart';
import 'desktop_tun.dart';
import 'native_client.dart';
import 'system_proxy.dart';
import 'traffic_runtime.dart';

enum TrafficMode { off, systemProxy, tun }

enum TrafficModePhase { inactive, applying, active, failed }

abstract interface class TrafficAdapter {
  TrafficMode get mode;
  Future<void>? get completion;
  Future<void> recover();
  Future<void> activate();
  Future<void> deactivate();
}

final class DesktopSystemProxyAdapter implements TrafficAdapter {
  DesktopSystemProxyAdapter(
    this._runtime,
    this._systemProxy, {
    SystemProxyOptions Function()? options,
    String Function()? routingRules,
  }) : _options = options ?? SystemProxyOptions.new,
       _routingRules = routingRules ?? (() => 'MATCH,PROXY');

  final TrafficRuntime _runtime;
  final SystemProxy _systemProxy;
  final SystemProxyOptions Function() _options;
  final String Function() _routingRules;

  @override
  TrafficMode get mode => TrafficMode.systemProxy;

  @override
  Future<void>? get completion => null;

  @override
  Future<void> recover() async {
    await _systemProxy.disable();
    _runtime.stopLoopbackProxy();
  }

  @override
  Future<void> activate() async {
    final options = _options();
    final port = _runtime.startLoopbackProxy(
      requestedPort: options.requestedPort,
      routingRules: _routingRules(),
    );
    try {
      await _systemProxy.enable(port, bypassHosts: options.bypassHosts);
    } on Object {
      try {
        await _systemProxy.disable();
      } on Object {
        // The persisted backup is retained when restoration fails.
      }
      _runtime.stopLoopbackProxy();
      rethrow;
    }
  }

  @override
  Future<void> deactivate() async {
    Object? restoreFailure;
    try {
      await _systemProxy.disable();
    } on Object catch (error) {
      restoreFailure = error;
    }
    _runtime.stopLoopbackProxy();
    if (restoreFailure != null) throw restoreFailure;
  }
}

final class AndroidTunAdapter implements TrafficAdapter {
  AndroidTunAdapter(
    this._runtime,
    this._vpn, {
    required String Function() libraryPath,
    TunOptions Function()? options,
  }) : _libraryPath = libraryPath,
       _options = options ?? TunOptions.new;

  final TrafficRuntime _runtime;
  final AndroidVpn _vpn;
  final String Function() _libraryPath;
  final TunOptions Function() _options;

  @override
  TrafficMode get mode => TrafficMode.tun;

  @override
  Future<void>? get completion => _vpn.completion;

  @override
  Future<void> recover() => _vpn.stop();

  @override
  Future<void> activate() async {
    if (!await _vpn.requestPermission()) {
      throw StateError('VPN permission was not granted.');
    }
    await _vpn.start(
      runtimeHandle: _runtime.runtimeHandleForTun(),
      libraryPath: _libraryPath(),
      options: _options(),
    );
  }

  @override
  Future<void> deactivate() => _vpn.stop();
}

final class DesktopTunAdapter implements TrafficAdapter {
  DesktopTunAdapter(this._runtime, this._host, {TunOptions Function()? options})
    : _options = options ?? TunOptions.new;

  final TrafficRuntime _runtime;
  final DesktopTunControl _host;
  final TunOptions Function() _options;

  @override
  TrafficMode get mode => TrafficMode.tun;

  @override
  Future<void>? get completion => null;

  @override
  Future<void> recover() => _host.recover();

  @override
  Future<void> activate() => _host.start(
    runtimeHandle: _runtime.runtimeHandleForTun(),
    profileGeneration: _runtime.snapshot.generation,
    options: _options(),
  );

  @override
  Future<void> deactivate() => _host.stop();
}

/// Reconciles user traffic-routing intent with the authoritative runtime state.
final class TrafficModeController extends ChangeNotifier {
  TrafficModeController({
    required TrafficRuntime runtime,
    required Iterable<TrafficAdapter> adapters,
  }) : _runtime = runtime,
       _adapters = {for (final adapter in adapters) adapter.mode: adapter} {
    _runtime.addListener(_runtimeChanged);
    unawaited(
      _serialize(_recover).catchError((_) {
        if (_disposed) return;
        _recoveryFailed = true;
        _error = 'Previous traffic routing settings could not be restored.';
        _setPhase(TrafficModePhase.failed);
      }),
    );
  }

  factory TrafficModeController.platform({
    required ClientController runtime,
    required String Function() libraryPath,
    SystemProxyOptions Function()? systemProxyOptions,
    TunOptions Function()? tunOptions,
    String Function()? routingRules,
  }) {
    final adapters = <TrafficAdapter>[];
    if (Platform.isLinux || Platform.isMacOS || Platform.isWindows) {
      adapters.add(
        DesktopSystemProxyAdapter(
          runtime,
          SystemProxy(),
          options: systemProxyOptions,
          routingRules: routingRules,
        ),
      );
      if (DesktopTunHost.buildEnabled) {
        adapters.add(
          DesktopTunAdapter(runtime, DesktopTunHost(), options: tunOptions),
        );
      }
    }
    if (Platform.isAndroid) {
      adapters.add(
        AndroidTunAdapter(
          runtime,
          AndroidVpn(),
          libraryPath: libraryPath,
          options: tunOptions,
        ),
      );
    }
    return TrafficModeController(runtime: runtime, adapters: adapters);
  }

  final TrafficRuntime _runtime;
  final Map<TrafficMode, TrafficAdapter> _adapters;
  Future<void> _pending = Future.value();
  TrafficMode _selectedMode = TrafficMode.off;
  TrafficMode _activeMode = TrafficMode.off;
  TrafficModePhase _phase = TrafficModePhase.inactive;
  String? _error;
  bool _disposed = false;
  bool _recoveryFailed = false;
  int _activationEpoch = 0;

  TrafficMode get selectedMode => _selectedMode;
  TrafficMode get activeMode => _activeMode;
  TrafficModePhase get phase => _phase;
  String? get error => _error;
  bool get runtimeOnline =>
      _runtime.snapshot.phase == ClientRuntimePhase.online;
  bool get busy => _phase == TrafficModePhase.applying;
  Set<TrafficMode> get availableModes => {TrafficMode.off, ..._adapters.keys};

  Future<void> select(TrafficMode mode) {
    if (!availableModes.contains(mode)) {
      throw UnsupportedError('$mode is not supported on this platform.');
    }
    _selectedMode = mode;
    _error = null;
    notifyListeners();
    return _scheduleReconcile();
  }

  /// Removes OS integration before the encrypted runtime is stopped.
  /// The selected mode remains armed for the next successful connection.
  Future<void> suspend() => _serialize(() => _deactivateCurrent());

  Future<void> _scheduleReconcile() => _serialize(_reconcile);

  Future<void> _recover() async {
    for (final adapter in _adapters.values) {
      await adapter.recover();
    }
    _recoveryFailed = false;
    _error = null;
  }

  Future<void> _serialize(Future<void> Function() operation) {
    final result = _pending.then(
      (_) => operation(),
      onError: (_) => operation(),
    );
    _pending = result.then<void>((_) {}, onError: (_, _) {});
    return result;
  }

  Future<void> _reconcile() async {
    if (_disposed) return;
    if (_recoveryFailed) {
      _setPhase(TrafficModePhase.applying);
      try {
        await _recover();
      } on Object {
        _error = 'Previous traffic routing settings could not be restored.';
        _setPhase(TrafficModePhase.failed);
        rethrow;
      }
    }
    final target = runtimeOnline ? _selectedMode : TrafficMode.off;
    if (_activeMode == target && _phase != TrafficModePhase.failed) {
      _setPhase(
        target == TrafficMode.off
            ? TrafficModePhase.inactive
            : TrafficModePhase.active,
      );
      return;
    }

    _setPhase(TrafficModePhase.applying);
    try {
      await _deactivateCurrent(notify: false);
      if (target != TrafficMode.off) {
        final adapter = _adapters[target]!;
        await adapter.activate();
        _activeMode = target;
        _watchCompletion(adapter, ++_activationEpoch);
      }
      _error = null;
      _setPhase(
        target == TrafficMode.off
            ? TrafficModePhase.inactive
            : TrafficModePhase.active,
      );
    } on Object catch (error) {
      _activeMode = TrafficMode.off;
      _error = _messageFor(error);
      _setPhase(TrafficModePhase.failed);
      rethrow;
    }
  }

  Future<void> _deactivateCurrent({bool notify = true}) async {
    _activationEpoch += 1;
    final active = _activeMode;
    if (active != TrafficMode.off) {
      await _adapters[active]!.deactivate();
      _activeMode = TrafficMode.off;
    }
    _error = null;
    if (notify) _setPhase(TrafficModePhase.inactive);
  }

  void _runtimeChanged() {
    if (!_disposed) unawaited(_scheduleReconcile().catchError((_) {}));
  }

  void _watchCompletion(TrafficAdapter adapter, int epoch) {
    final completion = adapter.completion;
    if (completion == null) return;
    unawaited(
      completion.then<void>((_) {}, onError: (_, _) {}).whenComplete(() {
        if (_disposed ||
            epoch != _activationEpoch ||
            _activeMode != adapter.mode) {
          return;
        }
        _activeMode = TrafficMode.off;
        _error = adapter.mode == TrafficMode.tun
            ? 'The TUN VPN stopped unexpectedly.'
            : 'Traffic routing stopped unexpectedly.';
        _setPhase(TrafficModePhase.failed);
      }),
    );
  }

  void _setPhase(TrafficModePhase value) {
    if (_disposed) return;
    _phase = value;
    notifyListeners();
  }

  String _messageFor(Object error) {
    if (error is StateError) {
      return error.message;
    }
    if (error is UnsupportedError && error.message != null) {
      return '${error.message}';
    }
    return switch (_selectedMode) {
      TrafficMode.systemProxy => 'The system proxy could not be installed.',
      TrafficMode.tun => 'The TUN VPN could not be started.',
      TrafficMode.off => 'Traffic routing could not be disabled.',
    };
  }

  @override
  void dispose() {
    if (_disposed) return;
    _runtime.removeListener(_runtimeChanged);
    _disposed = true;
    _activationEpoch += 1;
    for (final adapter in _adapters.values) {
      unawaited(adapter.deactivate().catchError((_) {}));
    }
    super.dispose();
  }
}
