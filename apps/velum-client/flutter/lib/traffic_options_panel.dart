import 'package:flutter/material.dart';

import 'client_theme.dart';
import 'traffic_configuration.dart';
import 'traffic_mode_controller.dart';

class TrafficOptionsPanel extends StatelessWidget {
  const TrafficOptionsPanel({
    required this.draft,
    required this.availableModes,
    required this.onChanged,
    super.key,
  });

  final TrafficConfigurationDraft draft;
  final Set<TrafficMode> availableModes;
  final VoidCallback onChanged;

  @override
  Widget build(BuildContext context) => ClientPanel(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (availableModes.contains(TrafficMode.systemProxy)) ...[
          Text('System proxy', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 14),
          _field(
            draft.proxyPort,
            'Local proxy port',
            '0 selects an available port',
            numeric: true,
          ),
          _field(
            draft.proxyBypass,
            'System proxy bypass',
            'One hostname or pattern per line',
            lines: 3,
          ),
        ],
        if (availableModes.contains(TrafficMode.tun)) ...[
          if (availableModes.contains(TrafficMode.systemProxy))
            const Divider(height: 32),
          Text('TUN VPN', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 14),
          LayoutBuilder(
            builder: (context, constraints) {
              final compact = constraints.maxWidth < 520;
              final fields = [
                _field(draft.tunAddress, 'TUN address', 'IPv4 address'),
                _field(draft.tunPrefixLength, 'Prefix', '0-32', numeric: true),
                _field(draft.tunMtu, 'MTU', '576-65535', numeric: true),
              ];
              if (compact) return Column(children: fields);
              return Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(flex: 2, child: fields[0]),
                  const SizedBox(width: 12),
                  Expanded(child: fields[1]),
                  const SizedBox(width: 12),
                  Expanded(child: fields[2]),
                ],
              );
            },
          ),
          _field(
            draft.tunDnsServers,
            'TUN DNS servers',
            'One IPv4 DNS server per line',
            lines: 2,
          ),
          _field(
            draft.tunRoutes,
            'TUN routes',
            'One IPv4 CIDR per line',
            lines: 3,
          ),
        ],
      ],
    ),
  );

  Widget _field(
    TextEditingController controller,
    String label,
    String helper, {
    int lines = 1,
    bool numeric = false,
  }) => Padding(
    padding: const EdgeInsets.only(bottom: 14),
    child: TextFormField(
      controller: controller,
      minLines: lines,
      maxLines: lines,
      keyboardType: numeric ? TextInputType.number : TextInputType.text,
      onChanged: (_) => onChanged(),
      decoration: InputDecoration(
        labelText: label,
        helperText: helper,
        alignLabelWithHint: lines > 1,
        border: const OutlineInputBorder(),
      ),
    ),
  );
}
