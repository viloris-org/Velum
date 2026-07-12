import 'package:flutter/material.dart';

import '../../controller/velum_controller.dart';
import '../../theme/velum_theme.dart';
import '../widgets/common.dart';

class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key, required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(24, 28, 24, 48),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1050),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const SectionHeading(
                eyebrow: 'Client runtime',
                title: '客户端设置',
                description: '选择安全演示或本机 CLI 运行时，并指定 Velum 二进制与配置路径。',
              ),
              const SizedBox(height: 24),
              _RuntimeCard(controller: controller),
              const SizedBox(height: 16),
              _PathsCard(controller: controller),
              const SizedBox(height: 16),
              const _PrinciplesCard(),
              const SizedBox(height: 16),
              const _AboutCard(),
            ],
          ),
        ),
      ),
    );
  }
}

class _RuntimeCard extends StatelessWidget {
  const _RuntimeCard({required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const PanelTitle(title: '运行模式', subtitle: '浏览器版本永远不会尝试启动本机命令。'),
            const SizedBox(height: 18),
            _RuntimeOption(
              selected: controller.demoMode,
              icon: Icons.science_outlined,
              color: VelumColors.amber,
              title: '安全演示',
              detail: '使用可预测的示例状态验证界面、流程和响应式布局。',
              onTap: () => controller.setDemoMode(true),
            ),
            const SizedBox(height: 10),
            _RuntimeOption(
              selected: !controller.demoMode,
              icon: Icons.terminal_rounded,
              color: VelumColors.aqua,
              title: '本机 Velum CLI',
              detail: controller.bridge.supportsLocalCommands
                  ? '通过 ${controller.bridge.adapterName} 执行 status、validate、drain 和 shutdown。'
                  : '当前为浏览器构建；请运行 Windows 客户端以启用本机进程。',
              enabled: controller.bridge.supportsLocalCommands,
              onTap: () => controller.setDemoMode(false),
            ),
          ],
        ),
      ),
    );
  }
}

class _RuntimeOption extends StatelessWidget {
  const _RuntimeOption({
    required this.selected,
    required this.icon,
    required this.color,
    required this.title,
    required this.detail,
    required this.onTap,
    this.enabled = true,
  });

  final bool selected;
  final IconData icon;
  final Color color;
  final String title;
  final String detail;
  final VoidCallback onTap;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    return Opacity(
      opacity: enabled ? 1 : 0.52,
      child: Material(
        color: selected
            ? color.withValues(alpha: 0.08)
            : VelumColors.ink.withValues(alpha: 0.36),
        borderRadius: BorderRadius.circular(14),
        child: InkWell(
          onTap: enabled ? onTap : null,
          borderRadius: BorderRadius.circular(14),
          child: Container(
            padding: const EdgeInsets.all(15),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(14),
              border: Border.all(
                color: selected
                    ? color.withValues(alpha: 0.55)
                    : VelumColors.line,
              ),
            ),
            child: Row(
              children: [
                Container(
                  width: 42,
                  height: 42,
                  decoration: BoxDecoration(
                    color: color.withValues(alpha: 0.1),
                    borderRadius: BorderRadius.circular(11),
                  ),
                  child: Icon(icon, color: color, size: 20),
                ),
                const SizedBox(width: 13),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        title,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 3),
                      Text(
                        detail,
                        style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: VelumColors.muted,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 10),
                Icon(
                  selected
                      ? Icons.radio_button_checked
                      : Icons.radio_button_off,
                  color: selected ? color : VelumColors.muted,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _PathsCard extends StatelessWidget {
  const _PathsCard({required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const PanelTitle(title: '本机路径', subtitle: '只有本机 CLI 模式会使用这些设置。'),
            const SizedBox(height: 18),
            TextFormField(
              initialValue: controller.binaryPath,
              onChanged: (value) =>
                  controller.setRuntimePaths(binaryPath: value),
              style: const TextStyle(fontFamily: 'Consolas'),
              decoration: const InputDecoration(
                labelText: 'Velum 二进制',
                prefixIcon: Icon(Icons.terminal_rounded, size: 19),
                hintText: 'velum 或 /usr/local/bin/velum',
              ),
            ),
            const SizedBox(height: 12),
            TextFormField(
              initialValue: controller.configPath,
              onChanged: (value) =>
                  controller.setRuntimePaths(configPath: value),
              style: const TextStyle(fontFamily: 'Consolas'),
              decoration: const InputDecoration(
                labelText: '配置文件',
                prefixIcon: Icon(Icons.description_outlined, size: 19),
                hintText: '/etc/velum/config.toml',
              ),
            ),
            const SizedBox(height: 16),
            Align(
              alignment: Alignment.centerLeft,
              child: OutlinedButton.icon(
                onPressed: controller.busy ? null : controller.refreshStatus,
                icon: const Icon(Icons.cable_rounded, size: 18),
                label: const Text('测试运行时'),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _PrinciplesCard extends StatelessWidget {
  const _PrinciplesCard();

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const PanelTitle(title: '安全与隐私边界', subtitle: '客户端遵循服务端已有的本地管理设计。'),
            const SizedBox(height: 18),
            const _Principle(
              icon: Icons.lan_outlined,
              title: '没有远程管理端口',
              detail: '控制命令通过配置指定的本地管理套接字完成。',
            ),
            const Divider(height: 22),
            const _Principle(
              icon: Icons.visibility_off_outlined,
              title: '不展示流量载荷',
              detail: '状态仅包含阶段、监听器、运行时间、连接数与活动流数量。',
            ),
            const Divider(height: 22),
            const _Principle(
              icon: Icons.key_off_outlined,
              title: '不读取秘密',
              detail: '配置编辑器只记录凭据文件、证书与私钥的路径。',
            ),
          ],
        ),
      ),
    );
  }
}

class _Principle extends StatelessWidget {
  const _Principle({
    required this.icon,
    required this.title,
    required this.detail,
  });

  final IconData icon;
  final String title;
  final String detail;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(icon, size: 20, color: VelumColors.aqua),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title, style: Theme.of(context).textTheme.titleMedium),
              const SizedBox(height: 4),
              Text(
                detail,
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

class _AboutCard extends StatelessWidget {
  const _AboutCard();

  @override
  Widget build(BuildContext context) {
    return Card(
      color: VelumColors.panel,
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Icon(Icons.biotech_outlined, color: VelumColors.amber),
            const SizedBox(width: 13),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Velum 仍是研究阶段软件',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 6),
                  Text(
                    '项目尚未经过安全审计，也没有稳定的线协议或生产安全声明。请仅在获得授权的环境中使用。',
                    style: Theme.of(
                      context,
                    ).textTheme.bodySmall?.copyWith(color: VelumColors.muted),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 12),
            const StatusPill(label: 'v0.0.1 beta', color: VelumColors.amber),
          ],
        ),
      ),
    );
  }
}
