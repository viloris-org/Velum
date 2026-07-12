import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/controller/velum_controller.dart';
import 'package:velum_client/services/velum_bridge.dart';
import 'package:velum_client/ui/velum_app.dart';

void main() {
  testWidgets('renders the Velum operator overview', (tester) async {
    final controller = VelumController(createVelumBridge());
    await tester.pumpWidget(VelumApp(controller: controller));
    await tester.pump();

    expect(find.text('VELUM'), findsWidgets);
    expect(find.textContaining('会话不必重来'), findsOneWidget);
    expect(find.text('刷新状态'), findsOneWidget);
  });

  testWidgets('opens configuration workspace from navigation', (tester) async {
    final controller = VelumController(createVelumBridge());
    await tester.pumpWidget(VelumApp(controller: controller));
    await tester.pump();

    await tester.tap(find.text('配置').first);
    await tester.pump();

    expect(find.text('配置工作台'), findsOneWidget);
    expect(find.text('config.toml'), findsOneWidget);
  });
}
