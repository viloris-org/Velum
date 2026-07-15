import 'package:flutter/material.dart';

import 'client_theme.dart';
import 'routing_rules_panel.dart';
import 'traffic_configuration.dart';
import 'traffic_mode_controller.dart';
import 'traffic_mode_panel.dart';
import 'traffic_options_panel.dart';

class ClientSettingsPanel extends StatelessWidget {
  const ClientSettingsPanel({
    required this.controller,
    required this.configuration,
    required this.onModeChanged,
    required this.onConfigurationChanged,
    super.key,
  });

  final TrafficModeController controller;
  final TrafficConfigurationDraft configuration;
  final ValueChanged<TrafficMode> onModeChanged;
  final VoidCallback onConfigurationChanged;

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
      TrafficModePanel(
        availableModes: controller.availableModes,
        selectedMode: controller.selectedMode,
        activeMode: controller.activeMode,
        phase: controller.phase,
        runtimeOnline: controller.runtimeOnline,
        error: controller.error,
        onModeChanged: onModeChanged,
      ),
      const SizedBox(height: 16),
      TrafficOptionsPanel(
        draft: configuration,
        onChanged: onConfigurationChanged,
      ),
      const SizedBox(height: 16),
      RoutingRulesPanel(
        draft: configuration,
        onChanged: onConfigurationChanged,
      ),
      if (configuration.validate() case final error?) ...[
        const SizedBox(height: 12),
        Text(
          error,
          key: const ValueKey('traffic-configuration-error'),
          style: const TextStyle(color: ClientTheme.danger, fontSize: 12),
        ),
      ],
      const SizedBox(height: 16),
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
}
