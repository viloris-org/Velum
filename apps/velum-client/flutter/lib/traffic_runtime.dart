import 'package:flutter/foundation.dart';

import 'native_client.dart';

/// Narrow runtime surface required by platform traffic adapters.
abstract interface class TrafficRuntime {
  ClientRuntimeSnapshot get snapshot;
  void addListener(VoidCallback listener);
  void removeListener(VoidCallback listener);
  int startLoopbackProxy({
    int requestedPort = 0,
    String routingRules = 'MATCH,PROXY',
  });
  void stopLoopbackProxy();
  int runtimeHandleForTun();
}
