import 'dart:io';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:liquid_glass_bridge/liquid_glass_bridge.dart';

import 'client_theme.dart';

class ClientCompactNavigation extends StatelessWidget {
  const ClientCompactNavigation({
    required this.selectedIndex,
    required this.onSelected,
    super.key,
  });

  final int selectedIndex;
  final ValueChanged<int> onSelected;

  @override
  Widget build(BuildContext context) {
    final navigation = _navigation(context);
    if (Platform.isAndroid) {
      return LiquidGlassSurface(
        mode: LiquidGlassMode.androidNative,
        quality: LiquidGlassQuality.medium,
        borderRadius: BorderRadius.zero,
        padding: const EdgeInsets.all(5),
        elevation: 2,
        tintColor: ClientTheme.panel,
        tintOpacity: .20,
        blurSigma: 12,
        borderColor: ClientTheme.borderStrong.withValues(alpha: .72),
        highlightStrength: .20,
        noiseOpacity: .01,
        child: navigation,
      );
    }
    return _desktopNavigation(navigation);
  }

  Widget _desktopNavigation(Widget navigation) => ClipRRect(
    borderRadius: BorderRadius.zero,
    child: BackdropFilter(
      filter: ImageFilter.blur(sigmaX: 12, sigmaY: 12),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: ClientTheme.panel.withValues(alpha: .62),
          border: Border.all(
            color: ClientTheme.borderStrong.withValues(alpha: .72),
          ),
          borderRadius: BorderRadius.zero,
          boxShadow: const [],
        ),
        child: Padding(padding: const EdgeInsets.all(5), child: navigation),
      ),
    ),
  );

  Widget _navigation(BuildContext context) => Semantics(
    label: 'Primary navigation',
    child: Row(
      children: [
        _CompactDestination(
          icon: Icons.radar_outlined,
          label: 'Overview',
          selected: selectedIndex == 0,
          onTap: () => onSelected(0),
        ),
        _CompactDestination(
          icon: Icons.hub_outlined,
          label: 'Nodes',
          selected: selectedIndex == 1,
          onTap: () => onSelected(1),
        ),
        _CompactDestination(
          icon: Icons.tune_outlined,
          label: 'Config',
          selected: selectedIndex == 2,
          onTap: () => onSelected(2),
        ),
        _CompactDestination(
          icon: Icons.settings_outlined,
          label: 'Settings',
          selected: selectedIndex == 3,
          onTap: () => onSelected(3),
        ),
      ],
    ),
  );
}

class _CompactDestination extends StatelessWidget {
  const _CompactDestination({
    required this.icon,
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Expanded(
    child: Semantics(
      button: true,
      selected: selected,
      label: label,
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(17),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(17),
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 180),
            curve: Curves.easeOutCubic,
            height: 58,
            decoration: BoxDecoration(
              color: selected
                  ? ClientTheme.accent.withValues(alpha: .13)
                  : Colors.transparent,
              border: selected
                  ? Border.all(color: ClientTheme.accent.withValues(alpha: .18))
                  : null,
              borderRadius: BorderRadius.circular(17),
            ),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Icon(
                  icon,
                  size: 21,
                  color: selected ? ClientTheme.accent : ClientTheme.muted,
                ),
                const SizedBox(height: 3),
                Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: selected ? ClientTheme.text : ClientTheme.muted,
                    fontSize: 10,
                    fontWeight: selected ? FontWeight.w700 : FontWeight.w600,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    ),
  );
}
