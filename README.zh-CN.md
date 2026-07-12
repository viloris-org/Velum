# Velum

[![Required CI](https://github.com/viloris-org/Velum/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci.yml)
[![CI Health](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.97+](https://img.shields.io/badge/Rust-1.97%2B-orange.svg)](rust-toolchain.toml)

[English](README.md) | [Español](README.es.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

Velum 是一个仍处于研究阶段的加密隧道协议，面向受限、不稳定且异构的网络环境。

其预期的核心差异在于支持跨多种承载方式保持会话连续性：同一逻辑会话可在
QUIC/UDP 与 TLS/TCP 之间自适应切换，而无需让应用程序预先选择协议。Velum 还将
伪装视作与真实互联网服务的原生共存，而非一个数据包混淆开关。

> 项目状态：正在进行定位与架构探索。尚无稳定的线协议或安全性声明。

## 设计方向

- 在网络路径和承载方式变化时保留逻辑流。
- 为流、消息和数据报提供不同的传递语义。
- 使用标准密码学传输；不自行设计密码学方案。
- 让未经身份验证的端点表现为真实服务。
- 衡量性能、降级情况和可检测性声明。
- 按职责与协议层划分 Rust 实现。

请从[文档索引](docs/README.md)和[实现状态与路线图](docs/roadmap.md)开始了解项目。

## 实验性运维

`velum` 研究 CLI 可通过 `velum deploy --config PATH` 校验已完成配置，并将其部署为
systemd 用户服务。这是本地进程生命周期助手，而不是生产可用的一键基础设施安装器：
证书、密钥、DNS、防火墙、监控、升级与回滚仍由运维人员负责。使用前请阅读
[运维指南](docs/velum-node.md)。

从与目标 snapshot 相同的不可变标签下载安装脚本，并在本地执行，可安装指定研究快照：

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/snapshot-EXAMPLE/scripts/install.sh
sh ./install.sh --version snapshot-EXAMPLE --add-to-path
```

在下发配置、凭据文件和 PEM 材料后，以当前用户部署中继服务：

```bash
velum config validate --config /srv/velum/config.toml
velum deploy --config /srv/velum/config.toml
velum status --format json --config /srv/velum/config.toml
```

安装后请打开新的 shell，或在当前 shell 中运行
`export PATH="$HOME/.local/bin:$PATH"`。`--add-to-path` 只修改当前用户的 shell
启动文件；若 PATH 由其他方式统一管理，请省略该选项。

## 当前验证

此仓库固定使用 Node 22.22.2 和 Rust 1.97.0。安装 `cargo-deny` 0.20.2 后，可通过以下命令运行当前所有 Foundation 检查：

```bash
cargo xtask test
```

架构和文档检查也可单独通过 `cargo xtask architecture` 和 `cargo xtask docs` 运行。

## 当前非目标

- 声称无法被检测或无法被封锁。
- 设计新的密码套件或 TLS 替代方案。
- 取代 MASQUE、WireGuard 或所有应用层代理。
- 在第一个协议版本中提供多跳匿名性。
- 在示踪实验成功之前冻结线格式。

Velum 采用 [Apache License 2.0](LICENSE) 许可证。贡献、安全、支持和发布相关预期均定义于对应的仓库政策中。

## 免责声明

Velum 是实验性研究软件，尚未经过安全审计，绝不可依赖其提供生产环境级别的安全性、隐私性、可用性或规避网络限制的能力。请仅在获得授权的场景中使用，并自行承担所有相关风险。
