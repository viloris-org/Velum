import 'package:flutter/material.dart';

import 'native_client.dart';
import 'overview_dashboard.dart';
import 'traffic_configuration.dart';
import 'traffic_mode_controller.dart';

class ClientOverview extends StatelessWidget {
  const ClientOverview({
    required this.snapshot,
    required this.relayAddress,
    required this.serverName,
    required this.configurationReady,
    required this.onConfigure,
    this.availableTrafficModes = const {TrafficMode.off},
    this.selectedTrafficMode = TrafficMode.off,
    this.activeTrafficMode = TrafficMode.off,
    this.trafficModePhase = TrafficModePhase.inactive,
    this.trafficModeError,
    this.onTrafficModeChanged,
    this.routingMode = RoutingMode.rule,
    this.onRoutingModeChanged,
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
  Widget build(BuildContext context) => OverviewDashboard(
    snapshot: snapshot,
    relayAddress: relayAddress,
    serverName: serverName,
    configurationReady: configurationReady,
    onConfigure: onConfigure,
    availableTrafficModes: availableTrafficModes,
    selectedTrafficMode: selectedTrafficMode,
    activeTrafficMode: activeTrafficMode,
    trafficModePhase: trafficModePhase,
    trafficModeError: trafficModeError,
    onTrafficModeChanged: onTrafficModeChanged,
    routingMode: routingMode,
    onRoutingModeChanged: onRoutingModeChanged,
  );
}
