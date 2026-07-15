import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'native_client.dart';

final class ClientEnrollment {
  const ClientEnrollment({
    required this.nodeId,
    required this.nodeName,
    required this.relayAddress,
    required this.serverName,
    required this.principalId,
    required this.credential,
    required this.trustMode,
    this.certificatePem,
  });

  factory ClientEnrollment.parseCanonical(String source) {
    final decoded = jsonDecode(source);
    if (decoded is! Map<String, Object?>) {
      throw const FormatException('Enrollment document is invalid.');
    }
    _requireKeys(decoded, const {
      'kind',
      'version',
      'node',
      'principal-id',
      'credential',
      'trust',
    });
    if (decoded['kind'] != 'velum-enrollment' || decoded['version'] != 1) {
      throw const FormatException('Enrollment type or version is invalid.');
    }
    final node = _map(decoded['node'], 'node');
    _requireKeys(node, const {'id', 'name', 'relay-address', 'server-name'});
    final trust = _map(decoded['trust'], 'trust');
    final trustMode = _string(trust['mode'], 'trust mode');
    switch (trustMode) {
      case 'system':
        _requireKeys(trust, const {'mode'});
      case 'custom-ca':
        _requireKeys(trust, const {'mode', 'certificate-pem'});
      default:
        throw const FormatException('Enrollment trust mode is invalid.');
    }
    final principalId = decoded['principal-id'];
    if (principalId is! int || principalId < 0) {
      throw const FormatException('Enrollment principal is invalid.');
    }
    final credential = _decodeHex(_string(decoded['credential'], 'credential'));
    if (credential.length != 32) {
      throw const FormatException('Enrollment credential must be 32 bytes.');
    }
    return ClientEnrollment(
      nodeId: _string(node['id'], 'node id'),
      nodeName: _string(node['name'], 'node name'),
      relayAddress: _string(node['relay-address'], 'relay address'),
      serverName: _string(node['server-name'], 'server name'),
      principalId: principalId,
      credential: credential,
      trustMode: trustMode,
      certificatePem: trustMode == 'custom-ca'
          ? _string(trust['certificate-pem'], 'CA certificate')
          : null,
    );
  }

  final String nodeId;
  final String nodeName;
  final String relayAddress;
  final String serverName;
  final int principalId;
  final Uint8List credential;
  final String trustMode;
  final String? certificatePem;

  String get credentialRef => 'secret://velum/enrollment/$nodeId/$principalId';

  String? get certificateRef => certificatePem == null
      ? null
      : 'secret://velum/enrollment/$nodeId/$principalId/ca';

  InstalledEnrollmentNode get installedNode => InstalledEnrollmentNode(
    nodeId: nodeId,
    nodeName: nodeName,
    relayAddress: relayAddress,
    serverName: serverName,
    credentialRef: credentialRef,
    trustMode: trustMode,
    certificateRef: certificateRef,
  );
}

final class InstalledEnrollmentNode {
  const InstalledEnrollmentNode({
    required this.nodeId,
    required this.nodeName,
    required this.relayAddress,
    required this.serverName,
    required this.credentialRef,
    required this.trustMode,
    required this.certificateRef,
  });

  factory InstalledEnrollmentNode.fromJson(Object? source) {
    final value = _map(source, 'installed node');
    _requireKeys(value, const {
      'node-id',
      'node-name',
      'relay-address',
      'server-name',
      'credential-ref',
      'trust-mode',
      'certificate-ref',
    });
    final certificateRef = value['certificate-ref'];
    if (certificateRef != null && certificateRef is! String) {
      throw const FormatException(
        'Installed certificate reference is invalid.',
      );
    }
    final node = InstalledEnrollmentNode(
      nodeId: _string(value['node-id'], 'installed node id'),
      nodeName: _string(value['node-name'], 'installed node name'),
      relayAddress: _string(value['relay-address'], 'installed relay address'),
      serverName: _string(value['server-name'], 'installed server name'),
      credentialRef: _string(
        value['credential-ref'],
        'installed credential reference',
      ),
      trustMode: _string(value['trust-mode'], 'installed trust mode'),
      certificateRef: certificateRef as String?,
    );
    if (!node.credentialRef.startsWith('secret://velum/') ||
        (node.certificateRef != null &&
            !node.certificateRef!.startsWith('secret://velum/')) ||
        !const {'system', 'custom-ca'}.contains(node.trustMode) ||
        (node.trustMode == 'custom-ca') != (node.certificateRef != null)) {
      throw const FormatException(
        'Installed enrollment references are invalid.',
      );
    }
    return node;
  }

  final String nodeId;
  final String nodeName;
  final String relayAddress;
  final String serverName;
  final String credentialRef;
  final String trustMode;
  final String? certificateRef;

  Map<String, Object?> toJson() => {
    'node-id': nodeId,
    'node-name': nodeName,
    'relay-address': relayAddress,
    'server-name': serverName,
    'credential-ref': credentialRef,
    'trust-mode': trustMode,
    'certificate-ref': certificateRef,
  };
}

/// Redacted discovery index for enrollments already moved into secure storage.
final class EnrollmentRepository {
  EnrollmentRepository(this.file);

  factory EnrollmentRepository.defaultForPlatform() {
    final home = Platform.environment['HOME'];
    final appData = Platform.environment['APPDATA'];
    final state = Platform.isWindows && appData != null
        ? appData
        : home == null
        ? Directory.systemTemp.path
        : Platform.isMacOS
        ? '$home/Library/Application Support'
        : Platform.environment['XDG_STATE_HOME'] ?? '$home/.local/state';
    return EnrollmentRepository(File('$state/Velum/enrollments.json'));
  }

  final File file;

  Future<List<InstalledEnrollmentNode>> load() async {
    if (!await file.exists()) return const [];
    final source = await file.readAsString();
    if (source.length > 1024 * 1024) {
      throw const FormatException('Installed enrollment index is too large.');
    }
    final root = jsonDecode(source);
    if (root is! Map<String, Object?>) {
      throw const FormatException('Installed enrollment index is invalid.');
    }
    _requireKeys(root, const {'version', 'nodes'});
    if (root['version'] != 1 || root['nodes'] is! List<Object?>) {
      throw const FormatException('Installed enrollment index is invalid.');
    }
    final nodes = (root['nodes']! as List<Object?>)
        .map(InstalledEnrollmentNode.fromJson)
        .toList(growable: false);
    if (nodes.length > 128 ||
        nodes.map((node) => node.nodeId).toSet().length != nodes.length) {
      throw const FormatException('Installed enrollment node set is invalid.');
    }
    return nodes;
  }

  Future<void> upsert(InstalledEnrollmentNode node) async {
    final nodes = (await load()).toList();
    nodes.removeWhere((candidate) => candidate.nodeId == node.nodeId);
    if (nodes.length >= 128) {
      throw const FormatException('Installed enrollment node limit reached.');
    }
    nodes.add(node);
    await _commit(nodes);
  }

  Future<void> remove(String nodeId) async {
    final nodes = (await load()).toList();
    final originalLength = nodes.length;
    nodes.removeWhere((candidate) => candidate.nodeId == nodeId);
    if (nodes.length == originalLength) return;
    await _commit(nodes);
  }

  Future<void> _commit(List<InstalledEnrollmentNode> nodes) async {
    await file.parent.create(recursive: true);
    final temporary = File('${file.path}.tmp');
    final previous = File('${file.path}.previous');
    await temporary.writeAsString(
      '${jsonEncode({'version': 1, 'nodes': nodes})}\n',
      flush: true,
    );
    if (await previous.exists()) await previous.delete();
    if (await file.exists()) await file.rename(previous.path);
    try {
      await temporary.rename(file.path);
      if (await previous.exists()) await previous.delete();
    } on Object {
      if (await previous.exists() && !await file.exists()) {
        await previous.rename(file.path);
      }
      rethrow;
    }
  }
}

/// Native authority for the bounded, versioned enrollment JSON contract.
final class NativeEnrollmentCodec {
  NativeEnrollmentCodec._(this._validate, this._normalize);

  static const _abiVersion = 1;

  final _EnrollmentValidateDart _validate;
  final _EnrollmentNormalizeDart _normalize;

  factory NativeEnrollmentCodec.open(String libraryPath) {
    final library = DynamicLibrary.open(libraryPath);
    final version = library
        .lookup<NativeFunction<_EnrollmentAbiVersionNative>>(
          'velum_client_enrollment_abi_version',
        )
        .asFunction<_EnrollmentAbiVersionDart>();
    if (version() != _abiVersion) {
      throw const ClientProfileException(
        ClientProfileStatus.unsupportedVersion,
      );
    }
    return NativeEnrollmentCodec._(
      library
          .lookup<NativeFunction<_EnrollmentValidateNative>>(
            'velum_client_enrollment_validate_v1',
          )
          .asFunction<_EnrollmentValidateDart>(),
      library
          .lookup<NativeFunction<_EnrollmentNormalizeNative>>(
            'velum_client_enrollment_normalize_v1',
          )
          .asFunction<_EnrollmentNormalizeDart>(),
    );
  }

  String normalize(String source) {
    final sourceBytes = Uint8List.fromList(utf8.encode(source));
    final input = calloc<Uint8>(sourceBytes.length);
    final inputSlice = calloc<_EnrollmentByteSlice>();
    final required = calloc<Size>();
    try {
      input.asTypedList(sourceBytes.length).setAll(0, sourceBytes);
      inputSlice.ref
        ..pointer = input
        ..length = sourceBytes.length;
      _checkStatus(_validate(inputSlice.ref, required));
      final output = calloc<Uint8>(required.value);
      final outputSlice = calloc<_EnrollmentMutableByteSlice>();
      try {
        outputSlice.ref
          ..pointer = output
          ..length = required.value;
        _checkStatus(_normalize(inputSlice.ref, outputSlice.ref, required));
        return utf8.decode(output.asTypedList(required.value));
      } finally {
        output.asTypedList(required.value).fillRange(0, required.value, 0);
        calloc.free(outputSlice);
        calloc.free(output);
      }
    } finally {
      input.asTypedList(sourceBytes.length).fillRange(0, sourceBytes.length, 0);
      sourceBytes.fillRange(0, sourceBytes.length, 0);
      calloc.free(required);
      calloc.free(inputSlice);
      calloc.free(input);
    }
  }
}

final class _EnrollmentByteSlice extends Struct {
  external Pointer<Uint8> pointer;

  @Size()
  external int length;
}

final class _EnrollmentMutableByteSlice extends Struct {
  external Pointer<Uint8> pointer;

  @Size()
  external int length;
}

typedef _EnrollmentAbiVersionNative = Uint16 Function();
typedef _EnrollmentAbiVersionDart = int Function();
typedef _EnrollmentValidateNative =
    Int32 Function(_EnrollmentByteSlice, Pointer<Size>);
typedef _EnrollmentValidateDart =
    int Function(_EnrollmentByteSlice, Pointer<Size>);
typedef _EnrollmentNormalizeNative =
    Int32 Function(
      _EnrollmentByteSlice,
      _EnrollmentMutableByteSlice,
      Pointer<Size>,
    );
typedef _EnrollmentNormalizeDart =
    int Function(
      _EnrollmentByteSlice,
      _EnrollmentMutableByteSlice,
      Pointer<Size>,
    );

void _checkStatus(int value) {
  if (value == ClientProfileStatus.ok.index) return;
  if (value < 0 || value >= ClientProfileStatus.values.length) {
    throw const ClientProfileException(ClientProfileStatus.internal);
  }
  throw ClientProfileException(ClientProfileStatus.values[value]);
}

Map<String, Object?> _map(Object? value, String label) {
  if (value is! Map<String, Object?>) {
    throw FormatException('Enrollment $label is invalid.');
  }
  return value;
}

String _string(Object? value, String label) {
  if (value is! String || value.isEmpty) {
    throw FormatException('Enrollment $label is invalid.');
  }
  return value;
}

void _requireKeys(Map<String, Object?> value, Set<String> expected) {
  if (value.keys.toSet().difference(expected).isNotEmpty ||
      expected.difference(value.keys.toSet()).isNotEmpty) {
    throw const FormatException('Enrollment fields are invalid.');
  }
}

Uint8List _decodeHex(String value) {
  if (value.isEmpty || value.length.isOdd) {
    throw const FormatException('Enrollment credential is invalid.');
  }
  final bytes = Uint8List(value.length ~/ 2);
  for (var index = 0; index < bytes.length; index += 1) {
    final byte = int.tryParse(
      value.substring(index * 2, index * 2 + 2),
      radix: 16,
    );
    if (byte == null) {
      throw const FormatException('Enrollment credential is invalid.');
    }
    bytes[index] = byte;
  }
  return bytes;
}
