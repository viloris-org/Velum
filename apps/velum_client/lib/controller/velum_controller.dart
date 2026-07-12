import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../models/operator_models.dart';
import '../services/velum_bridge.dart';

class VelumController extends ChangeNotifier {
  VelumController(this.bridge) {
    _events = [
      OperatorEvent(
        time: DateTime.now().subtract(const Duration(minutes: 2)),
        title: '会话连续性正常',
        detail: '逻辑会话在 QUIC/UDP 与 TLS/TCP 承载之间保持稳定。',
        level: EventLevel.success,
      ),
      OperatorEvent(
        time: DateTime.now().subtract(const Duration(minutes: 18)),
        title: '配置已加载',
        detail: '监听器、凭据和目标策略通过本地校验。',
        level: EventLevel.info,
      ),
      OperatorEvent(
        time: DateTime.now().subtract(const Duration(hours: 1, minutes: 7)),
        title: '承载切换完成',
        detail: '路径质量下降后，3 个流从 QUIC/UDP 转移到 TLS/TCP。',
        level: EventLevel.warning,
      ),
    ];
  }

  final VelumBridge bridge;

  int _selectedIndex = 0;
  bool _demoMode = true;
  bool _busy = false;
  int _demoTick = 0;
  String _binaryPath = 'velum';
  String _configPath = '/etc/velum/config.toml';
  String? _lastCommandOutput;
  ServiceSnapshot _snapshot = ServiceSnapshot.demo();
  VelumConfiguration _configuration = const VelumConfiguration();
  late List<OperatorEvent> _events;

  int get selectedIndex => _selectedIndex;
  bool get demoMode => _demoMode;
  bool get busy => _busy;
  String get binaryPath => _binaryPath;
  String get configPath => _configPath;
  String? get lastCommandOutput => _lastCommandOutput;
  ServiceSnapshot get snapshot => _snapshot;
  VelumConfiguration get configuration => _configuration;
  List<OperatorEvent> get events => List.unmodifiable(_events);

  List<SessionSample> get sessions => const [
    SessionSample(
      id: '7F3A·91C2',
      target: '203.0.113.10:443',
      carrier: 'QUIC / UDP',
      latencyMs: 42,
      transfer: '18.4 MB',
      state: '活跃',
    ),
    SessionSample(
      id: 'A81D·02E7',
      target: '198.51.100.24:443',
      carrier: 'TLS / TCP',
      latencyMs: 87,
      transfer: '6.8 MB',
      state: '活跃',
    ),
    SessionSample(
      id: '2C90·E641',
      target: '203.0.113.10:443',
      carrier: 'QUIC / UDP',
      latencyMs: 51,
      transfer: '2.1 MB',
      state: '迁移中',
    ),
    SessionSample(
      id: 'D4B7·117A',
      target: '192.0.2.80:8443',
      carrier: 'TLS / TCP',
      latencyMs: 109,
      transfer: '920 KB',
      state: '活跃',
    ),
  ];

  void selectPage(int value) {
    if (_selectedIndex == value) return;
    _selectedIndex = value;
    notifyListeners();
  }

  void setDemoMode(bool value) {
    if (_demoMode == value) return;
    _demoMode = value;
    _lastCommandOutput = value ? '已切换到安全演示模式。' : null;
    _snapshot = value
        ? ServiceSnapshot.demo()
        : _snapshot.copyWith(phase: ServicePhase.checking);
    _addEvent(
      title: value ? '演示模式已启用' : '本机 CLI 模式已启用',
      detail: value ? '所有控制操作仅更新本地演示状态。' : '后续操作将通过 ${bridge.adapterName} 执行。',
      level: EventLevel.info,
    );
    notifyListeners();
  }

  void setRuntimePaths({String? binaryPath, String? configPath}) {
    if (binaryPath != null) _binaryPath = binaryPath.trim();
    if (configPath != null) _configPath = configPath.trim();
    notifyListeners();
  }

  void updateConfiguration(VelumConfiguration value) {
    _configuration = value;
    notifyListeners();
  }

  Future<void> refreshStatus() async {
    if (_busy) return;
    _setBusy(true);
    if (_demoMode) {
      await Future<void>.delayed(const Duration(milliseconds: 420));
      _demoTick += 1;
      _snapshot = _snapshot.copyWith(
        phase: ServicePhase.online,
        uptime: _snapshot.uptime + const Duration(seconds: 19),
        admittedConnections: _snapshot.admittedConnections + 3 + _demoTick,
        activeFlows: 34 + (_demoTick % 7),
        updatedAt: DateTime.now(),
      );
      _lastCommandOutput = '演示状态已刷新。';
      _addEvent(
        title: '状态已刷新',
        detail: '读取到 ${_snapshot.activeFlows} 个活跃流。',
        level: EventLevel.success,
      );
    } else {
      final result = await bridge.run(
        binaryPath: _binaryPath,
        arguments: ['status', '--format', 'json', '--config', _configPath],
      );
      _lastCommandOutput = result.output;
      if (result.success) {
        try {
          final decoded = jsonDecode(result.output) as Map<String, dynamic>;
          _snapshot = ServiceSnapshot.fromJson(decoded);
          _addEvent(
            title: '本机状态已同步',
            detail: 'Velum CLI 返回有效运行状态。',
            level: EventLevel.success,
          );
        } on FormatException {
          _snapshot = _snapshot.copyWith(
            phase: ServicePhase.offline,
            updatedAt: DateTime.now(),
          );
          _addEvent(
            title: '状态格式无法解析',
            detail: 'CLI 已响应，但未返回预期 JSON。',
            level: EventLevel.error,
          );
        }
      } else {
        _snapshot = _snapshot.copyWith(
          phase: ServicePhase.offline,
          updatedAt: DateTime.now(),
        );
        _addEvent(
          title: '无法读取服务状态',
          detail: result.output,
          level: EventLevel.error,
        );
      }
    }
    _setBusy(false);
  }

  Future<void> validateConfiguration() async {
    if (_busy) return;
    _setBusy(true);
    if (_demoMode) {
      await Future<void>.delayed(const Duration(milliseconds: 560));
      _lastCommandOutput =
          'Configuration is valid\nlistener=${_configuration.bind}\ntargets=${_configuration.allowedTargets}';
      _addEvent(
        title: '配置检查通过',
        detail: '演示配置中的字段、凭据引用和限制项均有效。',
        level: EventLevel.success,
      );
    } else {
      final result = await bridge.run(
        binaryPath: _binaryPath,
        arguments: ['config', 'validate', '--config', _configPath],
      );
      _lastCommandOutput = result.output;
      _addEvent(
        title: result.success ? '配置检查通过' : '配置检查失败',
        detail: result.output,
        level: result.success ? EventLevel.success : EventLevel.error,
      );
    }
    _setBusy(false);
  }

  Future<void> controlService(String action) async {
    if (_busy || !{'drain', 'shutdown'}.contains(action)) return;
    _setBusy(true);
    final label = action == 'drain' ? '排空' : '停止';
    if (_demoMode) {
      await Future<void>.delayed(const Duration(milliseconds: 520));
      _snapshot = _snapshot.copyWith(
        phase: action == 'drain' ? ServicePhase.draining : ServicePhase.offline,
        activeFlows: action == 'drain' ? _snapshot.activeFlows : 0,
        updatedAt: DateTime.now(),
      );
      _lastCommandOutput = '演示服务已$label。';
      _addEvent(
        title: '服务已$label',
        detail: action == 'drain' ? '停止接收新连接，现有流继续完成。' : '监听器和活动流已在演示状态中关闭。',
        level: EventLevel.warning,
      );
    } else {
      final result = await bridge.run(
        binaryPath: _binaryPath,
        arguments: [action, '--config', _configPath],
      );
      _lastCommandOutput = result.output;
      if (result.success) {
        _snapshot = _snapshot.copyWith(
          phase: action == 'drain'
              ? ServicePhase.draining
              : ServicePhase.offline,
          updatedAt: DateTime.now(),
        );
      }
      _addEvent(
        title: result.success ? '服务已$label' : '$label操作失败',
        detail: result.output,
        level: result.success ? EventLevel.warning : EventLevel.error,
      );
    }
    _setBusy(false);
  }

  void clearEvents() {
    _events = [];
    notifyListeners();
  }

  void _setBusy(bool value) {
    _busy = value;
    notifyListeners();
  }

  void _addEvent({
    required String title,
    required String detail,
    required EventLevel level,
  }) {
    _events = [
      OperatorEvent(
        time: DateTime.now(),
        title: title,
        detail: detail,
        level: level,
      ),
      ..._events,
    ].take(40).toList();
  }
}
