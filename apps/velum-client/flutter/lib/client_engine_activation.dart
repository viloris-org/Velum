import 'dart:io';
import 'dart:typed_data';

import 'client_configuration.dart';
import 'client_profile.dart';
import 'native_client.dart';
import 'native_engine.dart';

/// One fully resolved, short-lived input for native multi-node activation.
final class ClientEngineActivation {
  ClientEngineActivation({
    required this.libraryPath,
    required this.defaultNode,
    required this.nodes,
  });

  final String libraryPath;
  final String defaultNode;
  final List<ClientEngineNodeConfiguration> nodes;

  void clearSecrets() {
    for (final node in nodes) {
      node.clearSecrets();
    }
  }
}

/// Resolves UI-managed secret references immediately before native activation.
final class ClientEngineActivationResolver {
  ClientEngineActivationResolver(this._secretStore);

  final ClientSecretStore _secretStore;

  Future<ClientEngineActivation> resolve({
    required String libraryPath,
    required List<RelayNodeDraft> nodes,
    required String defaultNode,
  }) async {
    if (nodes.isEmpty || defaultNode.trim().isEmpty) {
      throw const FormatException('A default node is required.');
    }
    final resolved = <ClientEngineNodeConfiguration>[];
    try {
      for (final node in nodes) {
        if (!node.isComplete || node.id.text.trim().isEmpty) {
          throw const FormatException('Every engine node must be complete.');
        }
        final credential = await _credential(node);
        final certificate = await _certificate(node);
        resolved.add(
          ClientEngineNodeConfiguration(
            id: node.id.text.trim(),
            alias: node.name.text.trim(),
            relayAddress: node.relayAddress.text.trim(),
            serverName: node.serverName.text.trim(),
            credential: credential,
            trustMode: node.trustMode,
            certificatePem: certificate,
          ),
        );
      }
      return ClientEngineActivation(
        libraryPath: libraryPath,
        defaultNode: defaultNode.trim(),
        nodes: resolved,
      );
    } on Object {
      for (final node in resolved) {
        node.clearSecrets();
      }
      rethrow;
    }
  }

  Future<Uint8List> _credential(RelayNodeDraft node) async {
    final reference = node.credentialRef.text.trim();
    if (reference.isNotEmpty) {
      return _secretStore.credential(
        reference,
        migrationFile: node.credentialPath.text,
      );
    }
    return decodeCredentialHex(
      await File(node.credentialPath.text.trim()).readAsString(),
    );
  }

  Future<Uint8List> _certificate(RelayNodeDraft node) async {
    if (node.trustMode != ClientTrustMode.customCa) return Uint8List(0);
    final reference = node.certificateRef.text.trim();
    if (reference.isNotEmpty) {
      return _secretStore.certificate(
        reference,
        migrationFile: node.certificatePath.text,
      );
    }
    return File(node.certificatePath.text.trim()).readAsBytes();
  }
}
