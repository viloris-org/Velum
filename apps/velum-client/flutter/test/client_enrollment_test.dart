import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:velum_client/client_enrollment.dart';

void main() {
  String enrollment({String trustMode = 'system'}) => jsonEncode({
    'kind': 'velum-enrollment',
    'version': 1,
    'node': {
      'id': 'relay-2',
      'name': 'Relay',
      'relay-address': '203.0.113.10:4433',
      'server-name': 'relay.example',
    },
    'principal-id': 2,
    'credential': '09' * 32,
    'trust': trustMode == 'system'
        ? {'mode': 'system'}
        : {
            'mode': 'custom-ca',
            'certificate-pem': '-----BEGIN CERTIFICATE-----\nAA==\n',
          },
  });

  test('canonical enrollment projects secure-storage references', () {
    final parsed = ClientEnrollment.parseCanonical(enrollment());

    expect(parsed.nodeId, 'relay-2');
    expect(parsed.credential, hasLength(32));
    expect(parsed.credentialRef, 'secret://velum/enrollment/relay-2/2');
    expect(parsed.certificateRef, isNull);
  });

  test('custom CA enrollment derives a separate certificate reference', () {
    final parsed = ClientEnrollment.parseCanonical(
      enrollment(trustMode: 'custom-ca'),
    );

    expect(parsed.trustMode, 'custom-ca');
    expect(parsed.certificateRef, 'secret://velum/enrollment/relay-2/2/ca');
  });

  test('rejects unknown fields and weak credentials', () {
    final unknown = jsonDecode(enrollment()) as Map<String, Object?>;
    unknown['extra'] = true;
    expect(
      () => ClientEnrollment.parseCanonical(jsonEncode(unknown)),
      throwsFormatException,
    );

    final weak = jsonDecode(enrollment()) as Map<String, Object?>;
    weak['credential'] = '09';
    expect(
      () => ClientEnrollment.parseCanonical(jsonEncode(weak)),
      throwsFormatException,
    );
  });

  test('redacted enrollment repository survives restart and upserts', () async {
    final directory = await Directory.systemTemp.createTemp(
      'velum-enrollment-test-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final repository = EnrollmentRepository(
      File('${directory.path}/enrollments.json'),
    );
    final first = ClientEnrollment.parseCanonical(enrollment()).installedNode;

    await repository.upsert(first);
    await repository.upsert(first);
    final restored = await EnrollmentRepository(repository.file).load();

    expect(restored, hasLength(1));
    expect(restored.single.nodeId, 'relay-2');
    expect(restored.single.credentialRef, contains('secret://velum/'));
    expect(await repository.file.readAsString(), isNot(contains('090909')));

    await repository.remove('relay-2');
    expect(await repository.load(), isEmpty);
  });
}
