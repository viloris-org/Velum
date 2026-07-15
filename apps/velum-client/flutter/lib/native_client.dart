import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

final class _VelumByteSlice extends Struct {
  external Pointer<Uint8> pointer;

  @Size()
  external int length;
}

final class _VelumClientConfigInput extends Struct {
  external _VelumByteSlice relayAddress;
  external _VelumByteSlice serverName;
  external _VelumByteSlice credential;
  external _VelumByteSlice certificatePem;

  @Uint64()
  external int connectTimeoutMillis;

  @Uint32()
  external int trustMode;
}

/// Fixed-width snapshot layout shared with `velum-client-ffi` ABI v1.
final class VelumRuntimeSnapshotV1 extends Struct {
  @Uint64()
  external int revision;

  @Uint64()
  external int generation;

  @Uint32()
  external int phase;

  @Uint32()
  external int failure;
}

typedef _RuntimeAbiVersionNative = Uint16 Function();
typedef _RuntimeAbiVersionDart = int Function();
typedef _RuntimeCreateNative = Int32 Function(Pointer<Uint64>);
typedef _RuntimeCreateDart = int Function(Pointer<Uint64>);
typedef _RuntimeStartV1Native =
    Int32 Function(Uint64, Pointer<_VelumClientConfigInput>, Pointer<Uint64>);
typedef _RuntimeStartV1Dart =
    int Function(int, Pointer<_VelumClientConfigInput>, Pointer<Uint64>);
typedef _RuntimeSnapshotV1Native =
    Int32 Function(Uint64, Pointer<VelumRuntimeSnapshotV1>);
typedef _RuntimeSnapshotV1Dart =
    int Function(int, Pointer<VelumRuntimeSnapshotV1>);
typedef _RuntimeStopNative = Int32 Function(Uint64, Pointer<Uint64>);
typedef _RuntimeStopDart = int Function(int, Pointer<Uint64>);
typedef _RuntimeDestroyNative = Int32 Function(Uint64);
typedef _RuntimeDestroyDart = int Function(int);
typedef _RuntimeProxyStartNative =
    Int32 Function(Uint64, Uint16, Pointer<Uint16>);
typedef _RuntimeProxyStartDart = int Function(int, int, Pointer<Uint16>);
typedef _RuntimeProxyStopNative = Int32 Function(Uint64);
typedef _RuntimeProxyStopDart = int Function(int);

enum ClientControlStatus {
  ok,
  invalidArgument,
  invalidHandle,
  configuration,
  certificate,
  busy,
  internal,
}

enum ClientRuntimePhase { stopped, connecting, online, stopping, failed }

enum ClientTrustMode { system, customCa, insecure }

enum ClientRuntimeFailure {
  none,
  certificate,
  connectTimeout,
  connection,
  controlTooLarge,
  datagramTooLarge,
  datagramUnavailable,
  protocol,
  transport,
}

class ClientRuntimeSnapshot {
  const ClientRuntimeSnapshot({
    required this.revision,
    required this.generation,
    required this.phase,
    required this.failure,
  });

  const ClientRuntimeSnapshot.stopped()
    : revision = 0,
      generation = 0,
      phase = ClientRuntimePhase.stopped,
      failure = ClientRuntimeFailure.none;

  final int revision;
  final int generation;
  final ClientRuntimePhase phase;
  final ClientRuntimeFailure failure;
}

class ClientRuntimeConfiguration {
  const ClientRuntimeConfiguration({
    required this.libraryPath,
    required this.relayAddress,
    required this.serverName,
    required this.credential,
    required this.trustMode,
    required this.certificatePem,
    this.connectTimeoutMillis = 5000,
  });

  final String libraryPath;
  final String relayAddress;
  final String serverName;
  final Uint8List credential;
  final ClientTrustMode trustMode;
  final Uint8List certificatePem;
  final int connectTimeoutMillis;
}

class ClientControlException implements Exception {
  const ClientControlException(this.status, [this.context]);

  final ClientControlStatus status;
  final String? context;

  @override
  String toString() {
    if (context case final context?) return context;
    return switch (status) {
      ClientControlStatus.ok => 'The native runtime did not report an error.',
      ClientControlStatus.invalidArgument =>
        'The native runtime received invalid input.',
      ClientControlStatus.invalidHandle =>
        'The native runtime handle is no longer valid.',
      ClientControlStatus.configuration =>
        'The client configuration was rejected.',
      ClientControlStatus.certificate =>
        'The relay certificate could not be loaded.',
      ClientControlStatus.busy =>
        'The native runtime is already processing a lifecycle command.',
      ClientControlStatus.internal => 'The native runtime failed internally.',
    };
  }
}

/// Injectable control surface used by the lifecycle controller.
abstract interface class ClientRuntimeBridge {
  int start(ClientRuntimeConfiguration configuration);

  ClientRuntimeSnapshot snapshot();

  int stop();

  void destroy();
}

abstract interface class ClientProxyBridge {
  int startLoopbackProxy({int requestedPort = 0});

  void stopLoopbackProxy();
}

abstract interface class ClientTunBridge {
  int get runtimeHandle;
}

/// Hand-written binding for the versioned asynchronous runtime control ABI.
class NativeClientRuntime
    implements ClientRuntimeBridge, ClientProxyBridge, ClientTunBridge {
  NativeClientRuntime._({
    required _RuntimeStartV1Dart start,
    required _RuntimeSnapshotV1Dart snapshot,
    required _RuntimeStopDart stop,
    required _RuntimeProxyStartDart startProxy,
    required _RuntimeProxyStopDart stopProxy,
    required _RuntimeDestroyDart destroy,
    required this.handle,
  }) : _start = start,
       _snapshot = snapshot,
       _stop = stop,
       _startProxy = startProxy,
       _stopProxy = stopProxy,
       _destroy = destroy;

  static const int _abiVersion = 2;

  final _RuntimeStartV1Dart _start;
  final _RuntimeSnapshotV1Dart _snapshot;
  final _RuntimeStopDart _stop;
  final _RuntimeProxyStartDart _startProxy;
  final _RuntimeProxyStopDart _stopProxy;
  final _RuntimeDestroyDart _destroy;
  final int handle;

  @override
  int get runtimeHandle => handle;
  bool _destroyed = false;

  static String defaultLibraryName() {
    if (Platform.isMacOS) return 'libvelum_client_ffi.dylib';
    if (Platform.isWindows) return 'velum_client_ffi.dll';
    return 'libvelum_client_ffi.so';
  }

  static NativeClientRuntime open(String libraryPath) {
    final library = DynamicLibrary.open(libraryPath);
    final abiVersion = library
        .lookup<NativeFunction<_RuntimeAbiVersionNative>>(
          'velum_client_runtime_abi_version',
        )
        .asFunction<_RuntimeAbiVersionDart>();
    if (abiVersion() != _abiVersion) {
      throw const ClientControlException(
        ClientControlStatus.configuration,
        'The native runtime control ABI is unsupported.',
      );
    }

    final create = library
        .lookup<NativeFunction<_RuntimeCreateNative>>(
          'velum_client_runtime_create',
        )
        .asFunction<_RuntimeCreateDart>();
    final start = library
        .lookup<NativeFunction<_RuntimeStartV1Native>>(
          'velum_client_runtime_start_v1',
        )
        .asFunction<_RuntimeStartV1Dart>();
    final snapshot = library
        .lookup<NativeFunction<_RuntimeSnapshotV1Native>>(
          'velum_client_runtime_snapshot_v1',
        )
        .asFunction<_RuntimeSnapshotV1Dart>();
    final stop = library
        .lookup<NativeFunction<_RuntimeStopNative>>('velum_client_runtime_stop')
        .asFunction<_RuntimeStopDart>();
    final destroy = library
        .lookup<NativeFunction<_RuntimeDestroyNative>>(
          'velum_client_runtime_destroy',
        )
        .asFunction<_RuntimeDestroyDart>();
    final startProxy = library
        .lookup<NativeFunction<_RuntimeProxyStartNative>>(
          'velum_client_runtime_proxy_start',
        )
        .asFunction<_RuntimeProxyStartDart>();
    final stopProxy = library
        .lookup<NativeFunction<_RuntimeProxyStopNative>>(
          'velum_client_runtime_proxy_stop',
        )
        .asFunction<_RuntimeProxyStopDart>();

    final output = calloc<Uint64>();
    try {
      _checkStatus(create(output));
      return NativeClientRuntime._(
        start: start,
        snapshot: snapshot,
        stop: stop,
        startProxy: startProxy,
        stopProxy: stopProxy,
        destroy: destroy,
        handle: output.value,
      );
    } finally {
      calloc.free(output);
    }
  }

  @override
  int start(ClientRuntimeConfiguration configuration) {
    _ensureAlive();
    final input = calloc<_VelumClientConfigInput>();
    final outputGeneration = calloc<Uint64>();
    final allocations = <_AllocatedBytes>[];
    try {
      final relayAddress = _copy(
        Uint8List.fromList(utf8.encode(configuration.relayAddress)),
        allocations,
      );
      final serverName = _copy(
        Uint8List.fromList(utf8.encode(configuration.serverName)),
        allocations,
      );
      final credential = _copy(configuration.credential, allocations);
      final certificatePem = _copy(configuration.certificatePem, allocations);
      input.ref.relayAddress
        ..pointer = relayAddress.pointer
        ..length = relayAddress.length;
      input.ref.serverName
        ..pointer = serverName.pointer
        ..length = serverName.length;
      input.ref.credential
        ..pointer = credential.pointer
        ..length = credential.length;
      input.ref.certificatePem
        ..pointer = certificatePem.pointer
        ..length = certificatePem.length;
      input.ref.connectTimeoutMillis = configuration.connectTimeoutMillis;
      input.ref.trustMode = configuration.trustMode.index;

      _checkStatus(_start(handle, input, outputGeneration));
      return outputGeneration.value;
    } finally {
      for (final allocation in allocations) {
        allocation.clearAndFree();
      }
      calloc.free(outputGeneration);
      calloc.free(input);
    }
  }

  @override
  ClientRuntimeSnapshot snapshot() {
    _ensureAlive();
    final output = calloc<VelumRuntimeSnapshotV1>();
    try {
      _checkStatus(_snapshot(handle, output));
      return ClientRuntimeSnapshot(
        revision: output.ref.revision,
        generation: output.ref.generation,
        phase: _decodePhase(output.ref.phase),
        failure: _decodeFailure(output.ref.failure),
      );
    } finally {
      calloc.free(output);
    }
  }

  @override
  int stop() {
    _ensureAlive();
    final outputGeneration = calloc<Uint64>();
    try {
      _checkStatus(_stop(handle, outputGeneration));
      return outputGeneration.value;
    } finally {
      calloc.free(outputGeneration);
    }
  }

  @override
  int startLoopbackProxy({int requestedPort = 0}) {
    _ensureAlive();
    if (requestedPort < 0 || requestedPort > 65535) {
      throw const ClientControlException(ClientControlStatus.invalidArgument);
    }
    final output = calloc<Uint16>();
    try {
      _checkStatus(_startProxy(handle, requestedPort, output));
      return output.value;
    } finally {
      calloc.free(output);
    }
  }

  @override
  void stopLoopbackProxy() {
    _ensureAlive();
    _checkStatus(_stopProxy(handle));
  }

  @override
  void destroy() {
    if (_destroyed) return;
    final status = _destroy(handle);
    if (status == ClientControlStatus.invalidHandle.index) {
      _destroyed = true;
      return;
    }
    _checkStatus(status);
    _destroyed = true;
  }

  void _ensureAlive() {
    if (_destroyed) {
      throw const ClientControlException(ClientControlStatus.invalidHandle);
    }
  }

  static _AllocatedBytes _copy(
    Uint8List value,
    List<_AllocatedBytes> allocations,
  ) {
    if (value.isEmpty) return _AllocatedBytes(Pointer.fromAddress(0), 0);
    final allocation = calloc<Uint8>(value.length);
    allocation.asTypedList(value.length).setAll(0, value);
    final copied = _AllocatedBytes(allocation, value.length);
    allocations.add(copied);
    return copied;
  }
}

ClientRuntimePhase _decodePhase(int value) {
  if (value < 0 || value >= ClientRuntimePhase.values.length) {
    throw const ClientControlException(ClientControlStatus.internal);
  }
  return ClientRuntimePhase.values[value];
}

ClientRuntimeFailure _decodeFailure(int value) {
  if (value < 0 || value >= ClientRuntimeFailure.values.length) {
    throw const ClientControlException(ClientControlStatus.internal);
  }
  return ClientRuntimeFailure.values[value];
}

void _checkStatus(int value) {
  if (value == ClientControlStatus.ok.index) return;
  if (value < 0 || value >= ClientControlStatus.values.length) {
    throw const ClientControlException(ClientControlStatus.internal);
  }
  throw ClientControlException(ClientControlStatus.values[value]);
}

class _AllocatedBytes {
  const _AllocatedBytes(this.pointer, this.length);

  final Pointer<Uint8> pointer;
  final int length;

  void clearAndFree() {
    if (length == 0) return;
    pointer.asTypedList(length).fillRange(0, length, 0);
    calloc.free(pointer);
  }
}
