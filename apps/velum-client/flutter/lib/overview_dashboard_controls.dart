import 'package:flutter/material.dart';

import 'client_theme.dart';
import 'traffic_configuration.dart';
import 'traffic_mode_controller.dart';

class DashboardAdapterSwitch extends StatelessWidget {
  const DashboardAdapterSwitch({
    required this.label,
    required this.icon,
    required this.mode,
    required this.selectedMode,
    required this.available,
    required this.busy,
    required this.onModeChanged,
    super.key,
  });

  final String label;
  final IconData icon;
  final TrafficMode mode;
  final TrafficMode selectedMode;
  final bool available;
  final bool busy;
  final ValueChanged<TrafficMode>? onModeChanged;

  @override
  Widget build(BuildContext context) {
    final selected = selectedMode == mode;
    final enabled = available && !busy && onModeChanged != null;
    return _DashboardPanel(
      padding: const EdgeInsets.fromLTRB(14, 10, 10, 10),
      child: Row(
        children: [
          Icon(icon, size: 18, color: ClientTheme.text),
          const SizedBox(width: 9),
          Expanded(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontWeight: FontWeight.w700),
                ),
                const SizedBox(height: 5),
                Text(
                  available ? 'Options' : 'Unavailable',
                  style: const TextStyle(
                    color: ClientTheme.muted,
                    fontSize: 11,
                  ),
                ),
              ],
            ),
          ),
          Switch(
            value: selected,
            onChanged: enabled
                ? (value) => onModeChanged!(value ? mode : TrafficMode.off)
                : null,
          ),
        ],
      ),
    );
  }
}

class DashboardOutboundMode extends StatelessWidget {
  const DashboardOutboundMode({
    required this.mode,
    required this.onChanged,
    super.key,
  });

  final RoutingMode mode;
  final ValueChanged<RoutingMode>? onChanged;

  @override
  Widget build(BuildContext context) => _DashboardPanel(
    padding: const EdgeInsets.all(14),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const _PanelTitle(
          icon: Icons.call_split_rounded,
          label: 'Outbound mode',
        ),
        const SizedBox(height: 8),
        RadioGroup<RoutingMode>(
          groupValue: mode,
          onChanged: (selected) {
            if (selected != null) onChanged?.call(selected);
          },
          child: Column(
            children: [
              _modeOption('Rule', RoutingMode.rule),
              _modeOption('Global', RoutingMode.global),
              _modeOption('Direct', RoutingMode.direct),
            ],
          ),
        ),
      ],
    ),
  );

  Widget _modeOption(String label, RoutingMode value) => SizedBox(
    height: 32,
    child: Row(
      children: [
        Radio<RoutingMode>(value: value),
        const SizedBox(width: 2),
        Text(label, style: const TextStyle(fontWeight: FontWeight.w600)),
      ],
    ),
  );
}

class DashboardPanel extends _DashboardPanel {
  const DashboardPanel({required super.child, super.key, super.padding});
}

class DashboardPanelTitle extends _PanelTitle {
  const DashboardPanelTitle({
    required super.icon,
    required super.label,
    super.key,
  });
}

class _DashboardPanel extends StatelessWidget {
  const _DashboardPanel({required this.child, this.padding, super.key});

  final Widget child;
  final EdgeInsetsGeometry? padding;

  @override
  Widget build(BuildContext context) => Material(
    color: ClientTheme.panel.withValues(alpha: .88),
    shape: RoundedRectangleBorder(
      side: const BorderSide(color: ClientTheme.borderStrong),
      borderRadius: BorderRadius.circular(12),
    ),
    child: Padding(padding: padding ?? const EdgeInsets.all(14), child: child),
  );
}

class _PanelTitle extends StatelessWidget {
  const _PanelTitle({required this.icon, required this.label, super.key});

  final IconData icon;
  final String label;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      Icon(icon, size: 18, color: ClientTheme.text),
      const SizedBox(width: 8),
      Expanded(
        child: Text(
          label,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(fontWeight: FontWeight.w700),
        ),
      ),
    ],
  );
}
