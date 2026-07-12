import 'package:flutter/material.dart';

import '../controller/velum_controller.dart';
import '../theme/velum_theme.dart';
import 'app_shell.dart';

class VelumApp extends StatelessWidget {
  const VelumApp({super.key, required this.controller});

  final VelumController controller;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Velum Console',
      debugShowCheckedModeBanner: false,
      theme: VelumTheme.dark(),
      home: AnimatedBuilder(
        animation: controller,
        builder: (context, _) => AppShell(controller: controller),
      ),
    );
  }
}
