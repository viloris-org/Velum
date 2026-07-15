import 'package:flutter/material.dart';

import 'native_client.dart';
import 'overview_dashboard_controls.dart';
import 'overview_dashboard_layout.dart';
import 'overview_dashboard_network.dart';
import 'traffic_configuration.dart';
import 'traffic_mode_controller.dart';

class OverviewDashboard extends StatelessWidget {
  const OverviewDashboard({
    required this.snapshot,
    required this.relayAddress,
    required this.serverName,
    required this.configurationReady,
    required this.onConfigure,
    required this.availableTrafficModes,
    required this.selectedTrafficMode,
    required this.activeTrafficMode,
    required this.trafficModePhase,
    required this.trafficModeError,
    required this.onTrafficModeChanged,
    required this.routingMode,
    required this.onRoutingModeChanged,
    super.key,
  });

  final ClientRuntimeSnapshot snapshot;
  final String relayAddress;
  final String serverName;
  final bool configurationReady;
  final VoidCallback onConfigure;
  final Set<TrafficMode> availableTrafficModes;
  final TrafficMode selectedTrafficMode;
  final TrafficMode activeTrafficMode;
  final TrafficModePhase trafficModePhase;
  final String? trafficModeError;
  final ValueChanged<TrafficMode>? onTrafficModeChanged;
  final RoutingMode routingMode;
  final ValueChanged<RoutingMode>? onRoutingModeChanged;

  @override
  Widget build(BuildContext context) => ListView(
    children: [
      OverviewDashboardLayout(
        networkSpeed: const DashboardNetworkSpeed(),
        systemProxy: DashboardAdapterSwitch(
          label: 'System proxy',
          icon: Icons.shuffle_rounded,
          mode: TrafficMode.systemProxy,
          selectedMode: selectedTrafficMode,
          available: availableTrafficModes.contains(TrafficMode.systemProxy),
          busy: trafficModePhase == TrafficModePhase.applying,
          onModeChanged: onTrafficModeChanged,
        ),
        tun: DashboardAdapterSwitch(
          label: 'Virtual network',
          icon: Icons.vpn_lock_outlined,
          mode: TrafficMode.tun,
          selectedMode: selectedTrafficMode,
          available: availableTrafficModes.contains(TrafficMode.tun),
          busy: trafficModePhase == TrafficModePhase.applying,
          onModeChanged: onTrafficModeChanged,
        ),
        outboundMode: DashboardOutboundMode(
          mode: routingMode,
          onChanged: onRoutingModeChanged,
        ),
        publicIp: const DashboardPublicIp(),
        localIp: const DashboardLocalIp(),
        trafficStats: const DashboardTrafficStats(),
      ),
      if (trafficModeError case final error?) ...[
        const SizedBox(height: 10),
        Text(
          error,
          style: const TextStyle(color: Colors.redAccent, fontSize: 12),
        ),
      ],
    ],
  );
}
