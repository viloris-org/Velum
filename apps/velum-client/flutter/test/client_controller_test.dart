import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/client_controller.dart';
import 'package:velum_client/native_client.dart';

import 'support/fake_client_runtime.dart';

void main() {
  test('stop supersedes an in-flight connection generation', () {
    final runtime = FakeClientRuntime();
    final controller = ClientController(
      runtimeFactory: (_) => runtime,
      pollInterval: const Duration(days: 1),
    );
    addTearDown(controller.dispose);

    controller.start(testRuntimeConfiguration());
    expect(controller.snapshot.phase, ClientRuntimePhase.connecting);

    controller.stop();
    expect(runtime.stopCount, 1);
    expect(controller.snapshot.phase, ClientRuntimePhase.stopped);
    expect(controller.snapshot.generation, 2);

    runtime.current = const ClientRuntimeSnapshot(
      revision: 3,
      generation: 1,
      phase: ClientRuntimePhase.online,
      failure: ClientRuntimeFailure.none,
    );
    expect(controller.refresh(), isFalse);
    expect(controller.snapshot.phase, ClientRuntimePhase.stopped);
  });

  test('older snapshot revisions cannot overwrite current state', () {
    final runtime = FakeClientRuntime();
    final controller = ClientController(
      runtimeFactory: (_) => runtime,
      pollInterval: const Duration(days: 1),
    );
    addTearDown(controller.dispose);

    controller.start(testRuntimeConfiguration());
    runtime.current = const ClientRuntimeSnapshot(
      revision: 5,
      generation: 1,
      phase: ClientRuntimePhase.online,
      failure: ClientRuntimeFailure.none,
    );
    expect(controller.refresh(), isTrue);

    runtime.current = const ClientRuntimeSnapshot(
      revision: 4,
      generation: 1,
      phase: ClientRuntimePhase.failed,
      failure: ClientRuntimeFailure.connection,
    );
    expect(controller.refresh(), isFalse);
    expect(controller.snapshot.phase, ClientRuntimePhase.online);
    expect(controller.snapshot.revision, 5);
  });

  test('dispose swallows cleanup errors and destroys exactly once', () {
    final runtime = FakeClientRuntime()
      ..throwOnStop = true
      ..throwOnDestroy = true;
    final controller = ClientController(
      runtimeFactory: (_) => runtime,
      pollInterval: const Duration(days: 1),
    );
    controller.start(testRuntimeConfiguration());

    expect(controller.dispose, returnsNormally);
    expect(controller.dispose, returnsNormally);
    expect(runtime.stopCount, 1);
    expect(runtime.destroyCount, 1);
  });

  test('successful polling clears an error without a new revision', () async {
    final runtime = FakeClientRuntime();
    final controller = ClientController(
      runtimeFactory: (_) => runtime,
      pollInterval: const Duration(milliseconds: 1),
    );
    addTearDown(controller.dispose);
    controller.start(testRuntimeConfiguration());
    runtime.throwOnSnapshot = true;

    await Future<void>.delayed(const Duration(milliseconds: 10));
    expect(controller.pollingError, isNotNull);

    runtime.throwOnSnapshot = false;
    await Future<void>.delayed(const Duration(milliseconds: 10));
    expect(controller.pollingError, isNull);
  });
}
