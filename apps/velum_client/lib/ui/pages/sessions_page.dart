import 'package:flutter/material.dart';

import '../../controller/velum_controller.dart';
import '../../models/operator_models.dart';
import '../../theme/velum_theme.dart';
import '../widgets/common.dart';

class SessionsPage extends StatefulWidget {
  const SessionsPage({super.key, required this.controller});

  final VelumController controller;

  @override
  State<SessionsPage> createState() => _SessionsPageState();
}

class _SessionsPageState extends State<SessionsPage> {
  String _query = '';
  String _carrier = '全部承载';

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final sessions = controller.sessions.where((session) {
      final queryMatch =
          _query.isEmpty ||
          session.id.toLowerCase().contains(_query.toLowerCase()) ||
          session.target.toLowerCase().contains(_query.toLowerCase());
      final carrierMatch = _carrier == '全部承载' || session.carrier == _carrier;
      return queryMatch && carrierMatch;
    }).toList();

    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(24, 28, 24, 48),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1320),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SectionHeading(
                eyebrow: 'Logical sessions',
                title: '会话与承载',
                description: '观察逻辑会话如何分布在 QUIC/UDP 与 TLS/TCP 路径上。',
                trailing: StatusPill(
                  label: controller.demoMode ? '示例遥测' : '聚合遥测',
                  color: controller.demoMode
                      ? VelumColors.amber
                      : VelumColors.aqua,
                  icon: controller.demoMode
                      ? Icons.science_outlined
                      : Icons.shield_outlined,
                ),
              ),
              const SizedBox(height: 24),
              LayoutBuilder(
                builder: (context, constraints) {
                  final health = _CarrierHealth(controller: controller);
                  const semantics = _DeliverySemantics();
                  if (constraints.maxWidth < 850) {
                    return Column(
                      children: [health, const SizedBox(height: 16), semantics],
                    );
                  }
                  return Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(flex: 7, child: health),
                      const SizedBox(width: 16),
                      const Expanded(flex: 5, child: semantics),
                    ],
                  );
                },
              ),
              const SizedBox(height: 18),
              Card(
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      PanelTitle(
                        title: '会话观察台',
                        subtitle: controller.demoMode
                            ? '演示数据用于检查客户端布局与交互，不代表真实网络流量。'
                            : '当前管理接口只公开无载荷聚合状态，不公开逐会话标识。',
                      ),
                      const SizedBox(height: 18),
                      Wrap(
                        spacing: 10,
                        runSpacing: 10,
                        children: [
                          SizedBox(
                            width: 280,
                            child: TextField(
                              onChanged: (value) =>
                                  setState(() => _query = value),
                              decoration: const InputDecoration(
                                prefixIcon: Icon(
                                  Icons.search_rounded,
                                  size: 19,
                                ),
                                hintText: '搜索会话或目标',
                              ),
                            ),
                          ),
                          SizedBox(
                            width: 180,
                            child: DropdownButtonFormField<String>(
                              initialValue: _carrier,
                              decoration: const InputDecoration(
                                prefixIcon: Icon(
                                  Icons.alt_route_rounded,
                                  size: 18,
                                ),
                              ),
                              items: const [
                                DropdownMenuItem(
                                  value: '全部承载',
                                  child: Text('全部承载'),
                                ),
                                DropdownMenuItem(
                                  value: 'QUIC / UDP',
                                  child: Text('QUIC / UDP'),
                                ),
                                DropdownMenuItem(
                                  value: 'TLS / TCP',
                                  child: Text('TLS / TCP'),
                                ),
                              ],
                              onChanged: (value) {
                                if (value != null) {
                                  setState(() => _carrier = value);
                                }
                              },
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 18),
                      if (!controller.demoMode)
                        const EmptyState(
                          icon: Icons.privacy_tip_outlined,
                          title: '逐会话数据未公开',
                          detail: '这是有意的无载荷管理边界。更细粒度遥测需要未来的显式协议契约。',
                        )
                      else if (sessions.isEmpty)
                        const EmptyState(
                          icon: Icons.filter_alt_off_outlined,
                          title: '没有匹配会话',
                          detail: '更改搜索内容或承载筛选条件。',
                        )
                      else
                        LayoutBuilder(
                          builder: (context, constraints) {
                            if (constraints.maxWidth < 760) {
                              return Column(
                                children: [
                                  for (final session in sessions)
                                    Padding(
                                      padding: const EdgeInsets.only(
                                        bottom: 10,
                                      ),
                                      child: _SessionCard(session: session),
                                    ),
                                ],
                              );
                            }
                            return _SessionTable(sessions: sessions);
                          },
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

class _CarrierHealth extends StatelessWidget {
  const _CarrierHealth({required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    final active = controller.snapshot.activeFlows;
    final quic = controller.demoMode ? (active * 0.68).round() : 0;
    final tls = controller.demoMode ? active - quic : 0;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const PanelTitle(title: '承载健康度', subtitle: '同一会话可以根据路径质量调整承载。'),
            const SizedBox(height: 24),
            _CarrierBar(
              label: 'QUIC / UDP',
              detail: controller.demoMode ? '$quic 个活跃流 · 低延迟路径' : '等待遥测契约',
              value: controller.demoMode ? 0.68 : 0,
              color: VelumColors.aqua,
            ),
            const SizedBox(height: 20),
            _CarrierBar(
              label: 'TLS / TCP',
              detail: controller.demoMode ? '$tls 个活跃流 · 兼容路径' : '等待遥测契约',
              value: controller.demoMode ? 0.32 : 0,
              color: VelumColors.amber,
            ),
          ],
        ),
      ),
    );
  }
}

class _CarrierBar extends StatelessWidget {
  const _CarrierBar({
    required this.label,
    required this.detail,
    required this.value,
    required this.color,
  });

  final String label;
  final String detail;
  final double value;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Container(
              width: 8,
              height: 8,
              decoration: BoxDecoration(color: color, shape: BoxShape.circle),
            ),
            const SizedBox(width: 9),
            Text(label, style: Theme.of(context).textTheme.titleMedium),
            const Spacer(),
            Text(
              '${(value * 100).round()}%',
              style: Theme.of(context).textTheme.titleMedium?.copyWith(
                fontFamily: 'Consolas',
                color: color,
              ),
            ),
          ],
        ),
        const SizedBox(height: 9),
        ClipRRect(
          borderRadius: BorderRadius.circular(99),
          child: LinearProgressIndicator(
            value: value,
            minHeight: 6,
            color: color,
            backgroundColor: VelumColors.ink,
          ),
        ),
        const SizedBox(height: 7),
        Text(
          detail,
          style: Theme.of(
            context,
          ).textTheme.bodySmall?.copyWith(color: VelumColors.muted),
        ),
      ],
    );
  }
}

class _DeliverySemantics extends StatelessWidget {
  const _DeliverySemantics();

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const PanelTitle(title: '传递语义', subtitle: 'Velum 按工作负载表达不同的可靠性。'),
            const SizedBox(height: 17),
            const _SemanticRow(
              icon: Icons.linear_scale_rounded,
              label: 'Stream',
              detail: '有序字节流',
            ),
            const Divider(height: 20),
            const _SemanticRow(
              icon: Icons.mark_chat_unread_outlined,
              label: 'Message',
              detail: '有边界消息',
            ),
            const Divider(height: 20),
            const _SemanticRow(
              icon: Icons.grain_rounded,
              label: 'Datagram',
              detail: '不可靠数据报',
            ),
          ],
        ),
      ),
    );
  }
}

class _SemanticRow extends StatelessWidget {
  const _SemanticRow({
    required this.icon,
    required this.label,
    required this.detail,
  });

  final IconData icon;
  final String label;
  final String detail;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Icon(icon, size: 18, color: VelumColors.aquaSoft),
        const SizedBox(width: 11),
        Text(label, style: Theme.of(context).textTheme.titleMedium),
        const Spacer(),
        Text(
          detail,
          style: Theme.of(
            context,
          ).textTheme.bodySmall?.copyWith(color: VelumColors.muted),
        ),
      ],
    );
  }
}

class _SessionTable extends StatelessWidget {
  const _SessionTable({required this.sessions});

  final List<SessionSample> sessions;

  @override
  Widget build(BuildContext context) {
    return Table(
      columnWidths: const {
        0: FlexColumnWidth(1.15),
        1: FlexColumnWidth(1.8),
        2: FlexColumnWidth(1.25),
        3: FlexColumnWidth(0.8),
        4: FlexColumnWidth(0.9),
        5: FlexColumnWidth(0.8),
      },
      children: [
        TableRow(
          decoration: const BoxDecoration(
            border: Border(bottom: BorderSide(color: VelumColors.line)),
          ),
          children: [
            for (final label in ['会话', '目标', '承载', '延迟', '传输', '状态'])
              Padding(
                padding: const EdgeInsets.fromLTRB(10, 0, 10, 11),
                child: Text(
                  label,
                  style: Theme.of(
                    context,
                  ).textTheme.labelSmall?.copyWith(color: VelumColors.muted),
                ),
              ),
          ],
        ),
        for (final session in sessions)
          TableRow(
            decoration: const BoxDecoration(
              border: Border(bottom: BorderSide(color: VelumColors.line)),
            ),
            children: [
              _Cell(session.id, mono: true),
              _Cell(session.target, mono: true),
              _Cell(session.carrier),
              _Cell('${session.latencyMs} ms', mono: true),
              _Cell(session.transfer, mono: true),
              Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 15,
                ),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: StatusPill(
                    label: session.state,
                    color: session.state == '迁移中'
                        ? VelumColors.amber
                        : VelumColors.aqua,
                  ),
                ),
              ),
            ],
          ),
      ],
    );
  }
}

class _Cell extends StatelessWidget {
  const _Cell(this.text, {this.mono = false});

  final String text;
  final bool mono;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 18),
      child: Text(
        text,
        style: Theme.of(
          context,
        ).textTheme.bodyMedium?.copyWith(fontFamily: mono ? 'Consolas' : null),
      ),
    );
  }
}

class _SessionCard extends StatelessWidget {
  const _SessionCard({required this.session});

  final SessionSample session;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(15),
      decoration: BoxDecoration(
        color: VelumColors.ink.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(13),
        border: Border.all(color: VelumColors.line),
      ),
      child: Column(
        children: [
          Row(
            children: [
              Text(
                session.id,
                style: const TextStyle(
                  fontFamily: 'Consolas',
                  fontWeight: FontWeight.w700,
                ),
              ),
              const Spacer(),
              StatusPill(
                label: session.state,
                color: session.state == '迁移中'
                    ? VelumColors.amber
                    : VelumColors.aqua,
              ),
            ],
          ),
          const SizedBox(height: 12),
          _MobileField(label: '目标', value: session.target),
          _MobileField(label: '承载', value: session.carrier),
          _MobileField(
            label: '延迟 / 传输',
            value: '${session.latencyMs} ms · ${session.transfer}',
          ),
        ],
      ),
    );
  }
}

class _MobileField extends StatelessWidget {
  const _MobileField({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 7),
      child: Row(
        children: [
          Text(
            label,
            style: Theme.of(
              context,
            ).textTheme.bodySmall?.copyWith(color: VelumColors.muted),
          ),
          const Spacer(),
          Text(value, style: Theme.of(context).textTheme.bodySmall),
        ],
      ),
    );
  }
}
