import 'package:flutter/material.dart';

abstract final class VelumColors {
  static const ink = Color(0xFF07131A);
  static const deep = Color(0xFF0B1D27);
  static const panel = Color(0xFF102833);
  static const panelRaised = Color(0xFF153540);
  static const mist = Color(0xFFE6F0F0);
  static const muted = Color(0xFF8FA8AA);
  static const aqua = Color(0xFF62D6CE);
  static const aquaSoft = Color(0xFFB8EEE8);
  static const amber = Color(0xFFF3B56C);
  static const coral = Color(0xFFF27E6D);
  static const line = Color(0xFF27434B);
}

abstract final class VelumTheme {
  static ThemeData dark() {
    final scheme =
        ColorScheme.fromSeed(
          seedColor: VelumColors.aqua,
          brightness: Brightness.dark,
          surface: VelumColors.deep,
        ).copyWith(
          primary: VelumColors.aqua,
          secondary: VelumColors.amber,
          error: VelumColors.coral,
          surface: VelumColors.deep,
          onSurface: VelumColors.mist,
          outline: VelumColors.line,
        );

    return ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      colorScheme: scheme,
      scaffoldBackgroundColor: VelumColors.ink,
      fontFamily: 'Segoe UI Variable',
      dividerColor: VelumColors.line,
      textTheme: const TextTheme(
        displaySmall: TextStyle(
          fontSize: 38,
          height: 1.08,
          fontWeight: FontWeight.w700,
          letterSpacing: -1.3,
          color: VelumColors.mist,
        ),
        headlineMedium: TextStyle(
          fontSize: 26,
          height: 1.15,
          fontWeight: FontWeight.w600,
          letterSpacing: -0.6,
        ),
        titleLarge: TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
        titleMedium: TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
        bodyLarge: TextStyle(fontSize: 15, height: 1.55),
        bodyMedium: TextStyle(fontSize: 13.5, height: 1.45),
        labelLarge: TextStyle(fontSize: 13, fontWeight: FontWeight.w600),
        labelSmall: TextStyle(
          fontSize: 10.5,
          fontWeight: FontWeight.w700,
          letterSpacing: 1.1,
        ),
      ).apply(bodyColor: VelumColors.mist, displayColor: VelumColors.mist),
      cardTheme: const CardThemeData(
        color: VelumColors.deep,
        elevation: 0,
        margin: EdgeInsets.zero,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.all(Radius.circular(18)),
          side: BorderSide(color: VelumColors.line),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: VelumColors.ink.withValues(alpha: 0.56),
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 14,
          vertical: 13,
        ),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: const BorderSide(color: VelumColors.line),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: const BorderSide(color: VelumColors.line),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: const BorderSide(color: VelumColors.aqua, width: 1.4),
        ),
        labelStyle: const TextStyle(color: VelumColors.muted),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          foregroundColor: VelumColors.ink,
          backgroundColor: VelumColors.aqua,
          padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
          textStyle: const TextStyle(fontWeight: FontWeight.w700),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          foregroundColor: VelumColors.mist,
          side: const BorderSide(color: VelumColors.line),
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
        ),
      ),
      navigationRailTheme: const NavigationRailThemeData(
        backgroundColor: Colors.transparent,
        indicatorColor: VelumColors.panelRaised,
        selectedIconTheme: IconThemeData(color: VelumColors.aqua),
        unselectedIconTheme: IconThemeData(color: VelumColors.muted),
      ),
      snackBarTheme: const SnackBarThemeData(
        backgroundColor: VelumColors.panelRaised,
        contentTextStyle: TextStyle(color: VelumColors.mist),
        behavior: SnackBarBehavior.floating,
      ),
    );
  }
}
