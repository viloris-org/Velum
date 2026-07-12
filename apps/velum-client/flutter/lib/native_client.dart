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
}

typedef _AbiVersionNative = Uint16 Function();
typedef _AbiVersionDart = int Function();
typedef _ConnectNative = Int32 Function(
  Pointer<_VelumClientConfigInput>,
  Pointer<Uint64>,
);
typedef _ConnectDart = int Function(
  Pointer<_VelumClientConfigInput>,
  Pointer<Uint64>,
);
typedef _CloseNative = Int32 Function(Uint64);
typedef _CloseDart = int Function(int);

class DirectClientConfiguration {
  const DirectClientConfiguration({
    required this.libraryPath,
    required this.relayAddress,
    required this.serverName,
    required this.credential,
    required this.certificatePem,
    this.connectTimeoutMillis = 5000,
  });

  final String libraryPath;
  final String relayAddress;
  final String serverName;
  final Uint8List credential;
  final Uint8List certificatePem;
  final int connectTimeoutMillis;
}

class DirectClientException implements Exception {
  const DirectClientException(this.status);

  final int status;

  @override
  String toString() => switch (status) {
    1 => 'The native client received invalid input.',
    2 => 'The native client handle is no longer valid.',
    3 => 'The client configuration was rejected.',
    4 => 'The relay certificate could not be loaded.',
    5 => 'Connecting to the relay timed out.',
    6 => 'The relay connection failed.',
    7 => 'The requested stream control record is too large.',
    8 => 'A transport error occurred.',
    9 => 'The datagram is too large for the active path.',
    10 => 'The active relay connection does not support datagrams.',
    11 => 'The relay returned an invalid protocol message.',
    _ => 'The native client returned status $status.',
  };
}

class DirectClient {
  DirectClient._(this._close, this.handle);

  static const int _abiVersion = 1;

  final _CloseDart _close;
  final int handle;

  static String defaultLibraryName() {
    if (Platform.isMacOS) return 'libvelum_client_ffi.dylib';
    if (Platform.isWindows) return 'velum_client_ffi.dll';
    return 'libvelum_client_ffi.so';
  }

  static DirectClient connect(DirectClientConfiguration configuration) {
    final library = DynamicLibrary.open(configuration.libraryPath);
    final abiVersion = library
        .lookup<NativeFunction<_AbiVersionNative>>('velum_client_abi_version')
        .asFunction<_AbiVersionDart>();
    if (abiVersion() != _abiVersion) {
      throw const DirectClientException(3);
    }
    final connect = library
        .lookup<NativeFunction<_ConnectNative>>('velum_client_connect')
        .asFunction<_ConnectDart>();
    final close = library
        .lookup<NativeFunction<_CloseNative>>('velum_client_close')
        .asFunction<_CloseDart>();
    final input = calloc<_VelumClientConfigInput>();
    final output = calloc<Uint64>();
    final allocations = <_AllocatedBytes>[];
    try {
      final relayAddress = _copy(
        Uint8List.fromList(configuration.relayAddress.codeUnits),
        allocations,
      );
      final serverName = _copy(
        Uint8List.fromList(configuration.serverName.codeUnits),
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
      final status = connect(input, output);
      if (status != 0) throw DirectClientException(status);
      return DirectClient._(close, output.value);
    } finally {
      for (final allocation in allocations) {
        calloc.free(allocation.pointer);
      }
      calloc.free(output);
      calloc.free(input);
    }
  }

  static DirectClient attach(String libraryPath, int handle) {
    final close = DynamicLibrary.open(libraryPath)
        .lookup<NativeFunction<_CloseNative>>('velum_client_close')
        .asFunction<_CloseDart>();
    return DirectClient._(close, handle);
  }

  void close() {
    final status = _close(handle);
    if (status != 0 && status != 2) throw DirectClientException(status);
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

class _AllocatedBytes {
  const _AllocatedBytes(this.pointer, this.length);

  final Pointer<Uint8> pointer;
  final int length;
}
