import 'package:flutter/material.dart';

import 'controller/velum_controller.dart';
import 'services/velum_bridge.dart';
import 'ui/velum_app.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  final controller = VelumController(createVelumBridge());
  runApp(VelumApp(controller: controller));
}
