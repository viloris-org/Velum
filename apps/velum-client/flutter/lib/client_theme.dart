import 'package:flutter/material.dart';

abstract final class ClientTheme {
  static const background = Color(0xff07090d);
  static const panel = Color(0xff0d1117);
  static const panelRaised = Color(0xff131921);
  static const border = Color(0xff1c2533);
  static const borderStrong = Color(0xff243040);
  static const accent = Color(0xff00e5c8);
  static const text = Color(0xffd4dde8);
  static const muted = Color(0xff5a6a80);
  static const mutedDark = Color(0xff3a4a5c);
  static const danger = Color(0xffff4757);
  static const warning = Color(0xffffd32a);
  static const trafficDownload = Color(0xff65d6eb);
  static const trafficUpload = Color(0xff9ccca3);
  static const trafficGrid = Color(0xff151d27);

  static ThemeData data() => ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    scaffoldBackgroundColor: background,
    colorScheme: const ColorScheme.dark(
      primary: accent,
      surface: panel,
      onSurface: text,
      error: danger,
    ),
    textTheme: const TextTheme(
      bodyMedium: TextStyle(color: text),
      bodySmall: TextStyle(color: muted),
      titleMedium: TextStyle(color: text, fontWeight: FontWeight.w600),
    ),
    inputDecorationTheme: const InputDecorationTheme(
      filled: true,
      fillColor: panel,
      labelStyle: TextStyle(color: muted),
      helperStyle: TextStyle(color: muted, fontSize: 11),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.all(Radius.circular(8)),
        borderSide: BorderSide(color: border),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.all(Radius.circular(8)),
        borderSide: BorderSide(color: accent),
      ),
    ),
  );
}

class ClientPanel extends StatelessWidget {
  const ClientPanel({required this.child, super.key, this.padding});

  final Widget child;
  final EdgeInsetsGeometry? padding;

  @override
  Widget build(BuildContext context) => DecoratedBox(
    decoration: BoxDecoration(
      color: ClientTheme.panel,
      border: Border.all(color: ClientTheme.border),
      borderRadius: BorderRadius.circular(12),
    ),
    child: Padding(padding: padding ?? const EdgeInsets.all(24), child: child),
  );
}

class SectionLabel extends StatelessWidget {
  const SectionLabel(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) => Text(
    text.toUpperCase(),
    style: const TextStyle(
      color: ClientTheme.muted,
      fontSize: 11,
      fontWeight: FontWeight.w600,
      letterSpacing: 1.2,
    ),
  );
}
