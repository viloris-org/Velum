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

  test(
    'Dart enrollment ABI projects a custom CA issued by the native codec',
    () {
      final codec = NativeEnrollmentCodec.open(library!);
      final normalized = codec.normalize(
        jsonEncode({
          'kind': 'velum-enrollment',
          'version': 1,
          'node': {
            'id': 'relay-8',
            'name': 'Relay',
            'relay-address': '203.0.113.10:4433',
            'server-name': 'relay.example',
          },
          'principal-id': 8,
          'credential': '5a' * 32,
          'trust': {
            'mode': 'custom-ca',
            'certificate_pem':
                '-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----\n',
          },
        }),
      );
      final enrollment = ClientEnrollment.parseCanonical(normalized);

      expect(enrollment.trustMode, 'custom-ca');
      expect(enrollment.certificatePem, contains('BEGIN CERTIFICATE'));
      expect(enrollment.certificateRef, 'secret://velum/enrollment/relay-8/8/ca');
    },
    skip: library == null
        ? 'Set VELUM_CLIENT_LIBRARY to a built velum-client-ffi library.'
        : false,
  );
}
