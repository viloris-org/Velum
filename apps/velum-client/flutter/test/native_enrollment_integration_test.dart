import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/client_enrollment.dart';

void main() {
  final library = Platform.environment['VELUM_CLIENT_LIBRARY'];
  test(
    'Dart enrollment ABI validates and projects issued node material',
    () {
      final codec = NativeEnrollmentCodec.open(library!);
      final normalized = codec.normalize(
        jsonEncode({
          'kind': 'velum-enrollment',
          'version': 1,
          'node': {
            'id': 'relay-7',
            'name': 'Relay',
            'relay-address': '203.0.113.10:4433',
            'server-name': 'relay.example',
          },
          'principal-id': 7,
          'credential': '5a' * 32,
          'trust': {'mode': 'system'},
        }),
      );
      final enrollment = ClientEnrollment.parseCanonical(normalized);

      expect(enrollment.nodeId, 'relay-7');
      expect(enrollment.credential, hasLength(32));
      expect(enrollment.credentialRef, 'secret://velum/enrollment/relay-7/7');
    },
    skip: library == null
        ? 'Set VELUM_CLIENT_LIBRARY to a built velum-client-ffi library.'
        : false,
  );
}
