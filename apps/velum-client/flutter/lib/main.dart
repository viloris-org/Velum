import 'dart:async';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'native_client.dart';

void main() {
  runApp(const VelumClientApp());
}

class VelumClientApp extends StatelessWidget {
  const VelumClientApp({super.key});

  @override
  Widget build(BuildContext context) {
    const ink = Color(0xff171717);
    const gold = Color(0xffb78b1d);
    return MaterialApp(
      title: 'Velum Client',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: gold,
          brightness: Brightness.light,
        ),
        scaffoldBackgroundColor: const Color(0xfff7f7f5),
        useMaterial3: true,
        textTheme: const TextTheme(
          headlineMedium: TextStyle(color: ink, fontWeight: FontWeight.w700),
          titleLarge: TextStyle(color: ink, fontWeight: FontWeight.w600),
          titleMedium: TextStyle(color: ink, fontWeight: FontWeight.w600),
          bodyMedium: TextStyle(color: Color(0xff404040)),
        ),
      ),
      home: const ClientHome(),
    );
  }
}

enum ConnectionStateView { disconnected, connecting, connected, failed }

class ClientHome extends StatefulWidget {
  const ClientHome({super.key});

  @override
  State<ClientHome> createState() => _ClientHomeState();
}

class _ClientHomeState extends State<ClientHome> {
  final _formKey = GlobalKey<FormState>();
  final _library = TextEditingController(text: DirectClient.defaultLibraryName());
  final _relay = TextEditingController(text: '127.0.0.1:4433');
  final _serverName = TextEditingController(text: 'localhost');
  final _certificate = TextEditingController();
  final _credential = TextEditingController();
  final List<String> _logs = ['Ready. Configure a relay before connecting.'];

  DirectClient? _client;
  ConnectionStateView _connection = ConnectionStateView.disconnected;

  @override
  void dispose() {
    _client?.close();
    for (final controller in [
      _library,
      _relay,
      _serverName,
      _certificate,
      _credential,
    ]) {
      controller.dispose();
    }
    super.dispose();
  }

  Future<void> _toggleConnection() async {
    if (_connection == ConnectionStateView.connected ||
        _connection == ConnectionStateView.connecting) {
      _client?.close();
      setState(() {
        _client = null;
        _connection = ConnectionStateView.disconnected;
        _appendLog('Disconnected by user.');
      });
      return;
    }
    if (!_formKey.currentState!.validate()) {
      return;
    }
    setState(() {
      _connection = ConnectionStateView.connecting;
      _appendLog('Connecting through the native client API.');
    });
    try {
      final request = await _connectionRequest();
      final handle = await Isolate.run(() => _connectNative(request));
      if (!mounted) return;
      setState(() {
        _client = DirectClient.attach(request.libraryPath, handle);
        _connection = ConnectionStateView.connected;
        _appendLog('Connected through the native client API.');
      });
    } on DirectClientException catch (error) {
      setState(() {
        _connection = ConnectionStateView.failed;
        _appendLog(error.toString());
      });
    } on FileSystemException catch (error) {
      setState(() {
        _connection = ConnectionStateView.failed;
        _appendLog('Cannot read client configuration: ${error.message}');
      });
    } on FormatException catch (error) {
      setState(() {
        _connection = ConnectionStateView.failed;
        _appendLog('Invalid credential file: ${error.message}');
      });
    }
  }

  Future<_NativeConnectionRequest> _connectionRequest() async =>
      _NativeConnectionRequest(
        libraryPath: _library.text.trim(),
        relayAddress: _relay.text.trim(),
        serverName: _serverName.text.trim(),
        credential: _decodeCredential(await File(_credential.text.trim()).readAsString()),
        certificatePem: await File(_certificate.text.trim()).readAsBytes(),
      );

  void _appendLog(String message) {
    _logs.insert(0, '${TimeOfDay.now().format(context)}  $message');
    if (_logs.length > 20) _logs.removeLast();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final wide = constraints.maxWidth >= 980;
            final content = [
              Expanded(flex: 6, child: _overview()),
              const SizedBox(width: 20, height: 20),
              Expanded(flex: 5, child: _configuration()),
            ];
            if (wide) {
              return Padding(
                padding: const EdgeInsets.all(24),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: content,
                ),
              );
            }
            return ListView(
              padding: const EdgeInsets.all(24),
              children: [
                _overview(compact: true),
                const SizedBox(height: 20),
                _configuration(embedded: true),
              ],
            );
          },
        ),
      ),
    );
  }

  Widget _overview({bool compact = false}) {
    final (label, color, icon) = switch (_connection) {
      ConnectionStateView.disconnected => (
        'Disconnected',
        const Color(0xff5b5b5b),
        Icons.power_off_outlined,
      ),
      ConnectionStateView.connecting => (
        'Connecting',
        const Color(0xffa36b00),
        Icons.sync,
      ),
      ConnectionStateView.connected => (
        'Connected',
        const Color(0xff167244),
        Icons.check_circle_outline,
      ),
      ConnectionStateView.failed => (
        'Connection failed',
        const Color(0xffb42318),
        Icons.error_outline,
      ),
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          'VELUM',
          style: TextStyle(fontWeight: FontWeight.w800, letterSpacing: 2.4),
        ),
        const SizedBox(height: 26),
        const Text(
          'Client control',
          style: TextStyle(fontSize: 32, fontWeight: FontWeight.w700),
        ),
        const SizedBox(height: 8),
        const Text('Experimental QUIC relay through the direct native client API.'),
        const SizedBox(height: 28),
        _statusPanel(label, color, icon),
        const SizedBox(height: 16),
        FilledButton.icon(
          onPressed: _toggleConnection,
          icon: Icon(
            _connection == ConnectionStateView.connected ||
                    _connection == ConnectionStateView.connecting
                ? Icons.stop_circle_outlined
                : Icons.play_arrow_outlined,
          ),
          label: Text(
            _connection == ConnectionStateView.connected ||
                    _connection == ConnectionStateView.connecting
                ? 'Disconnect'
                : 'Connect',
          ),
          style: FilledButton.styleFrom(minimumSize: const Size.fromHeight(48)),
        ),
        const SizedBox(height: 20),
        const Text('Activity'),
        const SizedBox(height: 8),
        compact
            ? SizedBox(height: 220, child: _activityLog())
            : Expanded(child: _activityLog()),
      ],
    );
  }

  Widget _activityLog() {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: Colors.white,
        border: Border.all(color: const Color(0xffdfdfdf)),
      ),
      child: ListView.separated(
        padding: const EdgeInsets.all(14),
        itemCount: _logs.length,
        separatorBuilder: (_, _) => const Divider(height: 16),
        itemBuilder: (_, index) => Text(
          _logs[index],
          style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
        ),
      ),
    );
  }

  Widget _statusPanel(String label, Color color, IconData icon) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: Colors.white,
        border: Border.all(color: const Color(0xffdfdfdf)),
      ),
      child: Padding(
        padding: const EdgeInsets.all(18),
        child: Row(
          children: [
            Icon(icon, color: color, size: 30),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    label,
                    style: const TextStyle(
                      fontWeight: FontWeight.w700,
                      fontSize: 18,
                    ),
                  ),
                  Text(
                    _connection == ConnectionStateView.connected
                        ? 'Flutter holds a direct native client session.'
                        : 'No traffic is routed while disconnected.',
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _configuration({bool embedded = false}) {
    final fields = [
      const Text(
        'Relay configuration',
        style: TextStyle(fontSize: 20, fontWeight: FontWeight.w700),
      ),
      const SizedBox(height: 6),
      const Text(
        'Credentials stay in the referenced file and are never displayed here.',
      ),
      const SizedBox(height: 20),
      _field(
        _relay,
        'Relay address',
        'IP address and UDP port, e.g. 203.0.113.10:4433',
      ),
      _field(
        _serverName,
        'TLS server name',
        'Certificate name presented by the relay',
      ),
      _field(
        _certificate,
        'CA certificate file',
        'PEM file used to verify the relay',
      ),
      _field(
        _credential,
        'Credential file',
        'Hexadecimal credential supplied by the operator',
      ),
      _field(
        _library,
        'Native client library',
        'Path to libvelum_client_ffi for this desktop platform',
      ),
      const SizedBox(height: 12),
      const Text(
        'This is an experimental Stage 2 direct client API. It is not a production VPN and supports IP-address targets only.',
        style: TextStyle(color: Color(0xff7a4b00)),
      ),
    ];
    return Form(
      key: _formKey,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: Colors.white,
          border: Border.all(color: const Color(0xffdfdfdf)),
        ),
        child: embedded
            ? Padding(
                padding: const EdgeInsets.all(20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: fields,
                ),
              )
            : ListView(padding: const EdgeInsets.all(20), children: fields),
      ),
    );
  }

  Widget _field(TextEditingController controller, String label, String helper) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: TextFormField(
        controller: controller,
        enabled:
            _connection != ConnectionStateView.connected &&
            _connection != ConnectionStateView.connecting,
        decoration: InputDecoration(
          labelText: label,
          helperText: helper,
          border: const OutlineInputBorder(),
        ),
        validator: (value) => value == null || value.trim().isEmpty
            ? '$label is required.'
            : null,
      ),
    );
  }
}

class _NativeConnectionRequest {
  const _NativeConnectionRequest({
    required this.libraryPath,
    required this.relayAddress,
    required this.serverName,
    required this.credential,
    required this.certificatePem,
  });

  final String libraryPath;
  final String relayAddress;
  final String serverName;
  final Uint8List credential;
  final Uint8List certificatePem;
}

int _connectNative(_NativeConnectionRequest request) =>
    DirectClient.connect(
      DirectClientConfiguration(
        libraryPath: request.libraryPath,
        relayAddress: request.relayAddress,
        serverName: request.serverName,
        credential: request.credential,
        certificatePem: request.certificatePem,
      ),
    ).handle;

Uint8List _decodeCredential(String value) {
  final encoded = value.trim();
  if (encoded.isEmpty || encoded.length.isOdd) {
    throw const FormatException('credential must contain hexadecimal byte pairs');
  }
  final credential = Uint8List(encoded.length ~/ 2);
  for (var index = 0; index < credential.length; index += 1) {
    final pair = encoded.substring(index * 2, index * 2 + 2);
    final byte = int.tryParse(pair, radix: 16);
    if (byte == null) {
      throw const FormatException('credential must contain hexadecimal byte pairs');
    }
    credential[index] = byte;
  }
  return credential;
}
