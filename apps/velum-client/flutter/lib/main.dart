import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';

import 'client_configuration.dart';
import 'client_compact_navigation.dart';
import 'client_controller.dart';
import 'client_enrollment.dart';
import 'client_overview.dart';
import 'client_profile.dart';
import 'client_settings.dart';
import 'client_theme.dart';
import 'enrollment_scanner.dart';
import 'native_client.dart';
import 'traffic_configuration.dart';
import 'traffic_mode_controller.dart';

void main() {
  runApp(const VelumClientApp());
}

class VelumClientApp extends StatelessWidget {
  const VelumClientApp({super.key, this.controller});

  final ClientController? controller;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Velum',
      theme: ClientTheme.data(),
      home: ClientHome(controller: controller),
    );
  }
}

class ClientHome extends StatefulWidget {
  const ClientHome({super.key, this.controller});

  final ClientController? controller;

  @override
  State<ClientHome> createState() => _ClientHomeState();
}

class _ClientHomeState extends State<ClientHome> {
  final _formKey = GlobalKey<FormState>();
  final _profileFile = TextEditingController();
  final _nodes = <RelayNodeDraft>[
    RelayNodeDraft(
      id: 'local-relay',
      name: 'Local relay',
      relayAddress: '127.0.0.1:4433',
      serverName: 'localhost',
    ),
  ];
  final _trafficConfiguration = TrafficConfigurationDraft();
  final _profileRepository = ManagedProfileRepository.defaultForPlatform();
  final _enrollmentRepository = EnrollmentRepository.defaultForPlatform();
  final _secretStore = ClientSecretStore();

  late final ClientController _clientController;
  late final TrafficModeController _trafficController;
  late ClientRuntimeSnapshot _lastSnapshot;
  Object? _lastPollingError;
  ClientReconnectStatus _lastReconnectStatus =
      const ClientReconnectStatus.inactive();
  var _selectedTab = 0;
  var _activeNodeIndex = 0;
  var _insecureTrustAcknowledged = false;
  var _sidebarExpanded = false;

  RelayNodeDraft get _activeNode => _nodes[_activeNodeIndex];

  @override
  void initState() {
    super.initState();
    _clientController = widget.controller ?? ClientController();
    _trafficController = TrafficModeController.platform(
      runtime: _clientController,
      libraryPath: NativeClientRuntime.libraryPath,
      systemProxyOptions: _trafficConfiguration.systemProxyOptions,
      tunOptions: _trafficConfiguration.tunOptions,
      routingRules: () => _trafficConfiguration.routingRules().serialize(),
    );
    _lastSnapshot = _clientController.snapshot;
    _lastPollingError = _clientController.pollingError;
    _lastReconnectStatus = _clientController.reconnectStatus;
    _clientController.addListener(_handleControllerChanged);
    _trafficController.addListener(_handleTrafficChanged);
    unawaited(_restoreEnrollments());
  }

  @override
  void dispose() {
    _trafficController.removeListener(_handleTrafficChanged);
    _trafficController.dispose();
    _clientController.removeListener(_handleControllerChanged);
    _clientController.dispose();
    _profileFile.dispose();
    _trafficConfiguration.dispose();
    for (final node in _nodes) {
      node.dispose();
    }
    super.dispose();
  }

  Future<void> _toggleConnection() async {
    final phase = _clientController.snapshot.phase;
    if (phase == ClientRuntimePhase.stopping) return;
    if (phase == ClientRuntimePhase.online ||
        phase == ClientRuntimePhase.connecting) {
      if (_trafficController.activeMode != TrafficMode.off ||
          _trafficController.busy) {
        try {
          await _trafficController.suspend();
        } on Object {
          _reportError('Traffic routing could not be disabled safely.');
          return;
        }
      }
      try {
        _clientController.stop();
      } on ClientControlException catch (error) {
        _reportError(error.toString());
      } on Object {
        _reportError('The native runtime could not stop.');
      }
      return;
    }
    if (!_hasCompleteConfiguration) {
      setState(() => _selectedTab = 2);
      _reportError('Complete the local relay configuration before connecting.');
      return;
    }
    if (_activeNode.trustMode == ClientTrustMode.insecure &&
        !_insecureTrustAcknowledged) {
      setState(() => _selectedTab = 2);
      _reportError(
        'Acknowledge the insecure connection risk before connecting.',
      );
      return;
    }
    try {
      _clientController.start(
        await _connectionRequest(),
        reconnectConfiguration: _connectionRequest,
      );
    } on ClientControlException catch (error) {
      _reportError(error.toString());
    } on FileSystemException catch (error) {
      _reportError('Cannot read client configuration: ${error.message}');
    } on FormatException catch (error) {
      _reportError('Invalid credential file: ${error.message}');
    } on Object {
      _reportError('The native runtime library could not be loaded.');
    }
  }

  bool get _hasCompleteConfiguration => [
    _activeNode.id.text,
    _activeNode.relayAddress.text,
    _activeNode.serverName.text,
    _activeNode.credentialRef.text.isNotEmpty
        ? _activeNode.credentialRef.text
        : _activeNode.credentialPath.text,
    if (_activeNode.trustMode == ClientTrustMode.customCa)
      _activeNode.certificateRef.text.isNotEmpty
          ? _activeNode.certificateRef.text
          : _activeNode.certificatePath.text,
  ].every((value) => value.trim().isNotEmpty);

  Future<ClientRuntimeConfiguration> _connectionRequest() async {
    final credential = _activeNode.credentialRef.text.trim().isEmpty
        ? _decodeCredential(
            await File(_activeNode.credentialPath.text.trim()).readAsString(),
          )
        : await _secretStore.credential(
            _activeNode.credentialRef.text.trim(),
            migrationFile: _activeNode.credentialPath.text,
          );
    final certificate = _activeNode.trustMode != ClientTrustMode.customCa
        ? Uint8List(0)
        : _activeNode.certificateRef.text.trim().isEmpty
        ? await File(_activeNode.certificatePath.text.trim()).readAsBytes()
        : await _secretStore.certificate(
            _activeNode.certificateRef.text.trim(),
            migrationFile: _activeNode.certificatePath.text,
          );
    return ClientRuntimeConfiguration(
      libraryPath: NativeClientRuntime.libraryPath(),
      relayAddress: _activeNode.relayAddress.text.trim(),
      serverName: _activeNode.serverName.text.trim(),
      credential: credential,
      trustMode: _activeNode.trustMode,
      certificatePem: certificate,
    );
  }

  Future<void> _importProfile() async {
    final sourcePath = _profileFile.text.trim();
    if (sourcePath.isEmpty) {
      _reportError('Select a Velum profile YAML file to import.');
      return;
    }
    try {
      final codec = NativeProfileCodec.open(NativeClientRuntime.libraryPath());
      final profile = await _profileRepository.importFile(
        sourcePath,
        codec.normalize,
      );
      if (!mounted) return;
      final imported = profile.nodes.map((node) {
        return RelayNodeDraft(
          id: node.id,
          name: node.alias,
          relayAddress: node.relayAddress,
          serverName: node.serverName,
          credentialRef: node.credentialRef,
          certificateRef: node.caRef ?? '',
          trustMode: node.trustMode == 'custom-ca'
              ? ClientTrustMode.customCa
              : ClientTrustMode.system,
        );
      }).toList();
      final active = imported.indexWhere(
        (node) => node.id.text == profile.defaultNode,
      );
      setState(() {
        for (final node in _nodes) {
          node.dispose();
        }
        _nodes
          ..clear()
          ..addAll(imported);
        _activeNodeIndex = active < 0 ? 0 : active;
      });
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Imported ${profile.name}.')));
    } on Object catch (error) {
      _reportError('Profile import failed: $error');
    }
  }

  Future<void> _importEnrollmentFile() async {
    const enrollmentType = XTypeGroup(
      label: 'Velum enrollment',
      extensions: ['velum-enroll'],
    );
    try {
      final selected = await openFile(
        acceptedTypeGroups: const [enrollmentType],
      );
      if (selected == null) return;
      final installed = await _installEnrollment(await selected.readAsString());
      if (!installed) return;
      var removed = false;
      try {
        final source = File(selected.path);
        if (await source.exists()) {
          await source.delete();
          removed = true;
        }
      } on Object {
        // Some platform document providers do not grant delete access.
      }
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            removed
                ? 'Enrollment installed and source file removed.'
                : 'Enrollment installed. Remove the source file from its original location.',
          ),
        ),
      );
    } on Object catch (error) {
      _reportError('Enrollment import failed: $error');
    }
  }

  Future<void> _scanEnrollment() async {
    final source = await Navigator.of(context).push<String>(
      MaterialPageRoute(
        fullscreenDialog: true,
        builder: (_) => const EnrollmentScannerPage(),
      ),
    );
    if (source == null || !mounted) return;
    try {
      if (await _installEnrollment(source) && mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(const SnackBar(content: Text('Enrollment installed.')));
      }
    } on Object catch (error) {
      _reportError('Enrollment scan failed: $error');
    }
  }

  Future<bool> _installEnrollment(String source) async {
    final codec = NativeEnrollmentCodec.open(NativeClientRuntime.libraryPath());
    final enrollment = ClientEnrollment.parseCanonical(codec.normalize(source));
    final certificate = enrollment.certificatePem == null
        ? null
        : Uint8List.fromList(utf8.encode(enrollment.certificatePem!));
    var stored = false;
    try {
      await _secretStore.installEnrollment(
        credentialReference: enrollment.credentialRef,
        credential: enrollment.credential,
        certificateReference: enrollment.certificateRef,
        certificate: certificate,
      );
      stored = true;
      await _enrollmentRepository.upsert(enrollment.installedNode);
    } on Object {
      if (stored) {
        await _secretStore.removeEnrollment(
          credentialReference: enrollment.credentialRef,
          certificateReference: enrollment.certificateRef,
        );
      }
      rethrow;
    } finally {
      enrollment.credential.fillRange(0, enrollment.credential.length, 0);
      certificate?.fillRange(0, certificate.length, 0);
    }
    if (!mounted) return false;
    final node = RelayNodeDraft(
      id: enrollment.nodeId,
      name: enrollment.nodeName,
      relayAddress: enrollment.relayAddress,
      serverName: enrollment.serverName,
      credentialRef: enrollment.credentialRef,
      certificateRef: enrollment.certificateRef ?? '',
      trustMode: enrollment.trustMode == 'custom-ca'
          ? ClientTrustMode.customCa
          : ClientTrustMode.system,
    );
    setState(() {
      final existing = _nodes.indexWhere(
        (candidate) => candidate.id.text == enrollment.nodeId,
      );
      if (existing >= 0) {
        _nodes[existing].dispose();
        _nodes[existing] = node;
        _activeNodeIndex = existing;
      } else {
        _nodes.add(node);
        _activeNodeIndex = _nodes.length - 1;
      }
      _selectedTab = 1;
    });
    return true;
  }

  Future<void> _restoreEnrollments() async {
    try {
      final installed = await _enrollmentRepository.load();
      if (!mounted || installed.isEmpty) return;
      final restored = installed
          .map(
            (node) => RelayNodeDraft(
              id: node.nodeId,
              name: node.nodeName,
              relayAddress: node.relayAddress,
              serverName: node.serverName,
              credentialRef: node.credentialRef,
              certificateRef: node.certificateRef ?? '',
              trustMode: node.trustMode == 'custom-ca'
                  ? ClientTrustMode.customCa
                  : ClientTrustMode.system,
            ),
          )
          .toList();
      setState(() {
        for (final node in restored) {
          final existing = _nodes.indexWhere(
            (candidate) => candidate.id.text == node.id.text,
          );
          if (existing >= 0) {
            _nodes[existing].dispose();
            _nodes[existing] = node;
          } else {
            _nodes.add(node);
          }
        }
      });
    } on Object catch (error) {
      _reportError('Installed enrollments could not be restored: $error');
    }
  }

  void _addNode() {
    setState(() {
      final number = _nodes.length + 1;
      _nodes.add(RelayNodeDraft(id: 'node-$number', name: 'Node $number'));
      _activeNodeIndex = _nodes.length - 1;
    });
  }

  Future<void> _removeNode(int index) async {
    if (_nodes.length == 1) return;
    final node = _nodes[index];
    try {
      await _enrollmentRepository.remove(node.id.text);
      if (node.credentialRef.text.startsWith('secret://velum/enrollment/')) {
        await _secretStore.removeEnrollment(
          credentialReference: node.credentialRef.text,
          certificateReference: node.certificateRef.text.isEmpty
              ? null
              : node.certificateRef.text,
        );
      }
    } on Object catch (error) {
      _reportError('Node removal failed: $error');
      return;
    }
    if (!mounted) return;
    setState(() {
      final removed = _nodes.removeAt(index);
      removed.dispose();
      if (_activeNodeIndex >= _nodes.length) {
        _activeNodeIndex = _nodes.length - 1;
      } else if (index < _activeNodeIndex) {
        _activeNodeIndex -= 1;
      }
    });
  }

  void _selectActiveNode(int index) => setState(() => _activeNodeIndex = index);

  Future<void> _changeTrustMode(
    RelayNodeDraft node,
    ClientTrustMode trustMode,
  ) async {
    if (trustMode != ClientTrustMode.insecure || _insecureTrustAcknowledged) {
      setState(() => node.trustMode = trustMode);
      return;
    }
    final acknowledged = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (_) => const _InsecureTrustDialog(),
    );
    if (acknowledged == true && mounted) {
      setState(() {
        _insecureTrustAcknowledged = true;
        node.trustMode = ClientTrustMode.insecure;
      });
    }
  }

  void _handleControllerChanged() {
    if (!mounted) return;
    final next = _clientController.snapshot;
    final pollingError = _clientController.pollingError;
    final reconnectStatus = _clientController.reconnectStatus;
    if (next.revision == _lastSnapshot.revision &&
        next.generation == _lastSnapshot.generation &&
        identical(pollingError, _lastPollingError) &&
        reconnectStatus == _lastReconnectStatus) {
      return;
    }
    setState(() {
      _lastSnapshot = next;
      _lastPollingError = pollingError;
      _lastReconnectStatus = reconnectStatus;
    });
  }

  void _handleTrafficChanged() {
    if (mounted) setState(() {});
  }

  Future<void> _selectTrafficMode(TrafficMode mode) async {
    final configurationError = switch (mode) {
      TrafficMode.off => null,
      TrafficMode.systemProxy => _trafficConfiguration.validateSystemProxy(),
      TrafficMode.tun => _trafficConfiguration.validateTun(),
    };
    if (configurationError case final error?) {
      _reportError(error);
      return;
    }
    try {
      await _trafficController.select(mode);
    } on Object {
      if (_trafficController.error case final message?) _reportError(message);
    }
  }

  void _reportError(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }

  void _selectTab(int index) => setState(() => _selectedTab = index);

  String get _pageTitle => switch (_selectedTab) {
    0 => 'Overview',
    1 => 'Nodes',
    2 => 'Configuration',
    _ => 'Settings',
  };

  @override
  Widget build(BuildContext context) {
    final snapshot = _clientController.snapshot;
    final configurationReady = _hasCompleteConfiguration;
    final page = switch (_selectedTab) {
      0 => ClientOverview(
        snapshot: snapshot,
        relayAddress: _activeNode.relayAddress.text.trim(),
        serverName: _activeNode.serverName.text.trim(),
        configurationReady: configurationReady,
        onConfigure: () => _selectTab(2),
        availableTrafficModes: _trafficController.availableModes,
        selectedTrafficMode: _trafficController.selectedMode,
        activeTrafficMode: _trafficController.activeMode,
        trafficModePhase: _trafficController.phase,
        trafficModeError: _trafficController.error,
        onTrafficModeChanged: _selectTrafficMode,
        routingMode: _trafficConfiguration.routingMode,
        onRoutingModeChanged: (mode) =>
            setState(() => _trafficConfiguration.routingMode = mode),
      ),
      1 => _NodesPanel(
        nodes: _nodes,
        activeNodeIndex: _activeNodeIndex,
        editable: const {
          ClientRuntimePhase.stopped,
          ClientRuntimePhase.failed,
        }.contains(snapshot.phase),
        onSelect: _selectActiveNode,
        onConfigure: () => _selectTab(2),
      ),
      2 => ClientConfigurationPanel(
        formKey: _formKey,
        snapshot: snapshot,
        nodes: _nodes,
        activeNodeIndex: _activeNodeIndex,
        onAddNode: _addNode,
        onRemoveNode: _removeNode,
        onSelectNode: _selectActiveNode,
        onTrustModeChanged: _changeTrustMode,
        profileFile: _profileFile,
        onImportProfile: _importProfile,
        onImportEnrollment: _importEnrollmentFile,
        onScanEnrollment: _scanEnrollment,
        canScanEnrollment: Platform.isAndroid,
      ),
      _ => ClientSettingsPanel(
        controller: _trafficController,
        reconnectStatus: _clientController.reconnectStatus,
        configuration: _trafficConfiguration,
        onModeChanged: _selectTrafficMode,
        onConfigurationChanged: () => setState(() {}),
      ),
    };
    return Scaffold(
      body: ClientBackdrop(
        child: SafeArea(
          child: LayoutBuilder(
            builder: (context, constraints) {
              final wide = constraints.maxWidth >= 900;
              final content = Expanded(
                child: Padding(
                  padding: EdgeInsets.fromLTRB(
                    wide && _selectedTab == 0
                        ? 16
                        : wide
                        ? 48
                        : _selectedTab == 0
                        ? 16
                        : 24,
                    wide && _selectedTab == 0 ? 0 : 36,
                    wide && _selectedTab == 0
                        ? 16
                        : wide
                        ? 48
                        : _selectedTab == 0
                        ? 16
                        : 24,
                    wide ? 96 : 176,
                  ),
                  child: page,
                ),
              );
              if (wide) {
                return Stack(
                  children: [
                    Row(
                      children: [
                        _Sidebar(
                          selectedIndex: _selectedTab,
                          onSelected: _selectTab,
                          online: snapshot.phase == ClientRuntimePhase.online,
                          expanded: _sidebarExpanded,
                          onToggleExpanded: () => setState(
                            () => _sidebarExpanded = !_sidebarExpanded,
                          ),
                        ),
                        Expanded(
                          child: Column(
                            children: [
                              _TopBar(
                                title: _pageTitle,
                                snapshot: snapshot,
                                onEdit: _selectedTab == 0
                                    ? () => _selectTab(2)
                                    : null,
                              ),
                              content,
                            ],
                          ),
                        ),
                      ],
                    ),
                    Positioned(
                      right: 32,
                      bottom: 28,
                      child: _ConnectionAction(
                        snapshot: snapshot,
                        configurationReady: configurationReady,
                        onToggle: _toggleConnection,
                      ),
                    ),
                  ],
                );
              }
              return Stack(
                children: [
                  Column(
                    children: [
                      _TopBar(
                        title: _pageTitle,
                        snapshot: snapshot,
                        onEdit: _selectedTab == 0 ? () => _selectTab(2) : null,
                      ),
                      content,
                    ],
                  ),
                  Positioned(
                    right: 24,
                    bottom: 92,
                    child: _ConnectionAction(
                      snapshot: snapshot,
                      configurationReady: configurationReady,
                      onToggle: _toggleConnection,
                    ),
                  ),
                  Positioned(
                    left: 0,
                    right: 0,
                    bottom: 0,
                    child: SizedBox(
                      height: 78,
                      child: ClientCompactNavigation(
                        selectedIndex: _selectedTab,
                        onSelected: _selectTab,
                      ),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}

class _InsecureTrustDialog extends StatefulWidget {
  const _InsecureTrustDialog();

  @override
  State<_InsecureTrustDialog> createState() => _InsecureTrustDialogState();
}

class _InsecureTrustDialogState extends State<_InsecureTrustDialog> {
  static const _acknowledgementDelay = Duration(seconds: 3);

  late final Timer _timer;
  var _remainingSeconds = _acknowledgementDelay.inSeconds;
  var _understandsRisk = false;

  @override
  void initState() {
    super.initState();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted) return;
      setState(() {
        _remainingSeconds -= 1;
        if (_remainingSeconds == 0) _timer.cancel();
      });
    });
  }

  @override
  void dispose() {
    _timer.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: const Text('Insecure connection'),
    content: Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          'This disables certificate and server-name verification. A malicious network can impersonate the relay and read or alter traffic.',
        ),
        const SizedBox(height: 16),
        CheckboxListTile(
          contentPadding: EdgeInsets.zero,
          value: _understandsRisk,
          onChanged: _remainingSeconds == 0
              ? (value) => setState(() => _understandsRisk = value ?? false)
              : null,
          title: const Text('I understand the risk'),
        ),
        if (_remainingSeconds > 0)
          Text(
            'Reviewing risk: $_remainingSeconds s',
            style: const TextStyle(color: ClientTheme.warning),
          ),
      ],
    ),
    actions: [
      TextButton(
        onPressed: () => Navigator.of(context).pop(false),
        child: const Text('Cancel'),
      ),
      FilledButton(
        onPressed: _remainingSeconds == 0 && _understandsRisk
            ? () => Navigator.of(context).pop(true)
            : null,
        child: const Text('I understand the risk'),
      ),
    ],
  );
}

class _Sidebar extends StatelessWidget {
  const _Sidebar({
    required this.selectedIndex,
    required this.onSelected,
    required this.online,
    required this.expanded,
    required this.onToggleExpanded,
  });

  final int selectedIndex;
  final ValueChanged<int> onSelected;
  final bool online;
  final bool expanded;
  final VoidCallback onToggleExpanded;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: expanded ? 224 : 78,
    child: ClipRect(
      child: Stack(
        fit: StackFit.expand,
        children: [
          const DecoratedBox(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                stops: [.0, .42, 1],
                colors: [
                  Color(0xff0a3033),
                  Color(0xff08171d),
                  ClientTheme.background,
                ],
              ),
            ),
          ),
          BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 18, sigmaY: 18),
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: ClientTheme.panel.withValues(alpha: .56),
                border: const Border(
                  right: BorderSide(color: ClientTheme.border),
                ),
              ),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(12, 20, 12, 16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Container(
                      width: 40,
                      height: 40,
                      decoration: BoxDecoration(
                        color: ClientTheme.accent.withValues(alpha: .11),
                        border: Border.all(
                          color: ClientTheme.accent.withValues(alpha: .30),
                        ),
                        borderRadius: BorderRadius.circular(9),
                      ),
                      child: const Icon(
                        Icons.shield_outlined,
                        color: ClientTheme.accent,
                        size: 21,
                      ),
                    ),
                    const SizedBox(height: 24),
                    _NavigationItem(
                      label: 'Overview',
                      icon: Icons.radar_outlined,
                      expanded: expanded,
                      selected: selectedIndex == 0,
                      onTap: () => onSelected(0),
                    ),
                    _NavigationItem(
                      label: 'Nodes',
                      icon: Icons.hub_outlined,
                      expanded: expanded,
                      selected: selectedIndex == 1,
                      onTap: () => onSelected(1),
                    ),
                    _NavigationItem(
                      label: 'Config',
                      icon: Icons.tune_outlined,
                      expanded: expanded,
                      selected: selectedIndex == 2,
                      onTap: () => onSelected(2),
                    ),
                    _NavigationItem(
                      label: 'Settings',
                      icon: Icons.settings_outlined,
                      expanded: expanded,
                      selected: selectedIndex == 3,
                      onTap: () => onSelected(3),
                    ),
                    const Spacer(),
                    _SidebarStatus(online: online),
                    const SizedBox(height: 10),
                    IconButton(
                      tooltip: expanded
                          ? 'Collapse navigation'
                          : 'Expand navigation',
                      onPressed: onToggleExpanded,
                      icon: Icon(
                        expanded ? Icons.menu_open_rounded : Icons.menu_rounded,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    ),
  );
}

class _NavigationItem extends StatelessWidget {
  const _NavigationItem({
    required this.label,
    required this.icon,
    required this.selected,
    required this.expanded,
    required this.onTap,
  });

  final String label;
  final IconData icon;
  final bool selected;
  final bool expanded;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 8),
    child: Material(
      color: selected
          ? ClientTheme.accent.withValues(alpha: .10)
          : Colors.transparent,
      child: InkWell(
        onTap: onTap,
        child: SizedBox(
          height: 44,
          child: Row(
            mainAxisAlignment: expanded
                ? MainAxisAlignment.start
                : MainAxisAlignment.center,
            children: [
              Tooltip(
                message: expanded ? '' : label,
                child: Icon(
                  icon,
                  size: 20,
                  color: selected ? ClientTheme.accent : ClientTheme.muted,
                ),
              ),
              if (expanded) ...[
                const SizedBox(width: 12),
                Text(
                  label,
                  style: TextStyle(
                    color: selected ? ClientTheme.text : ClientTheme.muted,
                    fontWeight: selected ? FontWeight.w700 : FontWeight.w500,
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    ),
  );
}

class _SidebarStatus extends StatelessWidget {
  const _SidebarStatus({required this.online});
  final bool online;

  @override
  Widget build(BuildContext context) => Tooltip(
    message: online ? 'Runtime online' : 'Runtime offline',
    child: Column(
      children: [
        Container(
          width: 8,
          height: 8,
          decoration: BoxDecoration(
            color: online ? ClientTheme.accent : ClientTheme.danger,
            shape: BoxShape.circle,
            boxShadow: [
              BoxShadow(
                color: (online ? ClientTheme.accent : ClientTheme.danger)
                    .withValues(alpha: .45),
                blurRadius: 8,
              ),
            ],
          ),
        ),
        const SizedBox(height: 6),
        Text(
          online ? 'ON' : 'OFF',
          style: TextStyle(
            color: online ? ClientTheme.accent : ClientTheme.muted,
            fontSize: 9,
            fontWeight: FontWeight.w700,
            letterSpacing: 1,
          ),
        ),
      ],
    ),
  );
}

class _NodesPanel extends StatelessWidget {
  const _NodesPanel({
    required this.nodes,
    required this.activeNodeIndex,
    required this.editable,
    required this.onSelect,
    required this.onConfigure,
  });

  final List<RelayNodeDraft> nodes;
  final int activeNodeIndex;
  final bool editable;
  final ValueChanged<int> onSelect;
  final VoidCallback onConfigure;

  @override
  Widget build(BuildContext context) => ListView(
    children: [
      SectionLabel('Nodes'),
      SizedBox(height: 12),
      Text(
        'Relay nodes',
        style: TextStyle(fontSize: 21, fontWeight: FontWeight.w700),
      ),
      SizedBox(height: 6),
      Text(
        'Choose the node used for the next connection. Node credentials remain local to this device.',
      ),
      SizedBox(height: 20),
      ...List.generate(nodes.length, (index) {
        final node = nodes[index];
        final active = index == activeNodeIndex;
        return Padding(
          padding: const EdgeInsets.only(bottom: 12),
          child: ClientPanel(
            child: Material(
              color: Colors.transparent,
              child: ListTile(
                contentPadding: EdgeInsets.zero,
                leading: Icon(
                  active
                      ? Icons.radio_button_checked_outlined
                      : Icons.radio_button_unchecked_outlined,
                  color: active ? ClientTheme.accent : ClientTheme.muted,
                ),
                title: Text(
                  node.name.text.trim().isEmpty
                      ? 'Node ${index + 1}'
                      : node.name.text.trim(),
                ),
                subtitle: Text(
                  node.relayAddress.text.trim().isEmpty
                      ? 'Incomplete configuration'
                      : node.relayAddress.text.trim(),
                ),
                onTap: editable ? () => onSelect(index) : null,
              ),
            ),
          ),
        );
      }),
      OutlinedButton.icon(
        onPressed: onConfigure,
        icon: const Icon(Icons.tune_outlined),
        label: const Text('Manage nodes'),
      ),
    ],
  );
}

class _TopBar extends StatelessWidget {
  const _TopBar({required this.title, required this.snapshot, this.onEdit});

  final String title;
  final ClientRuntimeSnapshot snapshot;
  final VoidCallback? onEdit;

  @override
  Widget build(BuildContext context) => Container(
    height: 64,
    width: double.infinity,
    padding: const EdgeInsets.symmetric(horizontal: 20),
    decoration: const BoxDecoration(color: ClientTheme.background),
    child: Row(
      children: [
        Expanded(
          child: Text(
            title,
            style: const TextStyle(fontWeight: FontWeight.w700),
          ),
        ),
        Tooltip(
          message: switch (snapshot.phase) {
            ClientRuntimePhase.stopped => 'Offline',
            ClientRuntimePhase.connecting => 'Connecting',
            ClientRuntimePhase.online => 'Connected',
            ClientRuntimePhase.stopping => 'Disconnecting',
            ClientRuntimePhase.failed => 'Connection failed',
          },
          child: Icon(
            switch (snapshot.phase) {
              ClientRuntimePhase.online => Icons.check_circle_rounded,
              ClientRuntimePhase.connecting => Icons.sync_rounded,
              ClientRuntimePhase.failed => Icons.error_rounded,
              ClientRuntimePhase.stopped ||
              ClientRuntimePhase.stopping => Icons.circle_outlined,
            },
            color: switch (snapshot.phase) {
              ClientRuntimePhase.online => ClientTheme.accent,
              ClientRuntimePhase.connecting => ClientTheme.warning,
              ClientRuntimePhase.failed => ClientTheme.danger,
              ClientRuntimePhase.stopped ||
              ClientRuntimePhase.stopping => ClientTheme.muted,
            },
            size: 30,
          ),
        ),
        if (onEdit != null) ...[
          const SizedBox(width: 8),
          IconButton(
            tooltip: 'Edit connection',
            onPressed: onEdit,
            icon: const Icon(Icons.edit_rounded, size: 19),
          ),
        ],
      ],
    ),
  );
}

class _ConnectionAction extends StatelessWidget {
  const _ConnectionAction({
    required this.snapshot,
    required this.configurationReady,
    required this.onToggle,
  });

  final ClientRuntimeSnapshot snapshot;
  final bool configurationReady;
  final Future<void> Function() onToggle;

  @override
  Widget build(BuildContext context) {
    final stopping = snapshot.phase == ClientRuntimePhase.stopping;
    final active = switch (snapshot.phase) {
      ClientRuntimePhase.connecting || ClientRuntimePhase.online => true,
      ClientRuntimePhase.stopped ||
      ClientRuntimePhase.stopping ||
      ClientRuntimePhase.failed => false,
    };
    final needsConfiguration = !active && !stopping && !configurationReady;
    final label = stopping
        ? 'STOPPING'
        : active
        ? 'STOP'
        : 'START';
    final icon = stopping
        ? const SizedBox(
            width: 18,
            height: 18,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        : Icon(active ? Icons.stop_rounded : Icons.play_arrow_rounded);

    return Tooltip(
      message: active
          ? 'Stop connection'
          : needsConfiguration
          ? 'Complete connection configuration to start'
          : 'Start connection',
      child: SizedBox(
        key: const ValueKey('connection-action'),
        width: 132,
        height: 52,
        child: FilledButton.icon(
          onPressed: stopping ? null : onToggle,
          style: FilledButton.styleFrom(
            backgroundColor: active ? ClientTheme.danger : ClientTheme.accent,
            foregroundColor: ClientTheme.background,
          ),
          icon: icon,
          label: Text(label),
        ),
      ),
    );
  }
}

Uint8List _decodeCredential(String value) {
  final encoded = value.trim();
  if (encoded.isEmpty || encoded.length.isOdd) {
    throw const FormatException(
      'credential must contain hexadecimal byte pairs',
    );
  }
  final credential = Uint8List(encoded.length ~/ 2);
  for (var index = 0; index < credential.length; index += 1) {
    final pair = encoded.substring(index * 2, index * 2 + 2);
    final byte = int.tryParse(pair, radix: 16);
    if (byte == null) {
      throw const FormatException(
        'credential must contain hexadecimal byte pairs',
      );
    }
    credential[index] = byte;
  }
  return credential;
}
