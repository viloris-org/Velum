import 'dart:convert';
import 'dart:ffi';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'native_client.dart';

final class _EngineByteSlice extends Struct {
  external Pointer<Uint8> pointer;

  @Size()
  external int length;
}

final class _EngineClientConfigInput extends Struct {
  external _EngineByteSlice relayAddress;
  external _EngineByteSlice serverName;
  external _EngineByteSlice credential;
  external _EngineByteSlice certificatePem;

  @Uint64()
  external int connectTimeoutMillis;

  @Uint32()
  external int trustMode;
}

final class _EngineNodeInput extends Struct {
  external _EngineByteSlice id;
  external _EngineByteSlice alias;
  external _EngineClientConfigInput configuration;
}

final class _EngineRuntimeSnapshot extends Struct {
  @Uint64()
  external int revision;

  @Uint64()
  external int generation;

  @Uint32()
  external int phase;

  @Uint32()
  external int failure;
}

final class _EngineNodeSnapshot extends Struct {
  @Uint64()
  external int profileGeneration;

  @Uint32()
  external int configured;

  @Uint32()
  external int isDefault;

  external _EngineRuntimeSnapshot runtime;
}

typedef _EngineVersionNative = Uint16 Function();
typedef _EngineVersionDart = int Function();
typedef _EngineCreateNative = Int32 Function(Pointer<Uint64>);
typedef _EngineCreateDart = int Function(Pointer<Uint64>);
typedef _EngineActivateNative =
    Int32 Function(
      Uint64,
      Pointer<_EngineNodeInput>,
      Size,
      _EngineByteSlice,
      Pointer<Uint64>,
    );
typedef _EngineActivateDart =
    int Function(
      int,
      Pointer<_EngineNodeInput>,
      int,
      _EngineByteSlice,
      Pointer<Uint64>,
    );
typedef _EngineNodeSnapshotNative =
    Int32 Function(Uint64, _EngineByteSlice, Pointer<_EngineNodeSnapshot>);
typedef _EngineNodeSnapshotDart =
    int Function(int, _EngineByteSlice, Pointer<_EngineNodeSnapshot>);
typedef _EngineStopNative = Int32 Function(Uint64, Pointer<Uint64>);
typedef _EngineStopDart = int Function(int, Pointer<Uint64>);
typedef _EngineDestroyNative = Int32 Function(Uint64);
typedef _EngineDestroyDart = int Function(int);
typedef _EngineProxyStartNative =
    Int32 Function(Uint64, Uint16, _EngineByteSlice, Pointer<Uint16>);
typedef _EngineProxyStartDart =
    int Function(int, int, _EngineByteSlice, Pointer<Uint16>);
typedef _EngineProxyStopNative = Int32 Function(Uint64);
typedef _EngineProxyStopDart = int Function(int);

/// A resolved node input whose secret bytes live only until native activation.
final class ClientEngineNodeConfiguration {
  const ClientEngineNodeConfiguration({
    required this.id,
    required this.alias,
    required this.relayAddress,
    required this.serverName,
    required this.credential,
    required this.trustMode,
    required this.certificatePem,
    this.connectTimeoutMillis = 5000,
  });

  final String id;
  final String alias;
  final String relayAddress;
  final String serverName;
  final Uint8List credential;
  final ClientTrustMode trustMode;
  final Uint8List certificatePem;
  final int connectTimeoutMillis;

  void clearSecrets() {
    credential.fillRange(0, credential.length, 0);
    certificatePem.fillRange(0, certificatePem.length, 0);
  }
}

/// Payload-free state published by the native engine for one configured node.
final class ClientEngineNodeSnapshot {
  const ClientEngineNodeSnapshot({
    required this.profileGeneration,
    required this.isDefault,
    required this.runtime,
  });

  final int profileGeneration;
  final bool isDefault;
  final ClientRuntimeSnapshot runtime;
}

abstract interface class ClientEngineBridge {
  int activate(
    List<ClientEngineNodeConfiguration> nodes, {
    required String defaultNode,
  });
  ClientEngineNodeSnapshot nodeSnapshot(String reference);
  int stop();
  int startLoopbackProxy({int requestedPort = 0, String routingRules});
  void stopLoopbackProxy();
  void destroy();
}

/// Versioned binding for native multi-node lifecycle control.
final class NativeClientEngine implements ClientEngineBridge {
  NativeClientEngine._({
    required _EngineActivateDart activate,
    required _EngineNodeSnapshotDart nodeSnapshot,
    required _EngineStopDart stop,
    required _EngineDestroyDart destroy,
    required _EngineProxyStartDart startProxy,
    required _EngineProxyStopDart stopProxy,
    required this.handle,
  }) : _activate = activate,
       _nodeSnapshot = nodeSnapshot,
       _stop = stop,
       _destroy = destroy,
       _startProxy = startProxy,
       _stopProxy = stopProxy;

  static const _abiVersion = 1;

  final _EngineActivateDart _activate;
  final _EngineNodeSnapshotDart _nodeSnapshot;
  final _EngineStopDart _stop;
  final _EngineDestroyDart _destroy;
  final _EngineProxyStartDart _startProxy;
  final _EngineProxyStopDart _stopProxy;
  final int handle;
  bool _destroyed = false;

  factory NativeClientEngine.open(String libraryPath) {
    final library = DynamicLibrary.open(libraryPath);
    final version = library
        .lookup<NativeFunction<_EngineVersionNative>>(
          'velum_client_engine_abi_version',
        )
        .asFunction<_EngineVersionDart>();
    if (version() != _abiVersion) {
      throw const ClientControlException(
        ClientControlStatus.configuration,
        'The native multi-node engine ABI is unsupported.',
      );
    }
    final create = library
        .lookup<NativeFunction<_EngineCreateNative>>(
          'velum_client_engine_create',
        )
        .asFunction<_EngineCreateDart>();
    final output = calloc<Uint64>();
    try {
      _checkEngineStatus(create(output));
      return NativeClientEngine._(
        activate: library
            .lookup<NativeFunction<_EngineActivateNative>>(
              'velum_client_engine_activate_v1',
            )
            .asFunction<_EngineActivateDart>(),
        nodeSnapshot: library
            .lookup<NativeFunction<_EngineNodeSnapshotNative>>(
              'velum_client_engine_node_snapshot_v1',
            )
            .asFunction<_EngineNodeSnapshotDart>(),
        stop: library
            .lookup<NativeFunction<_EngineStopNative>>(
              'velum_client_engine_stop',
            )
            .asFunction<_EngineStopDart>(),
        destroy: library
            .lookup<NativeFunction<_EngineDestroyNative>>(
              'velum_client_engine_destroy',
            )
            .asFunction<_EngineDestroyDart>(),
        startProxy: library
            .lookup<NativeFunction<_EngineProxyStartNative>>(
              'velum_client_engine_proxy_start_v1',
            )
            .asFunction<_EngineProxyStartDart>(),
        stopProxy: library
            .lookup<NativeFunction<_EngineProxyStopNative>>(
              'velum_client_engine_proxy_stop',
            )
            .asFunction<_EngineProxyStopDart>(),
        handle: output.value,
      );
    } finally {
      calloc.free(output);
    }
  }

  @override
  int activate(
    List<ClientEngineNodeConfiguration> nodes, {
    required String defaultNode,
  }) {
    _ensureAlive();
    if (nodes.isEmpty || defaultNode.isEmpty) {
      throw const ClientControlException(ClientControlStatus.invalidArgument);
    }
    final inputs = calloc<_EngineNodeInput>(nodes.length);
    final generation = calloc<Uint64>();
    final allocations = <_EngineAllocation>[];
    try {
      for (var index = 0; index < nodes.length; index++) {
        final node = nodes[index];
        final input = (inputs + index).ref;
        final id = _copyText(node.id, allocations);
        final alias = _copyText(node.alias, allocations);
        final relay = _copyText(node.relayAddress, allocations);
        final serverName = _copyText(node.serverName, allocations);
        final credential = _copyBytes(node.credential, allocations);
        final certificate = _copyBytes(node.certificatePem, allocations);
        input.id
          ..pointer = id.pointer
          ..length = id.length;
        input.alias
          ..pointer = alias.pointer
          ..length = alias.length;
        input.configuration.relayAddress
          ..pointer = relay.pointer
          ..length = relay.length;
        input.configuration.serverName
          ..pointer = serverName.pointer
          ..length = serverName.length;
        input.configuration.credential
          ..pointer = credential.pointer
          ..length = credential.length;
        input.configuration.certificatePem
          ..pointer = certificate.pointer
          ..length = certificate.length;
        input.configuration.connectTimeoutMillis = node.connectTimeoutMillis;
        input.configuration.trustMode = node.trustMode.index;
      }
      final selected = _copyText(defaultNode, allocations);
      final selection = calloc<_EngineByteSlice>();
      try {
        selection.ref
          ..pointer = selected.pointer
          ..length = selected.length;
        _checkEngineStatus(
          _activate(handle, inputs, nodes.length, selection.ref, generation),
        );
      } finally {
        calloc.free(selection);
      }
      return generation.value;
    } finally {
      for (final allocation in allocations) {
        allocation.clearAndFree();
      }
      calloc.free(generation);
      calloc.free(inputs);
    }
  }

  @override
  ClientEngineNodeSnapshot nodeSnapshot(String reference) {
    _ensureAlive();
    final output = calloc<_EngineNodeSnapshot>();
    final allocations = <_EngineAllocation>[];
    try {
      final value = _copyText(reference, allocations);
      final input = calloc<_EngineByteSlice>();
      try {
        input.ref
          ..pointer = value.pointer
          ..length = value.length;
        _checkEngineStatus(_nodeSnapshot(handle, input.ref, output));
      } finally {
        calloc.free(input);
      }
      return ClientEngineNodeSnapshot(
        profileGeneration: output.ref.profileGeneration,
        isDefault: output.ref.isDefault != 0,
        runtime: ClientRuntimeSnapshot(
          revision: output.ref.runtime.revision,
          generation: output.ref.runtime.generation,
          phase: _decodeEnginePhase(output.ref.runtime.phase),
          failure: _decodeEngineFailure(output.ref.runtime.failure),
        ),
      );
    } finally {
      for (final allocation in allocations) {
        allocation.clearAndFree();
      }
      calloc.free(output);
    }
  }

  @override
  int stop() {
    _ensureAlive();
    final generation = calloc<Uint64>();
    try {
      _checkEngineStatus(_stop(handle, generation));
      return generation.value;
    } finally {
      calloc.free(generation);
    }
  }

  @override
  int startLoopbackProxy({
    int requestedPort = 0,
    String routingRules = 'MATCH,PROXY',
  }) {
    _ensureAlive();
    if (requestedPort < 0 || requestedPort > 65535) {
      throw const ClientControlException(ClientControlStatus.invalidArgument);
    }
    final port = calloc<Uint16>();
    final allocations = <_EngineAllocation>[];
    try {
      final rules = _copyText(routingRules, allocations);
      final input = calloc<_EngineByteSlice>();
      try {
        input.ref
          ..pointer = rules.pointer
          ..length = rules.length;
        _checkEngineStatus(_startProxy(handle, requestedPort, input.ref, port));
      } finally {
        calloc.free(input);
      }
      return port.value;
    } finally {
      for (final allocation in allocations) {
        allocation.clearAndFree();
      }
      calloc.free(port);
    }
  }

  @override
  void stopLoopbackProxy() {
    _ensureAlive();
    _checkEngineStatus(_stopProxy(handle));
  }

  @override
  void destroy() {
    if (_destroyed) return;
    final status = _destroy(handle);
    if (status == ClientControlStatus.invalidHandle.index) {
      _destroyed = true;
      return;
    }
    _checkEngineStatus(status);
    _destroyed = true;
  }

  void _ensureAlive() {
    if (_destroyed) {
      throw const ClientControlException(ClientControlStatus.invalidHandle);
    }
  }

  static _EngineAllocation _copyText(
    String value,
    List<_EngineAllocation> allocations,
  ) => _copyBytes(Uint8List.fromList(utf8.encode(value)), allocations);

  static _EngineAllocation _copyBytes(
    Uint8List value,
    List<_EngineAllocation> allocations,
  ) {
    if (value.isEmpty) return _EngineAllocation(Pointer.fromAddress(0), 0);
    final pointer = calloc<Uint8>(value.length);
    pointer.asTypedList(value.length).setAll(0, value);
    final allocation = _EngineAllocation(pointer, value.length);
    allocations.add(allocation);
    return allocation;
  }
}

void _checkEngineStatus(int value) {
  if (value == ClientControlStatus.ok.index) return;
  if (value < 0 || value >= ClientControlStatus.values.length) {
    throw const ClientControlException(ClientControlStatus.internal);
  }
  throw ClientControlException(ClientControlStatus.values[value]);
}

ClientRuntimePhase _decodeEnginePhase(int value) {
  if (value < 0 || value >= ClientRuntimePhase.values.length) {
    throw const ClientControlException(ClientControlStatus.internal);
  }
  return ClientRuntimePhase.values[value];
}

ClientRuntimeFailure _decodeEngineFailure(int value) {
  if (value < 0 || value >= ClientRuntimeFailure.values.length) {
    throw const ClientControlException(ClientControlStatus.internal);
  }
  return ClientRuntimeFailure.values[value];
}

final class _EngineAllocation {
  const _EngineAllocation(this.pointer, this.length);

  final Pointer<Uint8> pointer;
  final int length;

  void clearAndFree() {
    if (length == 0) return;
    pointer.asTypedList(length).fillRange(0, length, 0);
    calloc.free(pointer);
  }
}
