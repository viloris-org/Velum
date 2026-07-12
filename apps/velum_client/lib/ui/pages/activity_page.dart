import 'package:flutter/material.dart';

import '../../controller/velum_controller.dart';
import '../../models/operator_models.dart';
import '../../theme/velum_theme.dart';
import '../widgets/common.dart';

class ActivityPage extends StatelessWidget {
  const ActivityPage({super.key, required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(24, 28, 24, 48),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1120),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SectionHeading(
                eyebrow: 'Local operations',
                title: '活动记录',
                description: '记录本次客户端会话中的状态读取、配置验证和服务控制。',
                trailing: OutlinedButton.icon(
                  onPressed: controller.events.isEmpty
                      ? null
                      : controller.clearEvents,
                  icon: const Icon(Icons.delete_sweep_outlined, size: 18),
                  label: const Text('清空记录'),
                ),
              ),
              const SizedBox(height: 24),
              if (controller.lastCommandOutput != null) ...[
                _CommandOutput(output: controller.lastCommandOutput!),
                const SizedBox(height: 18),
              ],
              Card(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      PanelTitle(
                        title: '操作时间线',
                        subtitle: '${controller.events.length} 条本地记录 · 不包含流量载荷',
                      ),
                      const SizedBox(height: 18),
                      if (controller.events.isEmpty)
                        const EmptyState(
                          icon: Icons.history_toggle_off_outlined,
                          title: '记录已清空',
                          detail: '执行状态刷新、配置检查或服务控制后，新记录会显示在这里。',
                        )
                      else
                        for (
                          var index = 0;
                          index < controller.events.length;
                          index++
                        )
                          _TimelineEvent(
                            event: controller.events[index],
                            first: index == 0,
                            last: index == controller.events.length - 1,
                          ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _CommandOutput extends StatelessWidget {
  const _CommandOutput({required this.output});

  final String output;

  @override
  Widget build(BuildContext context) {
    return Card(
      color: VelumColors.panel,
      child: Padding(
        padding: const EdgeInsets.all(18),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Row(
              children: [
                Icon(Icons.terminal_rounded, size: 18, color: VelumColors.aqua),
                SizedBox(width: 9),
                Text('最近命令输出', style: TextStyle(fontWeight: FontWeight.w600)),
              ],
            ),
            const SizedBox(height: 13),
            Container(
              width: double.infinity,
              padding: const EdgeInsets.all(14),
              decoration: BoxDecoration(
                color: VelumColors.ink,
                borderRadius: BorderRadius.circular(11),
                border: Border.all(color: VelumColors.line),
              ),
              child: SelectableText(
                output,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  fontFamily: 'Consolas',
                  color: VelumColors.aquaSoft,
                  height: 1.55,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _TimelineEvent extends StatelessWidget {
  const _TimelineEvent({
    required this.event,
    required this.first,
    required this.last,
  });

  final OperatorEvent event;
  final bool first;
  final bool last;

  @override
  Widget build(BuildContext context) {
    final color = switch (event.level) {
      EventLevel.success => VelumColors.aqua,
      EventLevel.warning => VelumColors.amber,
      EventLevel.error => VelumColors.coral,
      EventLevel.info => VelumColors.aquaSoft,
    };
    final icon = switch (event.level) {
      EventLevel.success => Icons.check_rounded,
      EventLevel.warning => Icons.swap_horiz_rounded,
      EventLevel.error => Icons.close_rounded,
      EventLevel.info => Icons.info_outline_rounded,
    };
    return IntrinsicHeight(
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SizedBox(
            width: 38,
            child: Column(
              children: [
                if (!first)
                  Expanded(child: Container(width: 1, color: VelumColors.line))
                else
                  const Spacer(),
                Container(
                  width: 28,
                  height: 28,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: color.withValues(alpha: 0.12),
                    border: Border.all(color: color.withValues(alpha: 0.5)),
                  ),
                  child: Icon(icon, size: 14, color: color),
                ),
                if (!last)
                  Expanded(child: Container(width: 1, color: VelumColors.line))
                else
                  const Spacer(),
              ],
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 14),
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
                        _timestamp(event.time),
                        style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: VelumColors.muted,
                          fontFamily: 'Consolas',
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 5),
                  Text(
                    event.detail,
                    style: Theme.of(
                      context,
                    ).textTheme.bodyMedium?.copyWith(color: VelumColors.muted),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  String _timestamp(DateTime time) =>
      '${time.year}-${time.month.toString().padLeft(2, '0')}-${time.day.toString().padLeft(2, '0')} '
      '${time.hour.toString().padLeft(2, '0')}:${time.minute.toString().padLeft(2, '0')}:${time.second.toString().padLeft(2, '0')}';
}
