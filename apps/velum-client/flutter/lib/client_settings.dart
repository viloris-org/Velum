import 'package:flutter/material.dart';

import 'client_controller.dart';
import 'client_theme.dart';
import 'routing_rules_panel.dart';
import 'settings_section.dart';
import 'traffic_configuration.dart';
import 'traffic_mode_controller.dart';
import 'traffic_mode_panel.dart';
import 'traffic_options_panel.dart';

class ClientSettingsPanel extends StatelessWidget {
  const ClientSettingsPanel({
    required this.controller,
    required this.reconnectStatus,
    required this.configuration,
    required this.onModeChanged,
    required this.onConfigurationChanged,
    super.key,
  });

  final TrafficModeController controller;
  final ClientReconnectStatus reconnectStatus;
  final TrafficConfigurationDraft configuration;
  final ValueChanged<TrafficMode> onModeChanged;
  final VoidCallback onConfigurationChanged;

  @override
  Widget build(BuildContext context) => ListView(
    children: [
      const SectionLabel('Settings'),
      const SizedBox(height: 12),
      const Text(
        'Traffic settings',
        style: TextStyle(fontSize: 21, fontWeight: FontWeight.w700),
      ),
      const SizedBox(height: 6),
      Text(_platformSummary()),
      const SizedBox(height: 20),
      SettingsSection(
        eyebrow: 'Device routing',
        title: 'Traffic control',
        description:
            'Choose how Velum takes over device traffic after the relay is online.',
        icon: Icons.route_outlined,
        child: TrafficModePanel(
          availableModes: controller.availableModes,
          selectedMode: controller.selectedMode,
          activeMode: controller.activeMode,
          phase: controller.phase,
          runtimeOnline: controller.runtimeOnline,
          error: controller.error,
          onModeChanged: onModeChanged,
        ),
      ),
      const SizedBox(height: 16),
      _ReconnectStatusPanel(status: reconnectStatus),
      const SizedBox(height: 28),
      SettingsSection(
        eyebrow: 'Adapter configuration',
        title: 'Local traffic adapters',
        description: _adapterSummary(),
        icon: Icons.settings_ethernet_outlined,
        child: TrafficOptionsPanel(
          draft: configuration,
          availableModes: controller.availableModes,
          onChanged: onConfigurationChanged,
        ),
      ),
      if (controller.availableModes.contains(TrafficMode.systemProxy)) ...[
        const SizedBox(height: 28),
        SettingsSection(
          eyebrow: 'Policy',
          title: 'Desktop proxy rules',
          description:
              'Rules are evaluated in order by the local desktop proxy.',
          icon: Icons.account_tree_outlined,
          child: RoutingRulesPanel(
            draft: configuration,
            onChanged: onConfigurationChanged,
          ),
        ),
      ],
      if (configuration.validate() case final error?) ...[
        const SizedBox(height: 16),
        ClientPanel(
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Icon(
                Icons.error_outline,
                color: ClientTheme.danger,
                size: 18,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  error,
                  key: const ValueKey('traffic-configuration-error'),
                  style: const TextStyle(
                    color: ClientTheme.danger,
                    fontSize: 12,
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
      const SizedBox(height: 28),
      const ClientPanel(
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
    ],
  );

  String _platformSummary() {
    final modes = controller.availableModes;
    if (modes.contains(TrafficMode.systemProxy) &&
        modes.contains(TrafficMode.tun)) {
      return 'This device supports both system proxy and TUN traffic routing.';
    }
    if (modes.contains(TrafficMode.systemProxy)) {
      return 'This device supports system proxy traffic routing.';
    }
    if (modes.contains(TrafficMode.tun)) {
      return 'This device supports TUN VPN traffic routing.';
    }
    return 'No system traffic adapter is available on this platform.';
  }

  String _adapterSummary() {
    final modes = controller.availableModes;
    if (modes.contains(TrafficMode.systemProxy) &&
        modes.contains(TrafficMode.tun)) {
      return 'Configure the system proxy and TUN adapter used by this device.';
    }
    if (modes.contains(TrafficMode.systemProxy)) {
      return 'Configure the local port and hosts excluded from the system proxy.';
    }
    return 'Configure the address, DNS servers, and routes used by the TUN VPN.';
  }
}

class _ReconnectStatusPanel extends StatelessWidget {
  const _ReconnectStatusPanel({required this.status});

  final ClientReconnectStatus status;

  @override
  Widget build(BuildContext context) {
    final (icon, title, description, color) = switch (status.phase) {
      ClientReconnectPhase.waiting => (
        Icons.schedule_outlined,
        'Reconnect scheduled',
        'Retry ${status.attempt} of ${status.maxAttempts} will start shortly.',
        ClientTheme.warning,
      ),
      ClientReconnectPhase.reconnecting => (
        Icons.sync_outlined,
        'Reconnecting',
        'Retry ${status.attempt} of ${status.maxAttempts} is connecting.',
        ClientTheme.accent,
      ),
      ClientReconnectPhase.exhausted => (
        Icons.error_outline,
        'Reconnect paused',
        '${status.maxAttempts} retry attempts were exhausted. Start the connection again to retry.',
        ClientTheme.danger,
      ),
      ClientReconnectPhase.inactive => (
        Icons.restart_alt_outlined,
        'Automatic reconnection',
        'Transport failures retry up to three times with bounded backoff.',
        ClientTheme.muted,
      ),
    };
    return ClientPanel(
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, color: color, size: 19),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title, style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 3),
                Text(
                  description,
                  style: const TextStyle(
                    color: ClientTheme.muted,
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
