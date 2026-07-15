import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:flutter/services.dart';

typedef _TunRunV2Native =
    Int32 Function(Uint64 runtimeHandle, Int32 tunFd, Uint16 mtu);
typedef _TunRunV2Dart = int Function(int runtimeHandle, int tunFd, int mtu);
typedef _TunStopNative = Int32 Function();
typedef _TunStopDart = int Function();

final class TunRoute {
  const TunRoute(this.address, this.prefixLength);

  final String address;
  final int prefixLength;

  Map<String, Object> toMethodArguments() => {
    'address': address,
    'prefixLength': prefixLength,
  };
}

final class TunOptions {
  factory TunOptions({
    String address = '172.19.0.1',
    int prefixLength = 30,
    String ipv6Address = 'fd00:19::1',
    int ipv6PrefixLength = 126,
    int mtu = 1500,
    Iterable<String> dnsServers = const ['8.8.8.8', '2001:4860:4860::8888'],
    Iterable<TunRoute> routes = const [
      TunRoute('0.0.0.0', 0),
      TunRoute('::', 0),
    ],
  }) {
    final dns = List<String>.unmodifiable(dnsServers);
    final configuredRoutes = List<TunRoute>.unmodifiable(routes);
    if (address.trim().isEmpty) throw ArgumentError.value(address, 'address');
    if (prefixLength < 0 || prefixLength > 32) {
      throw ArgumentError.value(prefixLength, 'prefixLength');
    }
    if (ipv6Address.trim().isEmpty) {
      throw ArgumentError.value(ipv6Address, 'ipv6Address');
    }
    if (ipv6PrefixLength < 0 || ipv6PrefixLength > 128) {
      throw ArgumentError.value(ipv6PrefixLength, 'ipv6PrefixLength');
    }
    if (mtu < 576 || mtu > 65535) throw ArgumentError.value(mtu, 'mtu');
    if (dns.any((server) => server.trim().isEmpty)) {
      throw ArgumentError.value(dnsServers, 'dnsServers');
    }
    for (final route in configuredRoutes) {
      final address = InternetAddress.tryParse(route.address);
      final maxPrefix = address?.type == InternetAddressType.IPv6 ? 128 : 32;
      if (address == null ||
          route.prefixLength < 0 ||
          route.prefixLength > maxPrefix) {
        throw ArgumentError.value(routes, 'routes');
      }
    }
    return TunOptions._(
      address,
      prefixLength,
      ipv6Address,
      ipv6PrefixLength,
      mtu,
      dns,
      configuredRoutes,
    );
  }

  const TunOptions._(
    this.address,
    this.prefixLength,
    this.ipv6Address,
    this.ipv6PrefixLength,
    this.mtu,
    this.dnsServers,
    this.routes,
  );

  final String address;
  final int prefixLength;
  final String ipv6Address;
  final int ipv6PrefixLength;
  final int mtu;
  final List<String> dnsServers;
  final List<TunRoute> routes;

  Map<String, Object> toMethodArguments() => {
    'address': address,
    'prefixLength': prefixLength,
    'ipv6Address': ipv6Address,
    'ipv6PrefixLength': ipv6PrefixLength,
    'mtu': mtu,
    'dnsServers': dnsServers,
    'routes': routes.map((route) => route.toMethodArguments()).toList(),
  };
}

/// Android consent, foreground service, and native packet-engine lifecycle.
class AndroidVpn {
  AndroidVpn();

  static const _channel = MethodChannel('org.velum.velum_client/vpn');
  Future<void>? _engine;
  Future<void>? _completion;
  String? _libraryPath;

  Future<void>? get completion => _completion;

  Future<bool> requestPermission() async {
    if (!Platform.isAndroid) return false;
    return await _channel.invokeMethod<bool>('requestPermission') ?? false;
  }

  Future<void> start({
    required int runtimeHandle,
    required String libraryPath,
    required TunOptions options,
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
    final tunFd = await _channel.invokeMethod<int>(
      'start',
      options.toMethodArguments(),
    );
    if (tunFd == null || tunFd < 0) {
      throw StateError('Android did not create a TUN fd.');
    }
    _libraryPath = libraryPath;
    final engine = Isolate.run(() {
      final library = DynamicLibrary.open(libraryPath);
      final run = library
          .lookup<NativeFunction<_TunRunV2Native>>(
            'velum_client_android_tun_run_v2',
          )
          .asFunction<_TunRunV2Dart>();
      final status = run(runtimeHandle, tunFd, options.mtu);
      if (status != 0) {
        throw StateError('Native TUN engine failed with status $status.');
      }
    });
    _engine = engine;
    _completion = engine;
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
    _completion = null;
    _libraryPath = null;
  }
}
