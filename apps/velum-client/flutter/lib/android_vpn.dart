import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:flutter/services.dart';

typedef _TunRunNative = Int32 Function(Uint64 runtimeHandle, Int32 tunFd);
typedef _TunRunDart = int Function(int runtimeHandle, int tunFd);
typedef _TunStopNative = Int32 Function();
typedef _TunStopDart = int Function();

/// Android consent, foreground service, and native packet-engine lifecycle.
class AndroidVpn {
  AndroidVpn();

  static const _channel = MethodChannel('org.velum.velum_client/vpn');
  Future<void>? _engine;
  String? _libraryPath;

  Future<bool> requestPermission() async {
    if (!Platform.isAndroid) return false;
    return await _channel.invokeMethod<bool>('requestPermission') ?? false;
  }

  Future<void> start({
    required int runtimeHandle,
    required String libraryPath,
  }) async {
    if (!Platform.isAndroid) {
      throw UnsupportedError('Android VPN is unavailable on this platform.');
    }
    if (runtimeHandle < 0) {
      throw ArgumentError.value(runtimeHandle, 'runtimeHandle');
    }
    if (_engine != null) {
      throw StateError('Android VPN is already running.');
    }
    final tunFd = await _channel.invokeMethod<int>('start');
    if (tunFd == null || tunFd < 0) {
      throw StateError('Android did not create a TUN fd.');
    }
    _libraryPath = libraryPath;
    final engine = Isolate.run(() {
      final library = DynamicLibrary.open(libraryPath);
      final run = library
          .lookup<NativeFunction<_TunRunNative>>('velum_client_android_tun_run')
          .asFunction<_TunRunDart>();
      final status = run(runtimeHandle, tunFd);
      if (status != 0) {
        throw StateError('Native TUN engine failed with status $status.');
      }
    });
    _engine = engine;
    unawaited(
      engine.then<void>((_) {}, onError: (_, _) {}).whenComplete(() {
        if (identical(_engine, engine)) _engine = null;
        unawaited(_channel.invokeMethod<bool>('stop'));
      }),
    );
  }

  Future<void> stop() async {
    if (!Platform.isAndroid) return;
    final engine = _engine;
    final libraryPath = _libraryPath;
    if (engine != null && libraryPath != null) {
      final library = DynamicLibrary.open(libraryPath);
      final stop = library
          .lookup<NativeFunction<_TunStopNative>>(
            'velum_client_android_tun_stop',
          )
          .asFunction<_TunStopDart>();
      if (stop() != 0) throw StateError('Native TUN engine did not stop.');
      try {
        await engine;
      } on Object {
        // The service is still removed below after a native engine failure.
      }
    }
    await _channel.invokeMethod<bool>('stop');
    _engine = null;
    _libraryPath = null;
  }
}
