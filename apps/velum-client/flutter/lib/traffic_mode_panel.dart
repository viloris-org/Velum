import 'package:flutter/material.dart';

import 'client_theme.dart';
import 'traffic_mode_controller.dart';

class TrafficModePanel extends StatelessWidget {
  const TrafficModePanel({
    required this.availableModes,
    required this.selectedMode,
    required this.activeMode,
    required this.phase,
    required this.runtimeOnline,
    required this.onModeChanged,
    super.key,
    this.error,
    this.compact = false,
  });

  final Set<TrafficMode> availableModes;
  final TrafficMode selectedMode;
  final TrafficMode activeMode;
  final TrafficModePhase phase;
  final bool runtimeOnline;
  final ValueChanged<TrafficMode>? onModeChanged;
  final String? error;
  final bool compact;

  bool get _busy => phase == TrafficModePhase.applying;

  @override
  Widget build(BuildContext context) => ClientPanel(
    padding: EdgeInsets.all(compact ? 20 : 24),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Container(
              width: 34,
              height: 34,
              decoration: BoxDecoration(
                color: ClientTheme.accent.withValues(alpha: .10),
                borderRadius: BorderRadius.circular(8),
              ),
              child: const Icon(
                Icons.route_outlined,
                size: 18,
                color: ClientTheme.accent,
              ),
            ),
            const SizedBox(width: 11),
            Expanded(
              child: Text(
                'Traffic routing',
                style: Theme.of(context).textTheme.titleMedium,
              ),
            ),
            _ModeStatus(phase: phase, activeMode: activeMode),
          ],
        ),
        const SizedBox(height: 18),
        SizedBox(
          width: double.infinity,
          child: SegmentedButton<TrafficMode>(
            showSelectedIcon: false,
            segments: [
              const ButtonSegment(
                value: TrafficMode.off,
                label: Text('Off'),
                icon: Icon(Icons.power_settings_new, size: 17),
              ),
              if (availableModes.contains(TrafficMode.systemProxy))
                const ButtonSegment(
                  value: TrafficMode.systemProxy,
                  label: Text('Proxy'),
                  icon: Icon(Icons.language, size: 17),
                ),
              if (availableModes.contains(TrafficMode.tun))
                const ButtonSegment(
                  value: TrafficMode.tun,
                  label: Text('TUN'),
                  icon: Icon(Icons.vpn_lock_outlined, size: 17),
                ),
            ],
            selected: {selectedMode},
            onSelectionChanged: _busy || onModeChanged == null
                ? null
                : (selection) => onModeChanged!(selection.single),
          ),
        ),
        const SizedBox(height: 14),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (_busy)
              const SizedBox(
                width: 17,
                height: 17,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            else
              Icon(
                phase == TrafficModePhase.failed
                    ? Icons.error_outline
                    : phase == TrafficModePhase.active
                    ? Icons.check_circle_outline
                    : Icons.pause_circle_outline,
                size: 17,
                color: phase == TrafficModePhase.failed
                    ? ClientTheme.danger
                    : phase == TrafficModePhase.active
                    ? ClientTheme.accent
                    : ClientTheme.muted,
              ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                _statusText(),
                style: const TextStyle(color: ClientTheme.muted, fontSize: 12),
              ),
            ),
          ],
        ),
      ],
    ),
  );

  String _statusText() {
    if (error case final message?) return message;
    if (_busy) return 'Applying routing changes...';
    if (phase == TrafficModePhase.active) {
      return activeMode == TrafficMode.tun
          ? 'Device traffic is using the TUN adapter.'
          : 'Compatible applications are using the system proxy.';
    }
    if (selectedMode != TrafficMode.off && !runtimeOnline) {
      return 'Selected mode will activate when the relay is online.';
    }
    if (availableModes.length == 1) {
      return 'No traffic adapter is available on this platform.';
    }
    return 'OS traffic routing is off.';
  }
}

class _ModeStatus extends StatelessWidget {
  const _ModeStatus({required this.phase, required this.activeMode});

  final TrafficModePhase phase;
  final TrafficMode activeMode;

  @override
  Widget build(BuildContext context) {
    final active = phase == TrafficModePhase.active;
    final label = active
        ? activeMode == TrafficMode.tun
              ? 'TUN active'
              : 'Proxy active'
        : phase == TrafficModePhase.applying
        ? 'Applying'
        : phase == TrafficModePhase.failed
        ? 'Failed'
        : 'Inactive';
    final color = active
        ? ClientTheme.accent
        : phase == TrafficModePhase.failed
        ? ClientTheme.danger
        : ClientTheme.muted;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
      decoration: BoxDecoration(
        color: color.withValues(alpha: .09),
        border: Border.all(color: color.withValues(alpha: .20)),
        borderRadius: BorderRadius.circular(20),
      ),
      child: Text(
        label.toUpperCase(),
        style: TextStyle(
          color: color,
          fontSize: 9,
          fontWeight: FontWeight.w700,
        ),
      ),
    );
  }
}
