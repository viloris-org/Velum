import 'dart:convert';
import 'dart:io';

class PublicIpDetails {
  const PublicIpDetails({required this.ip, required this.organization});

  final String ip;
  final String organization;
}

class PublicIpService {
  const PublicIpService();

  static const _requestTimeout = Duration(seconds: 5);

  Future<PublicIpDetails> lookup() async {
    final client = HttpClient();
    try {
      final request = await client
          .getUrl(Uri.https('ipinfo.io', '/json'))
          .timeout(_requestTimeout);
      final response = await request.close().timeout(_requestTimeout);
      if (response.statusCode != HttpStatus.ok) {
        throw HttpException('IPinfo returned ${response.statusCode}.');
      }
      final body = await utf8.decoder
          .bind(response)
          .join()
          .timeout(_requestTimeout);
      final data = jsonDecode(body) as Map<String, dynamic>;
      final ip = data['ip'] as String?;
      if (ip == null || ip.isEmpty) throw const FormatException('Missing IP.');
      return PublicIpDetails(
        ip: ip,
        organization: data['org'] as String? ?? 'Network not identified',
      );
    } finally {
      client.close(force: true);
    }
  }
}
