import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../controller/velum_controller.dart';
import '../../models/operator_models.dart';
import '../../theme/velum_theme.dart';
import '../widgets/common.dart';

class ConfigurationPage extends StatelessWidget {
  const ConfigurationPage({super.key, required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    final config = controller.configuration;
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(24, 28, 24, 48),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 1320),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SectionHeading(
                eyebrow: 'Versioned configuration',
                title: '配置工作台',
                description: '生成与 Velum v1 配置结构一致的 TOML，并通过 CLI 验证部署材料。',
                trailing: FilledButton.icon(
                  onPressed: controller.busy
                      ? null
                      : () async {
                          await controller.validateConfiguration();
                          if (context.mounted) {
                            ScaffoldMessenger.of(context).showSnackBar(
                              SnackBar(
                                content: Text(
                                  (controller.lastCommandOutput ?? '配置检查完成。')
                                      .split('\n')
                                      .first,
                                ),
                              ),
                            );
                          }
                        },
                  icon: const Icon(Icons.fact_check_outlined, size: 18),
                  label: const Text('验证配置'),
                ),
              ),
              const SizedBox(height: 24),
              LayoutBuilder(
                builder: (context, constraints) {
                  final wide = constraints.maxWidth >= 980;
                  final editor = _ConfigurationEditor(
                    controller: controller,
                    config: config,
                  );
                  final preview = _TomlPreview(
                    controller: controller,
                    config: config,
                  );
                  if (!wide) {
                    return Column(
                      children: [editor, const SizedBox(height: 18), preview],
                    );
                  }
                  return Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(flex: 7, child: editor),
                      const SizedBox(width: 18),
                      Expanded(flex: 5, child: preview),
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

class _ConfigurationEditor extends StatelessWidget {
  const _ConfigurationEditor({required this.controller, required this.config});

  final VelumController controller;
  final VelumConfiguration config;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        _ConfigSection(
          icon: Icons.sensors_outlined,
          title: '监听器与 TLS',
          subtitle: '设置 QUIC 监听地址及服务器证书材料。',
          child: _ResponsiveFields(
            children: [
              _TextField(
                label: '监听地址',
                value: config.bind,
                hint: '0.0.0.0:4433',
                onChanged: (value) => _update(config.copyWith(bind: value)),
              ),
              _TextField(
                label: '证书路径',
                value: config.certificate,
                onChanged: (value) =>
                    _update(config.copyWith(certificate: value)),
              ),
              _TextField(
                label: '私钥路径',
                value: config.privateKey,
                obscure: true,
                onChanged: (value) =>
                    _update(config.copyWith(privateKey: value)),
              ),
              _TextField(
                label: '管理套接字',
                value: config.adminSocket,
                onChanged: (value) =>
                    _update(config.copyWith(adminSocket: value)),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        _ConfigSection(
          icon: Icons.key_outlined,
          title: '凭据与目标策略',
          subtitle: '秘密仅保留文件引用；客户端不会读取或显示秘密内容。',
          child: Column(
            children: [
              _ResponsiveFields(
                children: [
                  _TextField(
                    label: 'Principal ID',
                    value: config.credentialId,
                    hint: '1',
                    onChanged: (value) =>
                        _update(config.copyWith(credentialId: value)),
                  ),
                  _TextField(
                    label: '凭据文件',
                    value: config.credentialFile,
                    obscure: true,
                    onChanged: (value) =>
                        _update(config.copyWith(credentialFile: value)),
                  ),
                ],
              ),
              const SizedBox(height: 14),
              TextFormField(
                initialValue: config.allowedTargets,
                minLines: 2,
                maxLines: 4,
                onChanged: (value) =>
                    _update(config.copyWith(allowedTargets: value)),
                decoration: const InputDecoration(
                  labelText: '允许目标（逗号或换行分隔）',
                  hintText: '203.0.113.10:443',
                  alignLabelWithHint: true,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        _ConfigSection(
          icon: Icons.speed_outlined,
          title: '资源限制',
          subtitle: '边界值用于保护中继，避免单个会话或连接占用全部资源。',
          child: _ResponsiveFields(
            compact: true,
            children: [
              _NumberField(
                label: '最大会话',
                value: config.maxSessions,
                onChanged: (value) =>
                    _update(config.copyWith(maxSessions: value)),
              ),
              _NumberField(
                label: '每会话最大流',
                value: config.maxFlows,
                onChanged: (value) => _update(config.copyWith(maxFlows: value)),
              ),
              _NumberField(
                label: '最大连接',
                value: config.maxConnections,
                onChanged: (value) =>
                    _update(config.copyWith(maxConnections: value)),
              ),
              _NumberField(
                label: '每连接最大流',
                value: config.maxStreams,
                onChanged: (value) =>
                    _update(config.copyWith(maxStreams: value)),
              ),
              _NumberField(
                label: '连接超时（秒）',
                value: config.connectTimeout,
                onChanged: (value) =>
                    _update(config.copyWith(connectTimeout: value)),
              ),
              _NumberField(
                label: '控制超时（秒）',
                value: config.controlTimeout,
                onChanged: (value) =>
                    _update(config.copyWith(controlTimeout: value)),
              ),
              _NumberField(
                label: '关闭时限（秒）',
                value: config.shutdownTimeout,
                onChanged: (value) =>
                    _update(config.copyWith(shutdownTimeout: value)),
              ),
            ],
          ),
        ),
      ],
    );
  }

  void _update(VelumConfiguration value) =>
      controller.updateConfiguration(value);
}

class _ConfigSection extends StatelessWidget {
  const _ConfigSection({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.child,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Container(
                  width: 38,
                  height: 38,
                  decoration: BoxDecoration(
                    color: VelumColors.aqua.withValues(alpha: 0.09),
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Icon(icon, size: 18, color: VelumColors.aqua),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: PanelTitle(title: title, subtitle: subtitle),
                ),
              ],
            ),
            const SizedBox(height: 20),
            child,
          ],
        ),
      ),
    );
  }
}

class _ResponsiveFields extends StatelessWidget {
  const _ResponsiveFields({required this.children, this.compact = false});

  final List<Widget> children;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final minWidth = compact ? 165.0 : 245.0;
        final columns = (constraints.maxWidth / minWidth).floor().clamp(
          1,
          compact ? 3 : 2,
        );
        const gap = 12.0;
        final width = (constraints.maxWidth - gap * (columns - 1)) / columns;
        return Wrap(
          spacing: gap,
          runSpacing: 12,
          children: [
            for (final child in children) SizedBox(width: width, child: child),
          ],
        );
      },
    );
  }
}

class _TextField extends StatelessWidget {
  const _TextField({
    required this.label,
    required this.value,
    required this.onChanged,
    this.hint,
    this.obscure = false,
  });

  final String label;
  final String value;
  final ValueChanged<String> onChanged;
  final String? hint;
  final bool obscure;

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      initialValue: value,
      onChanged: onChanged,
      style: TextStyle(fontFamily: obscure ? null : 'Consolas'),
      decoration: InputDecoration(
        labelText: label,
        hintText: hint,
        suffixIcon: obscure
            ? const Tooltip(
                message: '只保存文件路径，不读取秘密',
                child: Icon(Icons.lock_outline_rounded, size: 17),
              )
            : null,
      ),
    );
  }
}

class _NumberField extends StatelessWidget {
  const _NumberField({
    required this.label,
    required this.value,
    required this.onChanged,
  });

  final String label;
  final int value;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      initialValue: value.toString(),
      keyboardType: TextInputType.number,
      inputFormatters: [FilteringTextInputFormatter.digitsOnly],
      onChanged: (text) {
        final parsed = int.tryParse(text);
        if (parsed != null) onChanged(parsed);
      },
      style: const TextStyle(fontFamily: 'Consolas'),
      decoration: InputDecoration(labelText: label),
    );
  }
}

class _TomlPreview extends StatelessWidget {
  const _TomlPreview({required this.controller, required this.config});

  final VelumController controller;
  final VelumConfiguration config;

  @override
  Widget build(BuildContext context) {
    final toml = config.toToml();
    return Column(
      children: [
        Card(
          color: VelumColors.panel,
          child: Padding(
            padding: const EdgeInsets.all(20),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                PanelTitle(
                  title: 'config.toml',
                  subtitle: '版本化配置预览',
                  trailing: IconButton(
                    tooltip: '复制 TOML',
                    onPressed: () async {
                      await Clipboard.setData(ClipboardData(text: toml));
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(content: Text('TOML 已复制到剪贴板。')),
                        );
                      }
                    },
                    icon: const Icon(Icons.copy_all_outlined, size: 19),
                  ),
                ),
                const SizedBox(height: 16),
                Container(
                  width: double.infinity,
                  padding: const EdgeInsets.all(16),
                  decoration: BoxDecoration(
                    color: VelumColors.ink,
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(color: VelumColors.line),
                  ),
                  child: SelectableText(
                    toml,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      fontFamily: 'Consolas',
                      height: 1.62,
                      color: VelumColors.aquaSoft,
                    ),
                  ),
                ),
                const SizedBox(height: 14),
                SizedBox(
                  width: double.infinity,
                  child: OutlinedButton.icon(
                    onPressed: () async {
                      await Clipboard.setData(ClipboardData(text: toml));
                      if (context.mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('配置已复制，可保存为 config.toml。'),
                          ),
                        );
                      }
                    },
                    icon: const Icon(Icons.content_copy_rounded, size: 17),
                    label: const Text('复制完整配置'),
                  ),
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: 16),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(18),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const StatusPill(
                  label: 'SECURITY BOUNDARY',
                  color: VelumColors.amber,
                  icon: Icons.shield_outlined,
                ),
                const SizedBox(height: 13),
                Text(
                  '秘密不会进入界面',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 6),
                Text(
                  '客户端只处理 credential.hex、证书与私钥的路径。秘密内容由 Velum CLI 在本机读取。',
                  style: Theme.of(
                    context,
                  ).textTheme.bodySmall?.copyWith(color: VelumColors.muted),
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}
