import 'package:flutter/material.dart';

import 'client_theme.dart';
import 'traffic_configuration.dart';

class RoutingRulesPanel extends StatelessWidget {
  const RoutingRulesPanel({
    required this.draft,
    required this.onChanged,
    super.key,
  });

  final TrafficConfigurationDraft draft;
  final VoidCallback onChanged;

  @override
  Widget build(BuildContext context) => ClientPanel(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Desktop proxy rules',
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 14),
        SegmentedButton<RoutingMode>(
          showSelectedIcon: false,
          segments: const [
            ButtonSegment(
              value: RoutingMode.rule,
              label: Text('Rule'),
              icon: Icon(Icons.rule_outlined, size: 17),
            ),
            ButtonSegment(
              value: RoutingMode.global,
              label: Text('Global'),
              icon: Icon(Icons.public_outlined, size: 17),
            ),
            ButtonSegment(
              value: RoutingMode.direct,
              label: Text('Direct'),
              icon: Icon(Icons.arrow_outward, size: 17),
            ),
          ],
          selected: {draft.routingMode},
          onSelectionChanged: (selection) {
            draft.routingMode = selection.single;
            onChanged();
          },
        ),
        if (draft.routingMode == RoutingMode.rule) ...[
          const SizedBox(height: 16),
          TextFormField(
            key: const ValueKey('routing-rules'),
            controller: draft.rules,
            minLines: 7,
            maxLines: 12,
            onChanged: (_) => onChanged(),
            style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
            decoration: const InputDecoration(
              labelText: 'Ordered rules',
              alignLabelWithHint: true,
              border: OutlineInputBorder(),
            ),
          ),
        ],
      ],
    ),
  );
}
