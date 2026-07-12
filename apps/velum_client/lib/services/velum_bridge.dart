import 'bridge_contract.dart';
import 'velum_bridge_stub.dart'
    if (dart.library.io) 'velum_bridge_io.dart'
    as implementation;

export 'bridge_contract.dart';

VelumBridge createVelumBridge() => implementation.createBridge();
