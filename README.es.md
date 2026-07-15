# Velum

[![Required CI](https://github.com/viloris-org/Velum/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci.yml)
[![CI Health](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.97+](https://img.shields.io/badge/Rust-1.97%2B-orange.svg)](rust-toolchain.toml)

[English](README.md) | [Español](README.es.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

Velum es un protocolo de tunelización cifrada en etapa de investigación para
redes restringidas, inestables y heterogéneas.

Su principal diferenciador previsto es la continuidad de sesión entre varios
transportes: la misma sesión lógica puede adaptarse entre QUIC/UDP y TLS/TCP
sin que las aplicaciones tengan que elegir un protocolo de antemano. Velum
también considera el camuflaje como coexistencia nativa con servicios reales de
Internet, no como una opción para ofuscar paquetes.

> Estado del proyecto: exploración de posicionamiento y arquitectura. Aún no
> existe un protocolo de cable ni una afirmación de seguridad estable.

## Dirección de Diseño

- Preservar los flujos logicos mientras cambian las rutas de red y los transportes.
- Dar a los flujos, mensajes y datagramas semánticas de entrega distintas.
- Usar transportes criptográficos estándar; no inventar criptografía.
- Hacer que los puntos finales no autenticados se comporten como servicios reales.
- Medir las afirmaciones de rendimiento, degradación y detectabilidad.
- Mantener la implementación en Rust dividida por responsabilidad y capa de protocolo.

Comience con el [índice de documentación](docs/README.md) y el
[estado de implementación y hoja de ruta](docs/roadmap.md).

## Validación Actual

El repositorio fija Node 22.22.2 y Rust 1.97.0. Con `cargo-deny` 0.20.2
instalado, ejecute todas las verificaciones actuales de Foundation con:

```bash
cargo xtask test
```

Las comprobaciones de arquitectura y documentación también están disponibles
por separado como `cargo xtask architecture` y `cargo xtask docs`.

## Objetivos que Actualmente No se Persiguen

- Afirmar que es indetectable o imposible de bloquear.
- Diseñar una nueva suite de cifrado o un reemplazo de TLS.
- Reemplazar MASQUE, WireGuard o todos los proxies de aplicaciones.
- Incluir anonimato de múltiples saltos en la primera versión del protocolo.
- Congelar un formato de cable antes de que los experimentos exploratorios tengan éxito.

Velum está licenciado bajo la [Licencia Apache 2.0](LICENSE). Las expectativas
para contribuciones, seguridad, soporte y lanzamientos se definen en las
políticas correspondientes del repositorio.

## Descargo de Responsabilidad

Velum es software experimental de investigación. No ha recibido una auditoría
de seguridad y no debe utilizarse como base para la seguridad, privacidad,
disponibilidad en producción ni para eludir restricciones de red. Úselo solo
cuando esté autorizado para ello y acepte todos los riesgos asociados.
