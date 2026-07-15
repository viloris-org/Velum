import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/overview_dashboard_layout.dart';

void main() {
  testWidgets('wide dashboard matches the three-column reference grid', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    Widget card(String label) => ColoredBox(
      color: Colors.black,
      child: Center(child: Text(label)),
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: OverviewDashboardLayout(
            networkSpeed: card('network speed'),
            systemProxy: card('system proxy'),
            tun: card('tun'),
            outboundMode: card('outbound mode'),
            publicIp: card('public IP'),
            localIp: card('local IP'),
            trafficStats: card('traffic stats'),
          ),
        ),
      ),
    );

    final network = find.byKey(const ValueKey('dashboard-network-speed'));
    final systemProxy = find.byKey(const ValueKey('dashboard-system-proxy'));
    final tun = find.byKey(const ValueKey('dashboard-tun'));
    final outbound = find.byKey(const ValueKey('dashboard-outbound-mode'));
    final publicIp = find.byKey(const ValueKey('dashboard-public-ip'));
    final localIp = find.byKey(const ValueKey('dashboard-local-ip'));
    final stats = find.byKey(const ValueKey('dashboard-traffic-stats'));

    expect(tester.getSize(network).height, 174);
    expect(tester.getSize(network).width, closeTo(790.7, .1));
    expect(tester.getSize(systemProxy).height, 80);
    expect(tester.getSize(tun).height, 80);
    expect(tester.getTopLeft(tun).dy, 94);
    expect(tester.getTopLeft(outbound).dy, 188);
    expect(tester.getTopLeft(publicIp).dy, 188);
    expect(tester.getTopLeft(localIp).dy, 282);
    expect(tester.getTopLeft(stats).dy, 188);
  });

  testWidgets('mobile dashboard uses the compact two-column reference grid', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    Widget card(String label) => ColoredBox(
      color: Colors.black,
      child: Center(child: Text(label)),
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: OverviewDashboardLayout(
            networkSpeed: card('network speed'),
            systemProxy: card('system proxy'),
            tun: card('tun'),
            outboundMode: card('outbound mode'),
            publicIp: card('public IP'),
            localIp: card('local IP'),
            trafficStats: card('traffic stats'),
          ),
        ),
      ),
    );

    final network = find.byKey(const ValueKey('dashboard-network-speed'));
    final outbound = find.byKey(const ValueKey('dashboard-outbound-mode'));
    final publicIp = find.byKey(const ValueKey('dashboard-public-ip'));
    final localIp = find.byKey(const ValueKey('dashboard-local-ip'));
    final stats = find.byKey(const ValueKey('dashboard-traffic-stats'));

    expect(tester.getSize(network), const Size(390, 190));
    expect(tester.getTopLeft(outbound), const Offset(0, 204));
    expect(tester.getTopLeft(publicIp), const Offset(202, 204));
    expect(tester.getTopLeft(localIp), const Offset(0, 392));
    expect(tester.getTopLeft(stats), const Offset(202, 298));
    expect(find.byKey(const ValueKey('dashboard-system-proxy')), findsNothing);
    expect(find.byKey(const ValueKey('dashboard-tun')), findsNothing);
  });
}
