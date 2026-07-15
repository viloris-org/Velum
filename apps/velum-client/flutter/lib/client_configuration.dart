import 'package:flutter/material.dart';

import 'client_theme.dart';
import 'native_client.dart';

class RelayNodeDraft {
  RelayNodeDraft({
    String id = '',
    String name = '',
    String relayAddress = '',
    String serverName = '',
    String certificatePath = '',
    String credentialPath = '',
    String credentialRef = '',
    String certificateRef = '',
    this.trustMode = ClientTrustMode.system,
  }) : id = TextEditingController(text: id),
       name = TextEditingController(text: name),
       relayAddress = TextEditingController(text: relayAddress),
       serverName = TextEditingController(text: serverName),
       certificatePath = TextEditingController(text: certificatePath),
       credentialPath = TextEditingController(text: credentialPath),
       credentialRef = TextEditingController(text: credentialRef),
       certificateRef = TextEditingController(text: certificateRef);

  final TextEditingController id;
  final TextEditingController name;
  final TextEditingController relayAddress;
  final TextEditingController serverName;
  final TextEditingController certificatePath;
  final TextEditingController credentialPath;
  final TextEditingController credentialRef;
  final TextEditingController certificateRef;
  ClientTrustMode trustMode;

  bool get isComplete => [
    name.text,
    relayAddress.text,
    serverName.text,
    credentialRef.text.isNotEmpty ? credentialRef.text : credentialPath.text,
    if (trustMode == ClientTrustMode.customCa)
      certificateRef.text.isNotEmpty
          ? certificateRef.text
          : certificatePath.text,
  ].every((value) => value.trim().isNotEmpty);

  void dispose() {
    id.dispose();
    name.dispose();
    relayAddress.dispose();
    serverName.dispose();
    certificatePath.dispose();
    credentialPath.dispose();
    credentialRef.dispose();
    certificateRef.dispose();
  }
}

class ClientConfigurationPanel extends StatelessWidget {
  const ClientConfigurationPanel({
    required this.formKey,
    required this.snapshot,
    required this.nodes,
    required this.activeNodeIndex,
    required this.onAddNode,
    required this.onRemoveNode,
    required this.onSelectNode,
    required this.onTrustModeChanged,
    required this.profileFile,
    required this.onImportProfile,
    required this.onImportEnrollment,
    required this.onScanEnrollment,
    required this.canScanEnrollment,
    super.key,
  });

  final GlobalKey<FormState> formKey;
  final ClientRuntimeSnapshot snapshot;
  final List<RelayNodeDraft> nodes;
  final int activeNodeIndex;
  final VoidCallback onAddNode;
  final ValueChanged<int> onRemoveNode;
  final ValueChanged<int> onSelectNode;
  final void Function(RelayNodeDraft, ClientTrustMode) onTrustModeChanged;
  final TextEditingController profileFile;
  final Future<void> Function() onImportProfile;
  final Future<void> Function() onImportEnrollment;
  final Future<void> Function() onScanEnrollment;
  final bool canScanEnrollment;

  bool get _isEditable => const {
    ClientRuntimePhase.stopped,
    ClientRuntimePhase.failed,
  }.contains(snapshot.phase);

  @override
  Widget build(BuildContext context) {
    return Form(
      key: formKey,
      child: ListView(
        children: [
          const SectionLabel('Config'),
          const SizedBox(height: 12),
          const Text(
            'Connection configuration',
            style: TextStyle(fontSize: 21, fontWeight: FontWeight.w700),
          ),
          const SizedBox(height: 6),
          const Text(
            'Update the relay details before connecting. Sensitive values remain local to this device.',
          ),
          const SizedBox(height: 20),
          ClientPanel(
            child: Column(
              children: [
                Align(
                  alignment: Alignment.centerLeft,
                  child: Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: [
                      FilledButton.icon(
                        key: const ValueKey('import-enrollment'),
                        onPressed: _isEditable ? onImportEnrollment : null,
                        icon: const Icon(Icons.key_rounded),
                        label: const Text('Import enrollment'),
                      ),
                      if (canScanEnrollment)
                        OutlinedButton.icon(
                          key: const ValueKey('scan-enrollment'),
                          onPressed: _isEditable ? onScanEnrollment : null,
                          icon: const Icon(Icons.qr_code_scanner_rounded),
                          label: const Text('Scan QR'),
                        ),
                    ],
                  ),
                ),
                const SizedBox(height: 20),
                _field(
                  profileFile,
                  'Velum profile YAML',
                  'Native-validated profile imported into application-managed storage',
                ),
                Align(
                  alignment: Alignment.centerLeft,
                  child: OutlinedButton.icon(
                    key: const ValueKey('import-profile'),
                    onPressed: _isEditable ? onImportProfile : null,
                    icon: const Icon(Icons.file_open_outlined),
                    label: const Text('Import profile'),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          for (var index = 0; index < nodes.length; index++) ...[
            _NodeEditor(
              key: ValueKey(nodes[index]),
              node: nodes[index],
              index: index,
              isActive: index == activeNodeIndex,
              editable: _isEditable,
              removable: nodes.length > 1,
              onSelect: () => onSelectNode(index),
              onRemove: () => onRemoveNode(index),
              onTrustModeChanged: (mode) =>
                  onTrustModeChanged(nodes[index], mode),
            ),
            const SizedBox(height: 12),
          ],
          OutlinedButton.icon(
            key: const ValueKey('add-node'),
            onPressed: _isEditable ? onAddNode : null,
            icon: const Icon(Icons.add_outlined),
            label: const Text('Add node'),
          ),
          const Text(
            'Experimental Stage 2 direct client API. It is not a production VPN and supports IP-address targets only.',
            style: TextStyle(color: ClientTheme.warning, fontSize: 12),
          ),
        ],
      ),
    );
  }

  Widget _field(TextEditingController controller, String label, String helper) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 16),
      child: TextFormField(
        controller: controller,
        enabled: _isEditable,
        decoration: InputDecoration(
          labelText: label,
          helperText: helper,
          border: const OutlineInputBorder(),
        ),
        validator: (value) => value == null || value.trim().isEmpty
            ? '$label is required.'
            : null,
      ),
    );
  }
}

class _NodeEditor extends StatelessWidget {
  const _NodeEditor({
    required this.node,
    required this.index,
    required this.isActive,
    required this.editable,
    required this.removable,
    required this.onSelect,
    required this.onRemove,
    required this.onTrustModeChanged,
    super.key,
  });

  final RelayNodeDraft node;
  final int index;
  final bool isActive;
  final bool editable;
  final bool removable;
  final VoidCallback onSelect;
  final VoidCallback onRemove;
  final ValueChanged<ClientTrustMode> onTrustModeChanged;

  @override
  Widget build(BuildContext context) => ClientPanel(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                'Node ${index + 1}',
                style: const TextStyle(fontWeight: FontWeight.w700),
              ),
            ),
            Tooltip(
              message: isActive ? 'Active connection node' : 'Use this node',
              child: IconButton(
                onPressed: editable ? onSelect : null,
                icon: Icon(
                  isActive
                      ? Icons.radio_button_checked_outlined
                      : Icons.radio_button_unchecked_outlined,
                ),
                color: isActive ? ClientTheme.accent : ClientTheme.muted,
              ),
            ),
            if (removable)
              Tooltip(
                message: 'Remove node',
                child: IconButton(
                  onPressed: editable ? onRemove : null,
                  icon: const Icon(Icons.delete_outline),
                  color: ClientTheme.danger,
                ),
              ),
          ],
        ),
        const SizedBox(height: 12),
        _nodeField(node.id, 'Node ID', 'Stable profile identifier'),
        _nodeField(node.name, 'Node name', 'A local name for this relay'),
        _nodeField(
          node.relayAddress,
          'Relay address',
          'IP address and UDP port, for example 203.0.113.10:4433',
        ),
        _nodeField(
          node.serverName,
          'TLS server name',
          'Certificate name presented by the relay',
        ),
        DropdownButtonFormField<ClientTrustMode>(
          initialValue: node.trustMode,
          onChanged: editable
              ? (mode) {
                  if (mode != null) onTrustModeChanged(mode);
                }
              : null,
          decoration: const InputDecoration(
            labelText: 'Certificate verification',
            border: OutlineInputBorder(),
          ),
          items: const [
            DropdownMenuItem(
              value: ClientTrustMode.system,
              child: Text('Use system trust store'),
            ),
            DropdownMenuItem(
              value: ClientTrustMode.customCa,
              child: Text('Use custom CA certificate'),
            ),
            DropdownMenuItem(
              value: ClientTrustMode.insecure,
              child: Text('Allow insecure connection'),
            ),
          ],
        ),
        const SizedBox(height: 16),
        if (node.trustMode == ClientTrustMode.customCa)
          _nodeField(
            node.certificateRef,
            'CA secret reference',
            'secret://velum/... reference in platform secure storage',
            required: false,
          ),
        if (node.trustMode == ClientTrustMode.customCa)
          _nodeField(
            node.certificatePath,
            'CA certificate file',
            'PEM file used to verify the relay',
            required: false,
          ),
        _nodeField(
          node.credentialRef,
          'Credential secret reference',
          'secret://velum/... reference in platform secure storage',
          required: false,
        ),
        _nodeField(
          node.credentialPath,
          'Credential file',
          'Hexadecimal credential supplied by the operator',
          required: false,
        ),
      ],
    ),
  );

  Widget _nodeField(
    TextEditingController controller,
    String label,
    String helper, {
    bool required = true,
  }) => Padding(
    padding: const EdgeInsets.only(bottom: 16),
    child: TextFormField(
      controller: controller,
      enabled: editable,
      decoration: InputDecoration(
        labelText: label,
        helperText: helper,
        border: const OutlineInputBorder(),
      ),
      validator: (value) => required && (value == null || value.trim().isEmpty)
          ? '$label is required.'
          : null,
    ),
  );
}
