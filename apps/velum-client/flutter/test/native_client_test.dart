import 'dart:ffi';

import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/native_client.dart';

void main() {
  test('runtime snapshot ABI uses the fixed 24-byte v1 layout', () {
    expect(sizeOf<VelumRuntimeSnapshotV1>(), 24);
  });
}
