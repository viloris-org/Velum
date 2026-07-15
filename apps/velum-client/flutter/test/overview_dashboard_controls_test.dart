import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/overview_dashboard_controls.dart';
import 'package:velum_client/traffic_configuration.dart';
import 'package:velum_client/traffic_mode_controller.dart';

void main() {
  testWidgets('adapter switch selects its traffic mode', (tester) async {
    TrafficMode? selected;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 320,
            height: 80,
            child: DashboardAdapterSwitch(
              label: 'System proxy',
              icon: Icons.shuffle_rounded,
              mode: TrafficMode.systemProxy,
              selectedMode: TrafficMode.off,
              available: true,
              busy: false,
              onModeChanged: (mode) => selected = mode,
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.byType(Switch));
    expect(selected, TrafficMode.systemProxy);
  });

  testWidgets('outbound mode exposes rule global and direct choices', (
    tester,
  ) async {
    RoutingMode? selected;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 240,
            height: 174,
            child: DashboardOutboundMode(
              mode: RoutingMode.rule,
              onChanged: (mode) => selected = mode,
            ),
          ),
        ),
      ),
    );

    await tester.tap(
      find.byWidgetPredicate(
        (widget) =>
            widget is Radio<RoutingMode> && widget.value == RoutingMode.global,
      ),
    );
    expect(selected, RoutingMode.global);
  });
}
