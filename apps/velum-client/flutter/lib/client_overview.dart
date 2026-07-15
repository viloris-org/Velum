import 'package:flutter/material.dart';

import 'native_client.dart';
import 'overview_dashboard.dart';

class ClientOverview extends StatelessWidget {
  const ClientOverview({
    required this.snapshot,
    required this.relayAddress,
    required this.serverName,
    required this.configurationReady,
    required this.onConfigure,
    super.key,
  });

  final ClientRuntimeSnapshot snapshot;
  final String relayAddress;
  final String serverName;
  final bool configurationReady;
  final VoidCallback onConfigure;

  @override
  Widget build(BuildContext context) => OverviewDashboard(
    snapshot: snapshot,
    relayAddress: relayAddress,
    serverName: serverName,
    configurationReady: configurationReady,
    onConfigure: onConfigure,
  );
}
