import 'dart:async';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui';

import 'package:flutter/material.dart';

import 'client_configuration.dart';
import 'client_compact_navigation.dart';
import 'client_controller.dart';
import 'client_overview.dart';
import 'client_theme.dart';
import 'native_client.dart';
import 'system_proxy.dart';

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
  final _library = TextEditingController(
    text: NativeClientRuntime.defaultLibraryName(),
  );
  final _nodes = <RelayNodeDraft>[
    RelayNodeDraft(
      name: 'Local relay',
      relayAddress: '127.0.0.1:4433',
      serverName: 'localhost',
    ),
  ];

  late final ClientController _clientController;
  late ClientRuntimeSnapshot _lastSnapshot;
  Object? _lastPollingError;
  var _selectedTab = 0;
  var _activeNodeIndex = 0;
  var _insecureTrustAcknowledged = false;
  var _sidebarExpanded = true;
  final _systemProxy = SystemProxy();
  var _systemProxyEnabled = false;

  RelayNodeDraft get _activeNode => _nodes[_activeNodeIndex];

  @override
  void initState() {
    super.initState();
    _clientController = widget.controller ?? ClientController();
    _lastSnapshot = _clientController.snapshot;
    _lastPollingError = _clientController.pollingError;
    _clientController.addListener(_handleControllerChanged);
  }

  @override
  void dispose() {
    if (_systemProxyEnabled) {
      unawaited(_systemProxy.disable());
    }
    _clientController.removeListener(_handleControllerChanged);
    _clientController.dispose();
    _library.dispose();
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
      if (_systemProxyEnabled) await _disableSystemProxy();
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
      _clientController.start(await _connectionRequest());
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
    _library.text,
    _activeNode.relayAddress.text,
    _activeNode.serverName.text,
    _activeNode.credentialPath.text,
    if (_activeNode.trustMode == ClientTrustMode.customCa)
      _activeNode.certificatePath.text,
  ].every((value) => value.trim().isNotEmpty);

  Future<ClientRuntimeConfiguration> _connectionRequest() async =>
      ClientRuntimeConfiguration(
        libraryPath: _library.text.trim(),
        relayAddress: _activeNode.relayAddress.text.trim(),
        serverName: _activeNode.serverName.text.trim(),
        credential: _decodeCredential(
          await File(_activeNode.credentialPath.text.trim()).readAsString(),
        ),
        trustMode: _activeNode.trustMode,
        certificatePem: _activeNode.trustMode == ClientTrustMode.customCa
            ? await File(_activeNode.certificatePath.text.trim()).readAsBytes()
            : Uint8List(0),
      );

  void _addNode() {
    setState(() {
      _nodes.add(RelayNodeDraft(name: 'Node ${_nodes.length + 1}'));
      _activeNodeIndex = _nodes.length - 1;
    });
  }

  void _removeNode(int index) {
    if (_nodes.length == 1) return;
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
    if (next.revision == _lastSnapshot.revision &&
        next.generation == _lastSnapshot.generation &&
        identical(pollingError, _lastPollingError)) {
      return;
    }
    setState(() {
      _lastSnapshot = next;
      _lastPollingError = pollingError;
    });
    if (_systemProxyEnabled && next.phase != ClientRuntimePhase.online) {
      _disableSystemProxy();
    }
  }

  Future<void> _toggleSystemProxy(bool enabled) async {
    if (enabled) {
      if (_clientController.snapshot.phase != ClientRuntimePhase.online) {
        _reportError('Connect to the relay before enabling the system proxy.');
        return;
      }
      try {
        final port = _clientController.startLoopbackProxy();
        await _systemProxy.enable(port);
        if (!mounted) return;
        setState(() => _systemProxyEnabled = true);
      } on Object {
        try {
          await _systemProxy.disable();
        } on Object {
          // Keep the original failure as the user-visible outcome.
        }
        try {
          _clientController.stopLoopbackProxy();
        } on Object {
          // The system setting was not installed, so cleanup is best effort.
        }
        _reportError('The system proxy could not be enabled.');
      }
      return;
    }
    await _disableSystemProxy();
  }

  Future<void> _disableSystemProxy() async {
    try {
      await _systemProxy.disable();
    } on Object {
      _reportError('The system proxy could not be disabled.');
      return;
    }
    try {
      _clientController.stopLoopbackProxy();
    } on Object {
      // The system setting is already disabled; the native runtime will also
      // release its listener during stop and destroy.
    }
    if (mounted) setState(() => _systemProxyEnabled = false);
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
        library: _library,
      ),
      _ => _SettingsPanel(
        systemProxyEnabled: _systemProxyEnabled,
        available: Platform.isLinux || Platform.isMacOS || Platform.isWindows,
        runtimeOnline: snapshot.phase == ClientRuntimePhase.online,
        onSystemProxyChanged: _toggleSystemProxy,
      ),
    };
    return Scaffold(
      body: SafeArea(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final wide = constraints.maxWidth >= 900;
            final content = Expanded(
              child: Padding(
                padding: EdgeInsets.fromLTRB(
                  wide ? 48 : 24,
                  36,
                  wide ? 48 : 24,
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
                      ),
                      Expanded(
                        child: Column(
                          children: [
                            _TopBar(
                              title: _pageTitle,
                              snapshot: snapshot,
                              sidebarExpanded: _sidebarExpanded,
                              onToggleSidebar: () => setState(
                                () => _sidebarExpanded = !_sidebarExpanded,
                              ),
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
                      onConfigure: () => _selectTab(2),
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
                      sidebarExpanded: false,
                      onToggleSidebar: () {},
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
                    onConfigure: () => _selectTab(2),
                    onToggle: _toggleConnection,
                  ),
                ),
                Positioned(
                  left: 14,
                  right: 14,
                  bottom: 10,
                  child: SizedBox(
                    height: 68,
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
  });

  final int selectedIndex;
  final ValueChanged<int> onSelected;
  final bool online;
  final bool expanded;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: expanded ? 224 : 72,
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
  const _TopBar({
    required this.title,
    required this.snapshot,
    required this.sidebarExpanded,
    required this.onToggleSidebar,
  });

  final String title;
  final ClientRuntimeSnapshot snapshot;
  final bool sidebarExpanded;
  final VoidCallback onToggleSidebar;

  @override
  Widget build(BuildContext context) => Container(
    height: 64,
    width: double.infinity,
    padding: const EdgeInsets.symmetric(horizontal: 20),
    decoration: const BoxDecoration(
      color: ClientTheme.background,
      border: Border(bottom: BorderSide(color: ClientTheme.border)),
    ),
    child: Row(
      children: [
        Tooltip(
          message: sidebarExpanded
              ? 'Collapse navigation'
              : 'Expand navigation',
          child: IconButton(
            onPressed: onToggleSidebar,
            icon: Icon(
              sidebarExpanded ? Icons.menu_open_outlined : Icons.menu_outlined,
            ),
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            title,
            style: const TextStyle(fontWeight: FontWeight.w700),
          ),
        ),
        Text(
          switch (snapshot.phase) {
            ClientRuntimePhase.stopped => 'OFFLINE',
            ClientRuntimePhase.connecting => 'CONNECTING',
            ClientRuntimePhase.online => 'CONNECTED',
            ClientRuntimePhase.stopping => 'DISCONNECTING',
            ClientRuntimePhase.failed => 'CONNECTION FAILED',
          },
          style: TextStyle(
            color: switch (snapshot.phase) {
              ClientRuntimePhase.online => ClientTheme.accent,
              ClientRuntimePhase.connecting => ClientTheme.warning,
              ClientRuntimePhase.failed => ClientTheme.danger,
              ClientRuntimePhase.stopped ||
              ClientRuntimePhase.stopping => ClientTheme.muted,
            },
            fontSize: 11,
            fontWeight: FontWeight.w700,
          ),
        ),
      ],
    ),
  );
}

class _ConnectionAction extends StatelessWidget {
  const _ConnectionAction({
    required this.snapshot,
    required this.configurationReady,
    required this.onConfigure,
    required this.onToggle,
  });

  final ClientRuntimeSnapshot snapshot;
  final bool configurationReady;
  final VoidCallback onConfigure;
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
        : needsConfiguration
        ? 'CONFIGURE'
        : 'START';
    final icon = stopping
        ? const SizedBox(
            width: 18,
            height: 18,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        : Icon(
            active
                ? Icons.stop_rounded
                : needsConfiguration
                ? Icons.tune_rounded
                : Icons.play_arrow_rounded,
          );

    return Tooltip(
      message: active
          ? 'Stop connection'
          : needsConfiguration
          ? 'Configure connection'
          : 'Start connection',
      child: SizedBox(
        key: const ValueKey('connection-action'),
        width: 132,
        height: 52,
        child: FilledButton.icon(
          onPressed: stopping
              ? null
              : needsConfiguration
              ? onConfigure
              : onToggle,
          style: FilledButton.styleFrom(
            backgroundColor: active
                ? ClientTheme.danger
                : needsConfiguration
                ? ClientTheme.warning
                : ClientTheme.accent,
            foregroundColor: ClientTheme.background,
          ),
          icon: icon,
          label: Text(label),
        ),
      ),
    );
  }
}

class _SettingsPanel extends StatelessWidget {
  const _SettingsPanel({
    required this.systemProxyEnabled,
    required this.available,
    required this.runtimeOnline,
    required this.onSystemProxyChanged,
  });

  final bool systemProxyEnabled;
  final bool available;
  final bool runtimeOnline;
  final ValueChanged<bool> onSystemProxyChanged;

  @override
  Widget build(BuildContext context) => ListView(
    children: [
      const SectionLabel('Settings'),
      const SizedBox(height: 12),
      const Text(
        'Settings',
        style: TextStyle(fontSize: 21, fontWeight: FontWeight.w700),
      ),
      const SizedBox(height: 6),
      const Text('This local client does not require an account.'),
      const SizedBox(height: 20),
      ClientPanel(
        child: SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('System proxy'),
          subtitle: Text(
            !available
                ? 'System proxy is unavailable on this platform.'
                : !runtimeOnline
                ? 'Connect to the relay before enabling the system proxy.'
                : 'Enable IP-literal CONNECT and SOCKS traffic through the active relay.',
          ),
          value: systemProxyEnabled,
          onChanged: available && runtimeOnline ? onSystemProxyChanged : null,
        ),
      ),
      const SizedBox(height: 16),
      ClientPanel(
        child: const Padding(
          padding: EdgeInsets.zero,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'About Velum',
                style: TextStyle(fontSize: 16, fontWeight: FontWeight.w700),
              ),
              SizedBox(height: 12),
              Text('Experimental encrypted-tunneling client.'),
              SizedBox(height: 8),
              Text(
                'Apache-2.0 licensed. All configuration remains on this device.',
              ),
            ],
          ),
        ),
      ),
    ],
  );
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
