import 'dart:io';

import 'package:flutter/services.dart';

/// Android-only consent boundary for the system VPN service.
abstract interface class AndroidVpnPermission {
  Future<bool> request();
}

class PlatformAndroidVpnPermission implements AndroidVpnPermission {
  const PlatformAndroidVpnPermission();

  static const _channel = MethodChannel('org.velum.velum_client/vpn');

  @override
  Future<bool> request() async {
    if (!Platform.isAndroid) return false;
    return await _channel.invokeMethod<bool>('requestPermission') ?? false;
  }
}
