import 'package:flutter/material.dart';

class OverviewDashboardLayout extends StatelessWidget {
  const OverviewDashboardLayout({
    required this.networkSpeed,
    required this.systemProxy,
    required this.tun,
    required this.outboundMode,
    required this.publicIp,
    required this.localIp,
    required this.trafficStats,
    super.key,
  });

  final Widget networkSpeed;
  final Widget systemProxy;
  final Widget tun;
  final Widget outboundMode;
  final Widget publicIp;
  final Widget localIp;
  final Widget trafficStats;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      if (constraints.maxWidth >= 900) return _desktop();
      if (constraints.maxWidth >= 600) return _tablet();
      return _mobile();
    },
  );

  Widget _desktop() => Column(
    children: [
      SizedBox(
        height: 174,
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(
              flex: 2,
              child: _slot('dashboard-network-speed', networkSpeed),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                children: [
                  Expanded(child: _slot('dashboard-system-proxy', systemProxy)),
                  const SizedBox(height: 14),
                  Expanded(child: _slot('dashboard-tun', tun)),
                ],
              ),
            ),
          ],
        ),
      ),
      const SizedBox(height: 14),
      SizedBox(
        height: 174,
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(child: _slot('dashboard-outbound-mode', outboundMode)),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                children: [
                  Expanded(child: _slot('dashboard-public-ip', publicIp)),
                  const SizedBox(height: 14),
                  Expanded(child: _slot('dashboard-local-ip', localIp)),
                ],
              ),
            ),
            const SizedBox(width: 14),
            Expanded(child: _slot('dashboard-traffic-stats', trafficStats)),
          ],
        ),
      ),
    ],
  );

  Widget _tablet() => Column(
    children: [
      SizedBox(
        height: 210,
        child: _slot('dashboard-network-speed', networkSpeed),
      ),
      const SizedBox(height: 14),
      SizedBox(
        height: 80,
        child: Row(
          children: [
            Expanded(child: _slot('dashboard-system-proxy', systemProxy)),
            const SizedBox(width: 14),
            Expanded(child: _slot('dashboard-tun', tun)),
          ],
        ),
      ),
      const SizedBox(height: 14),
      _fixed(174, 'dashboard-outbound-mode', outboundMode),
      const SizedBox(height: 14),
      SizedBox(
        height: 80,
        child: Row(
          children: [
            Expanded(child: _slot('dashboard-public-ip', publicIp)),
            const SizedBox(width: 14),
            Expanded(child: _slot('dashboard-local-ip', localIp)),
          ],
        ),
      ),
      const SizedBox(height: 14),
      _fixed(174, 'dashboard-traffic-stats', trafficStats),
    ],
  );

  Widget _mobile() => Column(
    children: [
      SizedBox(
        height: 190,
        child: _slot('dashboard-network-speed', networkSpeed),
      ),
      const SizedBox(height: 14),
      Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              children: [
                _fixed(174, 'dashboard-outbound-mode', outboundMode),
                const SizedBox(height: 14),
                _fixed(80, 'dashboard-local-ip', localIp),
              ],
            ),
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              children: [
                _fixed(80, 'dashboard-public-ip', publicIp),
                const SizedBox(height: 14),
                _fixed(174, 'dashboard-traffic-stats', trafficStats),
              ],
            ),
          ),
        ],
      ),
    ],
  );

  Widget _slot(String key, Widget child) =>
      KeyedSubtree(key: ValueKey(key), child: child);

  Widget _fixed(double height, String key, Widget child) =>
      SizedBox(height: height, child: _slot(key, child));
}
