import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/controller/velum_controller.dart';
import 'package:velum_client/models/operator_models.dart';
import 'package:velum_client/services/velum_bridge.dart';

void main() {
  test('configuration emits top-level targets before TOML tables', () {
    const config = VelumConfiguration(
      allowedTargets: '203.0.113.10:443, 198.51.100.24:8443',
    );

    final toml = config.toToml();

    expect(toml, contains('version = 1'));
    expect(
      toml,
      contains('allowed_targets = ["203.0.113.10:443", "198.51.100.24:8443"]'),
    );
    expect(
      toml.indexOf('allowed_targets'),
      lessThan(toml.indexOf('[listener]')),
    );
    expect(toml, contains('[[credentials]]'));
    expect(toml, contains('max_connections = 1024'));
  });

  test(
    'demo refresh updates aggregate status without local commands',
    () async {
      final controller = VelumController(createVelumBridge());
      final before = controller.snapshot.admittedConnections;

      await controller.refreshStatus();

      expect(controller.snapshot.phase, ServicePhase.online);
      expect(controller.snapshot.admittedConnections, greaterThan(before));
      expect(controller.lastCommandOutput, isNotEmpty);
      expect(controller.events.first.title, '状态已刷新');
    },
  );

  test('demo service controls change lifecycle phase', () async {
    final controller = VelumController(createVelumBridge());

    await controller.controlService('drain');
    expect(controller.snapshot.phase, ServicePhase.draining);

    await controller.controlService('shutdown');
    expect(controller.snapshot.phase, ServicePhase.offline);
    expect(controller.snapshot.activeFlows, 0);
  });
}
