import 'package:flutter/material.dart';

import 'client_theme.dart';
import 'native_client.dart';
import 'public_ip_service.dart';
import 'traffic_chart.dart';

class OverviewDashboard extends StatelessWidget {
  const OverviewDashboard({
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
  Widget build(BuildContext context) {
    final online = snapshot.phase == ClientRuntimePhase.online;
    final status = switch (snapshot.phase) {
      ClientRuntimePhase.online => ('Connected', ClientTheme.accent),
      ClientRuntimePhase.connecting => ('Connecting', ClientTheme.warning),
      ClientRuntimePhase.stopping => ('Disconnecting', ClientTheme.muted),
      ClientRuntimePhase.failed => ('Connection failed', ClientTheme.danger),
      ClientRuntimePhase.stopped => ('Disconnected', ClientTheme.muted),
    };
    return LayoutBuilder(
      builder: (context, constraints) {
        final cardWidth = constraints.maxWidth >= 900
            ? (constraints.maxWidth - 16) / 2
            : constraints.maxWidth;
        return ListView(
          children: [
            const SectionLabel('Overview'),
            const SizedBox(height: 20),
            Wrap(
              spacing: 16,
              runSpacing: 16,
              children: [
                SizedBox(
                  width: cardWidth,
                  child: ClientPanel(
                    child: Column(
                      children: [
                        Text(
                          'Connection',
                          style: Theme.of(context).textTheme.titleMedium,
                        ),
                        const SizedBox(height: 24),
                        Text(
                          status.$1,
                          style: TextStyle(
                            color: status.$2,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                SizedBox(
                  width: cardWidth,
                  child: _DashboardCard(
                    title: 'Current node',
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        _ValueRow(
                          'Relay',
                          relayAddress.isEmpty
                              ? 'Not configured'
                              : relayAddress,
                        ),
                        const SizedBox(height: 12),
                        _ValueRow(
                          'Server name',
                          serverName.isEmpty ? 'Not configured' : serverName,
                        ),
                        const SizedBox(height: 12),
                        _ValueRow(
                          'Configuration',
                          configurationReady
                              ? 'Ready to connect'
                              : 'Needs configuration',
                          color: configurationReady
                              ? ClientTheme.accent
                              : ClientTheme.warning,
                        ),
                        const SizedBox(height: 16),
                        OutlinedButton.icon(
                          onPressed: onConfigure,
                          icon: const Icon(Icons.tune_rounded, size: 16),
                          label: const Text('Open configuration'),
                        ),
                      ],
                    ),
                  ),
                ),
                SizedBox(
                  width: cardWidth,
                  child: _PublicIpCard(online: online),
                ),
                SizedBox(
                  width: cardWidth,
                  child: _DashboardCard(
                    title: 'Runtime',
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        _ValueRow('State', status.$1, color: status.$2),
                        const SizedBox(height: 12),
                        _ValueRow('Generation', '${snapshot.generation}'),
                        const SizedBox(height: 12),
                        const _ValueRow(
                          'Diagnostics',
                          'No retained events yet',
                        ),
                      ],
                    ),
                  ),
                ),
                SizedBox(
                  width: cardWidth,
                  child: _DashboardCard(
                    title: 'Traffic',
                    child: online
                        ? const TrafficChart(samples: [])
                        : const Text(
                            'Connect to begin collecting runtime metrics.',
                            style: TextStyle(color: ClientTheme.muted),
                          ),
                  ),
                ),
              ],
            ),
          ],
        );
      },
    );
  }
}

class _PublicIpCard extends StatefulWidget {
  const _PublicIpCard({required this.online});
  final bool online;

  @override
  State<_PublicIpCard> createState() => _PublicIpCardState();
}

class _PublicIpCardState extends State<_PublicIpCard> {
  final _service = const PublicIpService();
  Future<PublicIpDetails>? _request;

  void _refresh() => setState(() => _request = _service.lookup());

  @override
  Widget build(BuildContext context) => _DashboardCard(
    title: 'Public IP',
    child: !widget.online
        ? const Text(
            'Connect before checking the device public IP.',
            style: TextStyle(color: ClientTheme.muted),
          )
        : _request == null
        ? Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Check the public IP through IPinfo.',
                style: TextStyle(color: ClientTheme.muted),
              ),
              const SizedBox(height: 12),
              OutlinedButton.icon(
                onPressed: _refresh,
                icon: const Icon(Icons.public_rounded, size: 16),
                label: const Text('Check public IP'),
              ),
            ],
          )
        : FutureBuilder<PublicIpDetails>(
            future: _request,
            builder: (context, snapshot) {
              if (snapshot.hasError) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      'Could not retrieve the public IP.',
                      style: TextStyle(color: ClientTheme.warning),
                    ),
                    const SizedBox(height: 12),
                    OutlinedButton.icon(
                      onPressed: _refresh,
                      icon: const Icon(Icons.refresh_rounded, size: 16),
                      label: const Text('Try again'),
                    ),
                  ],
                );
              }
              if (!snapshot.hasData) return const LinearProgressIndicator();
              final details = snapshot.data!;
              return Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    details.ip,
                    style: const TextStyle(
                      fontSize: 20,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    details.organization,
                    style: const TextStyle(color: ClientTheme.muted),
                  ),
                  const SizedBox(height: 12),
                  TextButton.icon(
                    onPressed: _refresh,
                    icon: const Icon(Icons.refresh_rounded, size: 16),
                    label: const Text('Refresh'),
                  ),
                ],
              );
            },
          ),
  );
}

class _DashboardCard extends StatelessWidget {
  const _DashboardCard({required this.title, required this.child});
  final String title;
  final Widget child;
  @override
  Widget build(BuildContext context) => ClientPanel(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 18),
        child,
      ],
    ),
  );
}

class _ValueRow extends StatelessWidget {
  const _ValueRow(this.label, this.value, {this.color = ClientTheme.text});
  final String label;
  final String value;
  final Color color;
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text(
        label.toUpperCase(),
        style: const TextStyle(color: ClientTheme.muted, fontSize: 10),
      ),
      const SizedBox(height: 4),
      Text(
        value,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(color: color, fontWeight: FontWeight.w600),
      ),
    ],
  );
}
