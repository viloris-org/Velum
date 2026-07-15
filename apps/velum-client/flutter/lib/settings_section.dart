import 'package:flutter/material.dart';

import 'client_theme.dart';

class SettingsSection extends StatelessWidget {
  const SettingsSection({
    required this.eyebrow,
    required this.title,
    required this.description,
    required this.icon,
    required this.child,
    super.key,
  });

  final String eyebrow;
  final String title;
  final String description;
  final IconData icon;
  final Widget child;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            width: 36,
            height: 36,
            decoration: BoxDecoration(
              color: ClientTheme.accent.withValues(alpha: .10),
              border: Border.all(
                color: ClientTheme.accent.withValues(alpha: .22),
              ),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Icon(icon, color: ClientTheme.accent, size: 19),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  eyebrow.toUpperCase(),
                  style: const TextStyle(
                    color: ClientTheme.muted,
                    fontSize: 10,
                    fontWeight: FontWeight.w700,
                    letterSpacing: 1.1,
                  ),
                ),
                const SizedBox(height: 2),
                Text(title, style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 3),
                Text(
                  description,
                  style: const TextStyle(
                    color: ClientTheme.muted,
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
      const SizedBox(height: 12),
      child,
    ],
  );
}
