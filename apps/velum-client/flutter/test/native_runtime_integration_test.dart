import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/native_client.dart';

const _libraryPath = String.fromEnvironment('VELUM_CLIENT_LIBRARY');

void main() {
  test(
    'Dart runtime ABI loads the native library and controls a stopped handle',
    () {
      final runtime = NativeClientRuntime.open(_libraryPath);
      addTearDown(runtime.destroy);

      final initial = runtime.snapshot();
      expect(initial.revision, 0);
      expect(initial.generation, 0);
      expect(initial.phase, ClientRuntimePhase.stopped);
      expect(initial.failure, ClientRuntimeFailure.none);

      expect(runtime.stop(), 0);
      final stopped = runtime.snapshot();
      expect(stopped.phase, ClientRuntimePhase.stopped);
      expect(stopped.generation, 0);
    },
    skip: _libraryPath.isEmpty
        ? 'Set VELUM_CLIENT_LIBRARY to a built velum-client-ffi library.'
        : false,
  );
}
