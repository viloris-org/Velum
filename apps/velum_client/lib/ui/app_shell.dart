import 'package:flutter/material.dart';

import '../controller/velum_controller.dart';
import '../theme/velum_theme.dart';
import 'pages/activity_page.dart';
import 'pages/configuration_page.dart';
import 'pages/overview_page.dart';
import 'pages/sessions_page.dart';
import 'pages/settings_page.dart';
import 'widgets/common.dart';

class AppShell extends StatelessWidget {
  const AppShell({super.key, required this.controller});

  final VelumController controller;

  static const _destinations = [
    (
      icon: Icons.space_dashboard_outlined,
      selected: Icons.space_dashboard,
      label: '总览',
    ),
    (icon: Icons.route_outlined, selected: Icons.route, label: '会话'),
    (icon: Icons.tune_outlined, selected: Icons.tune, label: '配置'),
    (
      icon: Icons.receipt_long_outlined,
      selected: Icons.receipt_long,
      label: '活动',
    ),
    (icon: Icons.settings_outlined, selected: Icons.settings, label: '设置'),
  ];

  @override
  Widget build(BuildContext context) {
    final pages = [
      OverviewPage(controller: controller),
      SessionsPage(controller: controller),
      ConfigurationPage(controller: controller),
      ActivityPage(controller: controller),
      SettingsPage(controller: controller),
    ];
    return LayoutBuilder(
      builder: (context, constraints) {
        final desktop = constraints.maxWidth >= 920;
        return Scaffold(
          body: Row(
            children: [
              if (desktop) _DesktopNavigation(controller: controller),
              Expanded(
                child: Column(
                  children: [
                    _TopBar(controller: controller, compact: !desktop),
                    Expanded(
                      child: Stack(
                        children: [
                          Positioned.fill(
                            child: DecoratedBox(
                              decoration: const BoxDecoration(
                                gradient: RadialGradient(
                                  center: Alignment(0.72, -0.8),
                                  radius: 1.05,
                                  colors: [
                                    Color(0x152E817E),
                                    Colors.transparent,
                                  ],
                                ),
                              ),
                            ),
                          ),
                          Positioned.fill(
                            child: IndexedStack(
                              index: controller.selectedIndex,
                              children: pages,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          bottomNavigationBar: desktop
              ? null
              : NavigationBar(
                  selectedIndex: controller.selectedIndex,
                  onDestinationSelected: controller.selectPage,
                  backgroundColor: VelumColors.deep,
                  indicatorColor: VelumColors.panelRaised,
                  destinations: [
                    for (final item in _destinations)
                      NavigationDestination(
                        icon: Icon(item.icon),
                        selectedIcon: Icon(
                          item.selected,
                          color: VelumColors.aqua,
                        ),
                        label: item.label,
                      ),
                  ],
                ),
        );
      },
    );
  }
}

class _DesktopNavigation extends StatelessWidget {
  const _DesktopNavigation({required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 238,
      decoration: const BoxDecoration(
        color: VelumColors.deep,
        border: Border(right: BorderSide(color: VelumColors.line)),
      ),
      child: SafeArea(
        child: Column(
          children: [
            const Padding(
              padding: EdgeInsets.fromLTRB(24, 23, 20, 24),
              child: _Brand(),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Column(
                children: [
                  for (
                    var index = 0;
                    index < AppShell._destinations.length;
                    index++
                  )
                    _NavItem(
                      icon: controller.selectedIndex == index
                          ? AppShell._destinations[index].selected
                          : AppShell._destinations[index].icon,
                      label: AppShell._destinations[index].label,
                      selected: controller.selectedIndex == index,
                      onTap: () => controller.selectPage(index),
                    ),
                ],
              ),
            ),
            const Spacer(),
            Padding(
              padding: const EdgeInsets.all(16),
              child: Container(
                padding: const EdgeInsets.all(14),
                decoration: BoxDecoration(
                  color: VelumColors.ink.withValues(alpha: 0.55),
                  borderRadius: BorderRadius.circular(14),
                  border: Border.all(color: VelumColors.line),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Icon(
                          controller.demoMode
                              ? Icons.science_outlined
                              : Icons.terminal,
                          size: 16,
                          color: controller.demoMode
                              ? VelumColors.amber
                              : VelumColors.aqua,
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            controller.demoMode ? '演示运行时' : '本机运行时',
                            style: Theme.of(context).textTheme.titleMedium,
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 7),
                    Text(
                      controller.bridge.adapterName,
                      style: Theme.of(
                        context,
                      ).textTheme.bodySmall?.copyWith(color: VelumColors.muted),
                    ),
                    const SizedBox(height: 12),
                    StatusPill(
                      label: controller.demoMode
                          ? 'SAFE PREVIEW'
                          : 'CLI CONNECTED',
                      color: controller.demoMode
                          ? VelumColors.amber
                          : VelumColors.aqua,
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Brand extends StatelessWidget {
  const _Brand();

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Container(
          width: 38,
          height: 38,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(11),
            color: VelumColors.aqua.withValues(alpha: 0.1),
            border: Border.all(color: VelumColors.aqua.withValues(alpha: 0.42)),
          ),
          child: const CustomPaint(painter: _MarkPainter()),
        ),
        const SizedBox(width: 12),
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'VELUM',
              style: Theme.of(context).textTheme.titleLarge?.copyWith(
                letterSpacing: 2.4,
                fontFamily: 'Consolas',
              ),
            ),
            Text(
              'OPERATOR CONSOLE',
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                color: VelumColors.muted,
                fontSize: 8,
                letterSpacing: 1.35,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _MarkPainter extends CustomPainter {
  const _MarkPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = VelumColors.aqua
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.7
      ..strokeCap = StrokeCap.round;
    final path = Path()
      ..moveTo(size.width * 0.22, size.height * 0.28)
      ..quadraticBezierTo(
        size.width * 0.45,
        size.height * 0.82,
        size.width * 0.57,
        size.height * 0.5,
      )
      ..quadraticBezierTo(
        size.width * 0.68,
        size.height * 0.2,
        size.width * 0.81,
        size.height * 0.7,
      );
    canvas.drawPath(path, paint);
    canvas.drawCircle(
      Offset(size.width * 0.57, size.height * 0.5),
      2.6,
      Paint()..color = VelumColors.amber,
    );
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

class _NavItem extends StatelessWidget {
  const _NavItem({
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
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 5),
      child: Material(
        color: selected ? VelumColors.panelRaised : Colors.transparent,
        borderRadius: BorderRadius.circular(12),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(12),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 13, vertical: 12),
            child: Row(
              children: [
                Icon(
                  icon,
                  size: 19,
                  color: selected ? VelumColors.aqua : VelumColors.muted,
                ),
                const SizedBox(width: 13),
                Text(
                  label,
                  style: Theme.of(context).textTheme.labelLarge?.copyWith(
                    color: selected ? VelumColors.mist : VelumColors.muted,
                  ),
                ),
                const Spacer(),
                if (selected)
                  Container(
                    width: 4,
                    height: 4,
                    decoration: const BoxDecoration(
                      shape: BoxShape.circle,
                      color: VelumColors.aqua,
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _TopBar extends StatelessWidget {
  const _TopBar({required this.controller, required this.compact});

  final VelumController controller;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 72,
      padding: EdgeInsets.symmetric(horizontal: compact ? 16 : 28),
      decoration: BoxDecoration(
        color: VelumColors.ink.withValues(alpha: 0.82),
        border: const Border(bottom: BorderSide(color: VelumColors.line)),
      ),
      child: SafeArea(
        bottom: false,
        child: Row(
          children: [
            if (compact) ...[
              const _Brand(),
              const Spacer(),
            ] else ...[
              Text(
                '研究级加密隧道 · 运维工作台',
                style: Theme.of(
                  context,
                ).textTheme.bodyMedium?.copyWith(color: VelumColors.muted),
              ),
              const Spacer(),
            ],
            if (!compact)
              StatusPill(
                label: controller.demoMode ? '演示模式' : '本机 CLI',
                color: controller.demoMode
                    ? VelumColors.amber
                    : VelumColors.aqua,
                icon: controller.demoMode
                    ? Icons.science_outlined
                    : Icons.terminal,
              ),
            if (!compact) const SizedBox(width: 12),
            IconButton(
              tooltip: '刷新状态',
              onPressed: controller.busy ? null : controller.refreshStatus,
              icon: controller.busy
                  ? const SizedBox.square(
                      dimension: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.refresh_rounded),
            ),
            const SizedBox(width: 4),
            Container(
              width: 34,
              height: 34,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: VelumColors.panelRaised,
                border: Border.all(color: VelumColors.line),
              ),
              child: Text(
                'VL',
                style: Theme.of(context).textTheme.labelSmall?.copyWith(
                  color: VelumColors.aqua,
                  letterSpacing: 0.2,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
