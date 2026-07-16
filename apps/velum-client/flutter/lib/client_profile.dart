import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:yaml/yaml.dart';

final class ManagedProfileNode {
  const ManagedProfileNode({
    required this.id,
    required this.alias,
    required this.relayAddress,
    required this.serverName,
    required this.credentialRef,
    required this.trustMode,
    this.caRef,
  });

  final String id;
  final String alias;
  final String relayAddress;
  final String serverName;
  final String credentialRef;
  final String trustMode;
  final String? caRef;
}

/// Redacted UI projection of a profile already validated by native ABI v3.
final class ManagedClientProfile {
  const ManagedClientProfile({
    required this.id,
    required this.name,
    required this.defaultNode,
    required this.nodes,
    required this.canonicalYaml,
  });

  factory ManagedClientProfile.parseCanonical(String source) {
    final root = _map(loadYaml(source), 'profile document');
    final metadata = _map(root['profile'], 'profile');
    final rawNodes = root['nodes'];
    if (rawNodes is! YamlList) throw const FormatException('nodes is invalid');
    final nodes = rawNodes
        .map((value) {
          final node = _map(value, 'node');
          final trust = _map(node['trust'], 'trust');
          return ManagedProfileNode(
            id: _string(node['id'], 'node id'),
            alias: (node['alias'] as String?) ?? _string(node['id'], 'node id'),
            relayAddress: _string(node['relay-address'], 'relay address'),
            serverName: _string(node['server-name'], 'server name'),
            credentialRef: _string(
              node['credential-ref'],
              'credential reference',
            ),
            trustMode: _string(trust['mode'], 'trust mode'),
            caRef: trust['ca-ref'] as String?,
          );
        })
        .toList(growable: false);
    return ManagedClientProfile(
      id: _string(metadata['id'], 'profile id'),
      name: _string(metadata['name'], 'profile name'),
      defaultNode: _string(metadata['default-node'], 'default node'),
      nodes: nodes,
      canonicalYaml: source,
    );
  }

  final String id;
  final String name;
  final String defaultNode;
  final List<ManagedProfileNode> nodes;
  final String canonicalYaml;
}

/// Application-owned canonical profile copy. Source files are never watched.
final class ManagedProfileRepository {
  ManagedProfileRepository(this.file);

  factory ManagedProfileRepository.defaultForPlatform() {
    final home = Platform.environment['HOME'];
    final appData = Platform.environment['APPDATA'];
    final state = Platform.isWindows && appData != null
        ? appData
        : home == null
        ? Directory.systemTemp.path
        : Platform.isMacOS
        ? '$home/Library/Application Support'
        : Platform.environment['XDG_STATE_HOME'] ?? '$home/.local/state';
    return ManagedProfileRepository(File('$state/Velum/profile.yaml'));
  }

  final File file;

  Future<ManagedClientProfile> importFile(
    String sourcePath,
    String Function(String source) normalize,
  ) async {
    final source = await File(sourcePath).readAsString();
    final canonical = normalize(source);
    final profile = ManagedClientProfile.parseCanonical(canonical);
    await _commit(canonical);
    return profile;
  }

  Future<ManagedClientProfile?> load() async {
    if (!await file.exists()) return null;
    return ManagedClientProfile.parseCanonical(await file.readAsString());
  }

  Future<void> _commit(String canonical) async {
    await file.parent.create(recursive: true);
    final temporary = File('${file.path}.tmp');
    final previous = File('${file.path}.previous');
    await temporary.writeAsString(canonical, flush: true);
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

/// Opaque secret reference resolver backed by each platform's secure storage.
final class ClientSecretStore {
  ClientSecretStore({FlutterSecureStorage? storage})
    : _storage = storage ?? const FlutterSecureStorage();

  final FlutterSecureStorage _storage;

  Future<void> installEnrollment({
    required String credentialReference,
    required Uint8List credential,
    String? certificateReference,
    Uint8List? certificate,
  }) async {
    _validateReference(credentialReference);
    if ((certificateReference == null) != (certificate == null)) {
      throw const FormatException('Enrollment certificate is incomplete.');
    }
    if (certificateReference != null) _validateReference(certificateReference);
    if (await _storage.read(key: credentialReference) != null) {
      throw StateError('This client enrollment is already installed.');
    }
    if (certificateReference != null &&
        await _storage.read(key: certificateReference) != null) {
      throw StateError('This client enrollment CA is already installed.');
    }
    await _storage.write(
      key: credentialReference,
      value: _encodeHex(credential),
    );
    try {
      if (certificateReference != null && certificate != null) {
        await _storage.write(
          key: certificateReference,
          value: base64Encode(certificate),
        );
      }
    } on Object {
      await _storage.delete(key: credentialReference);
      rethrow;
    }
  }

  Future<void> removeEnrollment({
    required String credentialReference,
    String? certificateReference,
  }) async {
    _validateReference(credentialReference);
    if (certificateReference != null) _validateReference(certificateReference);
    await _storage.delete(key: credentialReference);
    if (certificateReference != null) {
      await _storage.delete(key: certificateReference);
    }
  }

  Future<Uint8List> credential(
    String reference, {
    String? migrationFile,
  }) async {
    _validateReference(reference);
    var value = await _storage.read(key: reference);
    if (value == null &&
        migrationFile != null &&
        migrationFile.trim().isNotEmpty) {
      value = (await File(migrationFile.trim()).readAsString()).trim();
      final decoded = _decodeHex(value);
      await _storage.write(key: reference, value: _encodeHex(decoded));
      return decoded;
    }
    if (value == null) {
      throw StateError('Credential is missing from secure storage.');
    }
    return _decodeHex(value);
  }

  Future<Uint8List> certificate(
    String reference, {
    String? migrationFile,
  }) async {
    _validateReference(reference);
    var value = await _storage.read(key: reference);
    if (value == null &&
        migrationFile != null &&
        migrationFile.trim().isNotEmpty) {
      final bytes = await File(migrationFile.trim()).readAsBytes();
      value = base64Encode(bytes);
      await _storage.write(key: reference, value: value);
      return bytes;
    }
    if (value == null) {
      throw StateError('CA certificate is missing from secure storage.');
    }
    return Uint8List.fromList(base64Decode(value));
  }

  static void _validateReference(String value) {
    if (!value.startsWith('secret://velum/')) {
      throw const FormatException('Invalid Velum secret reference.');
    }
  }
}

YamlMap _map(Object? value, String label) {
  if (value is! YamlMap) throw FormatException('$label is invalid');
  return value;
}

String _string(Object? value, String label) {
  if (value is! String || value.isEmpty) {
    throw FormatException('$label is invalid');
  }
  return value;
}

Uint8List _decodeHex(String value) {
  final source = value.trim();
  if (source.isEmpty || source.length.isOdd) {
    throw const FormatException(
      'Credential must contain hexadecimal byte pairs.',
    );
  }
  final bytes = Uint8List(source.length ~/ 2);
  for (var index = 0; index < bytes.length; index++) {
    final byte = int.tryParse(
      source.substring(index * 2, index * 2 + 2),
      radix: 16,
    );
    if (byte == null) {
      throw const FormatException(
        'Credential must contain hexadecimal byte pairs.',
      );
    }
    bytes[index] = byte;
  }
  return bytes;
}

String _encodeHex(Uint8List value) =>
    value.map((byte) => byte.toRadixString(16).padLeft(2, '0')).join();

Uint8List decodeCredentialHex(String value) => _decodeHex(value);
