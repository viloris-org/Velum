import 'dart:io';

import 'package:flutter/material.dart';

import 'client_theme.dart';
import 'overview_dashboard_controls.dart';
import 'public_ip_service.dart';
import 'traffic_chart.dart';

class DashboardNetworkSpeed extends StatelessWidget {
  const DashboardNetworkSpeed({this.samples = const [], super.key});

  final List<TrafficSample> samples;

  @override
  Widget build(BuildContext context) {
    final latest = samples.isEmpty ? null : samples.last;
    return DashboardPanel(
      child: Column(
        children: [
          Row(
            children: [
              const Icon(Icons.speed_rounded, size: 18),
              const SizedBox(width: 8),
              const Expanded(
                child: Text(
                  'Network speed',
                  style: TextStyle(fontWeight: FontWeight.w700),
                ),
              ),
              Text(
                '↑ ${_rate(latest?.uploadBytesPerSecond ?? 0)}',
                style: const TextStyle(color: ClientTheme.muted, fontSize: 11),
              ),
              const SizedBox(width: 10),
              Text(
                '↓ ${_rate(latest?.downloadBytesPerSecond ?? 0)}',
                style: const TextStyle(color: ClientTheme.muted, fontSize: 11),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Expanded(
            child: TrafficChart(
              samples: samples,
              height: 110,
              showLegend: false,
              showAxisLabels: false,
            ),
          ),
        ],
      ),
    );
  }

  static String _rate(double bytesPerSecond) {
    if (bytesPerSecond < 1024) return '${bytesPerSecond.round()} B/s';
    return '${(bytesPerSecond / 1024).toStringAsFixed(1)} kB/s';
  }
}

class DashboardPublicIp extends StatefulWidget {
  const DashboardPublicIp({super.key});

  @override
  State<DashboardPublicIp> createState() => _DashboardPublicIpState();
}

class _DashboardPublicIpState extends State<DashboardPublicIp> {
  final _service = const PublicIpService();
  Future<PublicIpDetails>? _request;

  void _refresh() => setState(() => _request = _service.lookup());

  @override
  Widget build(BuildContext context) => DashboardPanel(
    padding: const EdgeInsets.fromLTRB(14, 10, 8, 10),
    child: Row(
      children: [
        const Icon(Icons.public_rounded, size: 18),
        const SizedBox(width: 9),
        Expanded(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Public IP',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 5),
              _value(),
            ],
          ),
        ),
        IconButton(
          tooltip: 'Refresh public IP',
          onPressed: _refresh,
          icon: const Icon(Icons.refresh_rounded, size: 18),
        ),
      ],
    ),
  );

  Widget _value() {
    final request = _request;
    if (request == null) return const _IpValue('--');
    return FutureBuilder<PublicIpDetails>(
      future: request,
      builder: (context, snapshot) {
        if (snapshot.connectionState != ConnectionState.done) {
          return const SizedBox(
            width: 14,
            height: 14,
            child: CircularProgressIndicator(strokeWidth: 2),
          );
        }
        if (!snapshot.hasData) return const _IpValue('Unavailable');
        return _IpValue(snapshot.data!.ip);
      },
    );
  }
}

class DashboardLocalIp extends StatefulWidget {
  const DashboardLocalIp({super.key});

  @override
  State<DashboardLocalIp> createState() => _DashboardLocalIpState();
}

class _DashboardLocalIpState extends State<DashboardLocalIp> {
  late final Future<String> _address = _loadAddress();

  Future<String> _loadAddress() async {
    final interfaces = await NetworkInterface.list(
      type: InternetAddressType.IPv4,
      includeLoopback: false,
    );
    for (final interface in interfaces) {
      if (interface.addresses.isNotEmpty) {
        return interface.addresses.first.address;
      }
    }
    return 'Unavailable';
  }

  @override
  Widget build(BuildContext context) => DashboardPanel(
    padding: const EdgeInsets.all(14),
    child: Row(
      children: [
        const Icon(Icons.devices_rounded, size: 18),
        const SizedBox(width: 9),
        Expanded(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Local IP',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 5),
              FutureBuilder<String>(
                future: _address,
                builder: (context, snapshot) => _IpValue(snapshot.data ?? '--'),
              ),
            ],
          ),
        ),
      ],
    ),
  );
}

class DashboardTrafficStats extends StatelessWidget {
  const DashboardTrafficStats({super.key});

  @override
  Widget build(BuildContext context) => const DashboardPanel(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        DashboardPanelTitle(
          icon: Icons.donut_large_rounded,
          label: 'Traffic stats',
        ),
        SizedBox(height: 14),
        Expanded(
          child: Row(
            children: [
              SizedBox(
                width: 52,
                height: 52,
                child: CircularProgressIndicator(
                  value: .72,
                  strokeWidth: 8,
                  backgroundColor: ClientTheme.mutedDark,
                  color: ClientTheme.text,
                ),
              ),
              SizedBox(width: 10),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    _TrafficValue(
                      color: ClientTheme.trafficUpload,
                      label: 'Upload',
                      value: '0 B',
                    ),
                    SizedBox(height: 12),
                    _TrafficValue(
                      color: ClientTheme.trafficDownload,
                      label: 'Download',
                      value: '0 B',
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ],
    ),
  );
}

class _IpValue extends StatelessWidget {
  const _IpValue(this.value);

  final String value;

  @override
  Widget build(BuildContext context) => Text(
    value,
    maxLines: 1,
    overflow: TextOverflow.ellipsis,
    style: const TextStyle(color: ClientTheme.muted, fontSize: 13),
  );
}

class _TrafficValue extends StatelessWidget {
  const _TrafficValue({
    required this.color,
    required this.label,
    required this.value,
  });

  final Color color;
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Row(
        children: [
          Container(
            width: 10,
            height: 5,
            decoration: BoxDecoration(
              color: color,
              borderRadius: BorderRadius.circular(3),
            ),
          ),
          const SizedBox(width: 5),
          Expanded(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 11),
            ),
          ),
        ],
      ),
      const SizedBox(height: 3),
      Text(
        value,
        maxLines: 1,
        style: const TextStyle(color: ClientTheme.muted, fontSize: 11),
      ),
    ],
  );
}
