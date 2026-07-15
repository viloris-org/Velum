import 'dart:io';

import 'package:flutter/services.dart';

import 'android_vpn.dart';

abstract interface class DesktopTunControl {
  Future<void> recover();
  Future<void> start({
    required int runtimeHandle,
    required int profileGeneration,
    required TunOptions options,
  });
  Future<void> stop();
}

/// Lifecycle-only bridge to an installed privileged desktop traffic host.
final class DesktopTunHost implements DesktopTunControl {
  DesktopTunHost({MethodChannel? channel})
    : _channel =
          channel ?? const MethodChannel('org.velum.velum_client/desktop_tun');

  static bool get buildEnabled =>
      const bool.fromEnvironment('VELUM_EXPERIMENTAL_DESKTOP_TUN') &&
      (Platform.isWindows || Platform.isLinux || Platform.isMacOS);

  final MethodChannel _channel;

  @override
  Future<void> recover() async {
    await _channel.invokeMethod<bool>('recover');
  }

  @override
  Future<void> start({
    required int runtimeHandle,
    required int profileGeneration,
    required TunOptions options,
  }) async {
    final started = await _channel.invokeMethod<bool>('start', {
      'runtimeHandle': runtimeHandle,
      'profileGeneration': profileGeneration,
      ...options.toMethodArguments(),
    });
    if (started != true) throw StateError('Desktop TUN host rejected start.');
  }

  @override
  Future<void> stop() async {
    final stopped = await _channel.invokeMethod<bool>('stop');
    if (stopped != true) throw StateError('Desktop TUN host rejected stop.');
  }
}
