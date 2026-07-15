import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/client_compact_navigation.dart';
import 'package:velum_client/client_controller.dart';
import 'package:velum_client/client_overview.dart';
import 'package:velum_client/main.dart';
import 'package:velum_client/native_client.dart';
import 'package:velum_client/traffic_chart.dart';

import 'support/fake_client_runtime.dart';

void main() {
  testWidgets('shows the disconnected client control surface', (tester) async {
    await tester.pumpWidget(const VelumClientApp());

    expect(find.text('Overview'), findsAtLeastNWidgets(1));
    expect(find.byTooltip('Offline'), findsOneWidget);
    expect(find.text('CONFIGURE'), findsNothing);
    expect(find.text('Network speed'), findsOneWidget);
    expect(find.text('System proxy'), findsOneWidget);
  });

  testWidgets('switches between overview, config, and settings', (
    tester,
  ) async {
    await tester.pumpWidget(const VelumClientApp());

    await tester.tap(find.text('Config'));
    await tester.pump();
    expect(find.text('Connection configuration'), findsOneWidget);
    expect(find.byKey(const ValueKey('import-enrollment')), findsOneWidget);
    expect(find.byKey(const ValueKey('scan-enrollment')), findsNothing);

    await tester.tap(find.text('Settings'));
    await tester.pump();
    expect(find.text('Traffic settings'), findsOneWidget);
    expect(find.text('Traffic control'), findsOneWidget);
    await tester.scrollUntilVisible(
      find.text('Local traffic adapters'),
      300,
      scrollable: find.byType(Scrollable).first,
    );
    await tester.pump();
    expect(find.text('Local traffic adapters'), findsOneWidget);
    expect(find.text('System proxy'), findsOneWidget);
    await tester.scrollUntilVisible(
      find.byKey(const ValueKey('routing-rules')),
      300,
      scrollable: find.byType(Scrollable).first,
    );
    await tester.pump();
    expect(find.text('Routing mode'), findsOneWidget);
    expect(find.byKey(const ValueKey('routing-rules')), findsOneWidget);
  });

  testWidgets('adds a node and makes it the active connection node', (
    tester,
  ) async {
    await tester.pumpWidget(const VelumClientApp());

    await tester.tap(find.text('Config'));
    await tester.pump();
    final addNode = find.byKey(const ValueKey('add-node'));
    await tester.scrollUntilVisible(
      addNode,
      300,
      scrollable: find.byType(Scrollable).first,
    );
    await tester.drag(find.byType(Scrollable).first, const Offset(0, -220));
    await tester.pump();
    await tester.tap(addNode);
    await tester.pump();

    expect(find.text('Node 2'), findsNWidgets(2));
    expect(find.byTooltip('Active connection node'), findsOneWidget);

    await tester.tap(find.text('Nodes'));
    await tester.pump();
    expect(find.text('Node 2'), findsOneWidget);
  });

  testWidgets(
    'requires a three-second risk review before enabling insecure trust',
    (tester) async {
      tester.view.physicalSize = const Size(800, 1200);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(const VelumClientApp());

      await tester.tap(find.text('Config'));
      await tester.pump();
      await tester.tap(find.byType(DropdownButtonFormField<ClientTrustMode>));
      await tester.pump();
      await tester.tap(find.text('Allow insecure connection').last);
      await tester.pump();

      expect(find.text('Insecure connection'), findsOneWidget);
      expect(tester.widget<Checkbox>(find.byType(Checkbox)).onChanged, isNull);
      for (var second = 0; second < 3; second += 1) {
        await tester.pump(const Duration(seconds: 1));
      }
      expect(
        tester.widget<Checkbox>(find.byType(Checkbox)).onChanged,
        isNotNull,
      );

      await tester.tap(find.byType(Checkbox));
      await tester.pump();
      await tester.tap(
        find.widgetWithText(FilledButton, 'I understand the risk'),
      );
      await tester.pumpAndSettle();
      expect(find.text('Insecure connection'), findsNothing);
    },
  );

  testWidgets('keeps compact navigation inside a mobile-safe bottom bar', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(const VelumClientApp());

    expect(find.byType(ClientCompactNavigation), findsOneWidget);
    expect(tester.getSize(find.byType(ClientCompactNavigation)).height, 78);
    expect(find.byKey(const ValueKey('connection-action')), findsOneWidget);
    expect(find.text('Overview'), findsAtLeastNWidgets(1));
    expect(find.text('Nodes'), findsOneWidget);
    expect(find.text('Config'), findsOneWidget);
    expect(find.text('Settings'), findsOneWidget);
  });

  testWidgets(
    'shows the primary start action when configuration is incomplete',
    (tester) async {
      await tester.pumpWidget(const VelumClientApp());

      expect(find.text('CONFIGURE'), findsNothing);
      expect(find.text('START'), findsOneWidget);
      expect(find.byKey(const ValueKey('connection-action')), findsOneWidget);
    },
  );

  testWidgets(
    'can stop while connecting without accepting a stale online state',
    (tester) async {
      final runtime = FakeClientRuntime();
      final controller = ClientController(
        runtimeFactory: (_) => runtime,
        pollInterval: const Duration(days: 1),
      );
      controller.start(testRuntimeConfiguration());

      await tester.pumpWidget(VelumClientApp(controller: controller));
      expect(find.byTooltip('Connecting'), findsOneWidget);
      expect(find.text('STOP'), findsOneWidget);

      await tester.tap(find.text('STOP'));
      await tester.pump();
      expect(runtime.stopCount, 1);
      expect(find.byTooltip('Offline'), findsOneWidget);

      runtime.current = const ClientRuntimeSnapshot(
        revision: 3,
        generation: 1,
        phase: ClientRuntimePhase.online,
        failure: ClientRuntimeFailure.none,
      );
      expect(controller.refresh(), isFalse);
      await tester.pump();
      expect(find.text('Runtime online'), findsNothing);
      expect(find.byTooltip('Offline'), findsOneWidget);

      await tester.pumpWidget(const SizedBox.shrink());
      expect(runtime.destroyCount, 1);
    },
  );

  testWidgets('shows the traffic chart while the runtime is online', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: ClientOverview(
          snapshot: const ClientRuntimeSnapshot(
            revision: 2,
            generation: 1,
            phase: ClientRuntimePhase.online,
            failure: ClientRuntimeFailure.none,
          ),
          relayAddress: '127.0.0.1:4433',
          serverName: 'localhost',
          configurationReady: true,
          onConfigure: () {},
        ),
      ),
    );

    expect(find.text('Network speed'), findsOneWidget);
    expect(find.byType(TrafficChart), findsOneWidget);
    expect(find.text('Waiting for runtime metrics'), findsNothing);
    expect(find.byIcon(Icons.power_settings_new_rounded), findsNothing);
    expect(find.byIcon(Icons.play_arrow_rounded), findsNothing);
  });
}
