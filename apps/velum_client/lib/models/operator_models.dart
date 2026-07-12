enum ServicePhase { online, draining, stopping, offline, checking }

enum EventLevel { info, success, warning, error }

class ServiceSnapshot {
  const ServiceSnapshot({
    required this.phase,
    required this.listener,
    required this.uptime,
    required this.admittedConnections,
    required this.activeFlows,
    required this.updatedAt,
  });

  final ServicePhase phase;
  final String listener;
  final Duration uptime;
  final int admittedConnections;
  final int activeFlows;
  final DateTime updatedAt;

  factory ServiceSnapshot.demo() => ServiceSnapshot(
    phase: ServicePhase.online,
    listener: '0.0.0.0:4433',
    uptime: const Duration(hours: 6, minutes: 42, seconds: 18),
    admittedConnections: 1284,
    activeFlows: 37,
    updatedAt: DateTime.now(),
  );

  factory ServiceSnapshot.fromJson(Map<String, dynamic> json) {
    final rawState = (json['state'] ?? json['phase'] ?? 'offline').toString();
    final phase = switch (rawState.toLowerCase()) {
      'running' || 'online' || 'ready' => ServicePhase.online,
      'draining' => ServicePhase.draining,
      'stopping' => ServicePhase.stopping,
      _ => ServicePhase.offline,
    };
    final uptimeSeconds = _intValue(
      json['uptime_secs'] ?? json['uptime_seconds'] ?? json['uptime'],
    );
    return ServiceSnapshot(
      phase: phase,
      listener: (json['listener'] ?? json['bind'] ?? 'unknown').toString(),
      uptime: Duration(seconds: uptimeSeconds),
      admittedConnections: _intValue(
        json['admitted_connections'] ?? json['connections'],
      ),
      activeFlows: _intValue(json['active_flows'] ?? json['flows']),
      updatedAt: DateTime.now(),
    );
  }

  static int _intValue(Object? value) {
    if (value is int) return value;
    if (value is num) return value.toInt();
    return int.tryParse(value?.toString() ?? '') ?? 0;
  }

  ServiceSnapshot copyWith({
    ServicePhase? phase,
    String? listener,
    Duration? uptime,
    int? admittedConnections,
    int? activeFlows,
    DateTime? updatedAt,
  }) {
    return ServiceSnapshot(
      phase: phase ?? this.phase,
      listener: listener ?? this.listener,
      uptime: uptime ?? this.uptime,
      admittedConnections: admittedConnections ?? this.admittedConnections,
      activeFlows: activeFlows ?? this.activeFlows,
      updatedAt: updatedAt ?? this.updatedAt,
    );
  }
}

class OperatorEvent {
  const OperatorEvent({
    required this.time,
    required this.title,
    required this.detail,
    required this.level,
  });

  final DateTime time;
  final String title;
  final String detail;
  final EventLevel level;
}

class SessionSample {
  const SessionSample({
    required this.id,
    required this.target,
    required this.carrier,
    required this.latencyMs,
    required this.transfer,
    required this.state,
  });

  final String id;
  final String target;
  final String carrier;
  final int latencyMs;
  final String transfer;
  final String state;
}

class VelumConfiguration {
  const VelumConfiguration({
    this.bind = '0.0.0.0:4433',
    this.certificate = '/etc/velum/cert.pem',
    this.privateKey = '/etc/velum/key.pem',
    this.adminSocket = '/run/user/1000/velum/admin.sock',
    this.credentialId = '1',
    this.credentialFile = '/etc/velum/credential.hex',
    this.allowedTargets = '203.0.113.10:443',
    this.maxSessions = 64,
    this.maxFlows = 16,
    this.maxConnections = 1024,
    this.maxStreams = 64,
    this.connectTimeout = 5,
    this.controlTimeout = 5,
    this.shutdownTimeout = 5,
  });

  final String bind;
  final String certificate;
  final String privateKey;
  final String adminSocket;
  final String credentialId;
  final String credentialFile;
  final String allowedTargets;
  final int maxSessions;
  final int maxFlows;
  final int maxConnections;
  final int maxStreams;
  final int connectTimeout;
  final int controlTimeout;
  final int shutdownTimeout;

  VelumConfiguration copyWith({
    String? bind,
    String? certificate,
    String? privateKey,
    String? adminSocket,
    String? credentialId,
    String? credentialFile,
    String? allowedTargets,
    int? maxSessions,
    int? maxFlows,
    int? maxConnections,
    int? maxStreams,
    int? connectTimeout,
    int? controlTimeout,
    int? shutdownTimeout,
  }) {
    return VelumConfiguration(
      bind: bind ?? this.bind,
      certificate: certificate ?? this.certificate,
      privateKey: privateKey ?? this.privateKey,
      adminSocket: adminSocket ?? this.adminSocket,
      credentialId: credentialId ?? this.credentialId,
      credentialFile: credentialFile ?? this.credentialFile,
      allowedTargets: allowedTargets ?? this.allowedTargets,
      maxSessions: maxSessions ?? this.maxSessions,
      maxFlows: maxFlows ?? this.maxFlows,
      maxConnections: maxConnections ?? this.maxConnections,
      maxStreams: maxStreams ?? this.maxStreams,
      connectTimeout: connectTimeout ?? this.connectTimeout,
      controlTimeout: controlTimeout ?? this.controlTimeout,
      shutdownTimeout: shutdownTimeout ?? this.shutdownTimeout,
    );
  }

  String toToml() {
    final targets = allowedTargets
        .split(RegExp(r'[,\n]'))
        .map((value) => value.trim())
        .where((value) => value.isNotEmpty)
        .map((value) => '"$value"')
        .join(', ');
    return '''version = 1
allowed_targets = [$targets]

[[credentials]]
id = $credentialId
secret_file = "$credentialFile"

[listener]
bind = "$bind"
certificate = "$certificate"
private_key = "$privateKey"

[admin]
socket = "$adminSocket"

[limits]
max_sessions = $maxSessions
max_flows_per_session = $maxFlows
max_connections = $maxConnections
max_streams_per_connection = $maxStreams
connect_timeout_secs = $connectTimeout
control_timeout_secs = $controlTimeout
shutdown_timeout_secs = $shutdownTimeout
''';
  }
}
