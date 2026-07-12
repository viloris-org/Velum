import 'package:flutter/material.dart';

import '../../controller/velum_controller.dart';
import '../../models/operator_models.dart';
import '../../theme/velum_theme.dart';
import '../widgets/common.dart';
import '../widgets/continuity_visual.dart';

class OverviewPage extends StatelessWidget {
  const OverviewPage({super.key, required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    final snapshot = controller.snapshot;
    final phase = _phaseInfo(snapshot.phase);
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(24, 28, 24, 48),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1320),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SectionHeading(
                eyebrow: 'Network continuity',
                title: '每一条路径都可以变化，\n会话不必重来。',
                description: '监控 Velum 中继、承载切换与本地运维控制。',
                trailing: StatusPill(label: phase.label, color: phase.color),
              ),
              const SizedBox(height: 24),
              _HeroPanel(controller: controller),
              const SizedBox(height: 18),
              LayoutBuilder(
                builder: (context, constraints) {
                  final columns = constraints.maxWidth >= 1000
                      ? 4
                      : constraints.maxWidth >= 570
                      ? 2
                      : 1;
                  const gap = 14.0;
                  final width =
                      (constraints.maxWidth - gap * (columns - 1)) / columns;
                  final cards = [
                    MetricCard(
                      label: '服务状态',
                      value: phase.label,
                      detail: snapshot.listener,
                      icon: Icons.hub_outlined,
                      accent: phase.color,
                    ),
                    MetricCard(
                      label: '运行时间',
                      value: _formatDuration(snapshot.uptime),
                      detail: '最近同步 ${_formatClock(snapshot.updatedAt)}',
                      icon: Icons.schedule_outlined,
                    ),
                    MetricCard(
                      label: '已接纳连接',
                      value: _compactNumber(snapshot.admittedConnections),
                      detail: '自本次启动以来',
                      icon: Icons.call_received_rounded,
                      accent: VelumColors.amber,
                    ),
                    MetricCard(
                      label: '活跃流',
                      value: snapshot.activeFlows.toString(),
                      detail: '跨逻辑会话聚合',
                      icon: Icons.swap_calls_rounded,
                      accent: VelumColors.aquaSoft,
                    ),
                  ];
                  return Wrap(
                    spacing: gap,
                    runSpacing: gap,
                    children: [
                      for (final card in cards)
                        SizedBox(width: width, child: card),
                    ],
                  );
                },
              ),
              const SizedBox(height: 18),
              LayoutBuilder(
                builder: (context, constraints) {
                  final wide = constraints.maxWidth >= 860;
                  final controls = _ControlPanel(controller: controller);
                  final events = _RecentEvents(controller: controller);
                  if (!wide) {
                    return Column(
                      children: [controls, const SizedBox(height: 18), events],
                    );
                  }
                  return Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(flex: 5, child: controls),
                      const SizedBox(width: 18),
                      Expanded(flex: 6, child: events),
                    ],
                  );
                },
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _HeroPanel extends StatelessWidget {
  const _HeroPanel({required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    return Card(
      color: VelumColors.panel,
      child: ClipRRect(
        borderRadius: BorderRadius.circular(18),
        child: LayoutBuilder(
          builder: (context, constraints) {
            final stacked = constraints.maxWidth < 760;
            final copy = Padding(
              padding: EdgeInsets.all(stacked ? 22 : 30),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      const StatusPill(
                        label: 'SESSION CONTINUITY',
                        color: VelumColors.aqua,
                      ),
                      const SizedBox(width: 9),
                      StatusPill(
                        label: controller.demoMode ? 'PREVIEW' : 'LIVE',
                        color: controller.demoMode
                            ? VelumColors.amber
                            : VelumColors.aquaSoft,
                      ),
                    ],
                  ),
                  const SizedBox(height: 22),
                  Text(
                    '同一逻辑会话，\n两种承载方式。',
                    style: Theme.of(context).textTheme.displaySmall,
                  ),
                  const SizedBox(height: 14),
                  Text(
                    '当 QUIC/UDP 路径不再可靠时，Velum 可以转向 TLS/TCP，同时保留应用层的会话语义。',
                    style: Theme.of(
                      context,
                    ).textTheme.bodyLarge?.copyWith(color: VelumColors.muted),
                  ),
                  const SizedBox(height: 23),
                  Wrap(
                    spacing: 10,
                    runSpacing: 10,
                    children: [
                      FilledButton.icon(
                        onPressed: controller.busy
                            ? null
                            : () async {
                                await controller.refreshStatus();
                                if (context.mounted) {
                                  ScaffoldMessenger.of(context).showSnackBar(
                                    const SnackBar(content: Text('运行状态已刷新。')),
                                  );
                                }
                              },
                        icon: const Icon(Icons.sync_rounded, size: 17),
                        label: const Text('刷新状态'),
                      ),
                      OutlinedButton.icon(
                        onPressed: () => controller.selectPage(1),
                        icon: const Icon(Icons.route_outlined, size: 17),
                        label: const Text('查看会话'),
                      ),
                    ],
                  ),
                ],
              ),
            );
            final visual = Container(
              constraints: const BoxConstraints(minHeight: 310),
              decoration: BoxDecoration(
                color: VelumColors.ink.withValues(alpha: 0.42),
                border: stacked
                    ? const Border(top: BorderSide(color: VelumColors.line))
                    : const Border(left: BorderSide(color: VelumColors.line)),
              ),
              child: const ContinuityVisual(),
            );
            if (stacked) {
              return Column(
                children: [
                  copy,
                  SizedBox(height: 330, child: visual),
                ],
              );
            }
            return SizedBox(
              height: 390,
              child: Row(
                children: [
                  Expanded(flex: 11, child: copy),
                  Expanded(flex: 9, child: visual),
                ],
              ),
            );
          },
        ),
      ),
    );
  }
}

class _ControlPanel extends StatelessWidget {
  const _ControlPanel({required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const PanelTitle(title: '服务控制', subtitle: '控制请求只通过配置中的本地管理套接字。'),
            const SizedBox(height: 20),
            _ControlRow(
              icon: Icons.fact_check_outlined,
              title: '检查配置',
              detail: '验证凭据、TLS 材料和限制项',
              action: '检查',
              onPressed: controller.busy
                  ? null
                  : () => _run(context, controller.validateConfiguration),
            ),
            const Divider(height: 25),
            _ControlRow(
              icon: Icons.hourglass_bottom_rounded,
              title: '排空服务',
              detail: '停止接纳连接，等待现有工作结束',
              action: '排空',
              onPressed: controller.busy
                  ? null
                  : () =>
                        _run(context, () => controller.controlService('drain')),
            ),
            const Divider(height: 25),
            _ControlRow(
              icon: Icons.power_settings_new_rounded,
              title: '停止服务',
              detail: '立即关闭监听器并应用关闭时限',
              action: '停止',
              danger: true,
              onPressed: controller.busy
                  ? null
                  : () async {
                      final confirmed = await showDialog<bool>(
                        context: context,
                        builder: (context) => AlertDialog(
                          title: const Text('停止 Velum 服务？'),
                          content: Text(
                            controller.demoMode
                                ? '这只会改变演示状态，不会影响本机服务。'
                                : '活动连接会被关闭。该操作将通过本机 CLI 执行。',
                          ),
                          actions: [
                            TextButton(
                              onPressed: () => Navigator.pop(context, false),
                              child: const Text('取消'),
                            ),
                            FilledButton(
                              style: FilledButton.styleFrom(
                                backgroundColor: VelumColors.coral,
                              ),
                              onPressed: () => Navigator.pop(context, true),
                              child: const Text('确认停止'),
                            ),
                          ],
                        ),
                      );
                      if (confirmed == true && context.mounted) {
                        await _run(
                          context,
                          () => controller.controlService('shutdown'),
                        );
                      }
                    },
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _run(
    BuildContext context,
    Future<void> Function() operation,
  ) async {
    await operation();
    if (!context.mounted) return;
    final output = controller.lastCommandOutput ?? '操作已完成。';
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(output.split('\n').first)));
  }
}

class _ControlRow extends StatelessWidget {
  const _ControlRow({
    required this.icon,
    required this.title,
    required this.detail,
    required this.action,
    required this.onPressed,
    this.danger = false,
  });

  final IconData icon;
  final String title;
  final String detail;
  final String action;
  final VoidCallback? onPressed;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final color = danger ? VelumColors.coral : VelumColors.aqua;
    return Row(
      children: [
        Container(
          width: 38,
          height: 38,
          decoration: BoxDecoration(
            color: color.withValues(alpha: 0.09),
            borderRadius: BorderRadius.circular(10),
          ),
          child: Icon(icon, size: 18, color: color),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title, style: Theme.of(context).textTheme.titleMedium),
              const SizedBox(height: 2),
              Text(
                detail,
                style: Theme.of(
                  context,
                ).textTheme.bodySmall?.copyWith(color: VelumColors.muted),
              ),
            ],
          ),
        ),
        const SizedBox(width: 8),
        TextButton(
          onPressed: onPressed,
          style: TextButton.styleFrom(foregroundColor: color),
          child: Text(action),
        ),
      ],
    );
  }
}

class _RecentEvents extends StatelessWidget {
  const _RecentEvents({required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    final events = controller.events.take(4).toList();
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            PanelTitle(
              title: '最近活动',
              subtitle: '本地控制与承载变化摘要。',
              trailing: TextButton(
                onPressed: () => controller.selectPage(3),
                child: const Text('全部记录'),
              ),
            ),
            const SizedBox(height: 14),
            if (events.isEmpty)
              const EmptyState(
                icon: Icons.inbox_outlined,
                title: '暂无活动',
                detail: '刷新状态或执行控制操作后，记录会显示在这里。',
              )
            else
              for (var i = 0; i < events.length; i++) ...[
                _EventRow(event: events[i]),
                if (i != events.length - 1) const Divider(height: 18),
              ],
          ],
        ),
      ),
    );
  }
}

class _EventRow extends StatelessWidget {
  const _EventRow({required this.event});

  final OperatorEvent event;

  @override
  Widget build(BuildContext context) {
    final color = switch (event.level) {
      EventLevel.success => VelumColors.aqua,
      EventLevel.warning => VelumColors.amber,
      EventLevel.error => VelumColors.coral,
      EventLevel.info => VelumColors.aquaSoft,
    };
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(top: 7),
          child: Container(
            width: 7,
            height: 7,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      event.title,
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                  ),
                  Text(
                    _formatClock(event.time),
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: VelumColors.muted,
                      fontFamily: 'Consolas',
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 3),
              Text(
                event.detail,
                style: Theme.of(
                  context,
                ).textTheme.bodySmall?.copyWith(color: VelumColors.muted),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

({String label, Color color}) _phaseInfo(ServicePhase phase) => switch (phase) {
  ServicePhase.online => (label: '运行中', color: VelumColors.aqua),
  ServicePhase.draining => (label: '排空中', color: VelumColors.amber),
  ServicePhase.stopping => (label: '停止中', color: VelumColors.coral),
  ServicePhase.offline => (label: '离线', color: VelumColors.coral),
  ServicePhase.checking => (label: '检查中', color: VelumColors.aquaSoft),
};

String _formatDuration(Duration duration) {
  final hours = duration.inHours;
  final minutes = duration.inMinutes.remainder(60);
  if (hours >= 24) return '${hours ~/ 24}d ${hours.remainder(24)}h';
  return '${hours}h ${minutes}m';
}

String _formatClock(DateTime time) =>
    '${time.hour.toString().padLeft(2, '0')}:${time.minute.toString().padLeft(2, '0')}';

String _compactNumber(int value) {
  if (value >= 1000000) return '${(value / 1000000).toStringAsFixed(1)}M';
  if (value >= 1000) return '${(value / 1000).toStringAsFixed(1)}K';
  return value.toString();
}
