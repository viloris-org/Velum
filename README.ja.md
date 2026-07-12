# Velum

[![Required CI](https://github.com/viloris-org/Velum/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci.yml)
[![CI Health](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.97+](https://img.shields.io/badge/Rust-1.97%2B-orange.svg)](rust-toolchain.toml)

[English](README.md) | [Español](README.es.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

Velum は、制限のある、不安定な、異種混在のネットワークを対象とした、
研究段階の暗号化トンネリングプロトコルです。

主な差別化要因は、複数のキャリアをまたぐセッション継続性です。同一の論理
セッションが、アプリケーションに事前のプロトコル選択を求めることなく、
QUIC/UDP と TLS/TCP の間で適応できます。Velum では、カモフラージュを
パケット難読化の切り替え機能ではなく、実在するインターネットサービスとの
ネイティブな共存として扱います。

> プロジェクトの状態: 位置づけとアーキテクチャを探索中です。ワイヤプロトコル
> およびセキュリティに関する主張は、まだ確定していません。

## 設計の方向性

- ネットワークパスやキャリアが変化しても、論理フローを維持する。
- ストリーム、メッセージ、データグラムに、それぞれ異なる配送セマンティクスを与える。
- 標準的な暗号化トランスポートを使用し、独自の暗号技術は作らない。
- 未認証のエンドポイントを、実在するサービスのように振る舞わせる。
- パフォーマンス、劣化、検出可能性に関する主張を測定する。
- Rust 実装を、責務とプロトコルレイヤーごとに分割して維持する。

[ドキュメントの索引](docs/README.md)および
[実装状況とロードマップ](docs/roadmap.md)から始めてください。

## 実験的な運用

研究用 CLI の `velum` は、準備済みの設定を検証し、
`velum deploy --config PATH` により systemd ユーザーサービスとして配置できます。
これはローカルプロセスのライフサイクルを補助するものであり、本番向けのワンクリック
インフラストラクチャインストーラーではありません。証明書、シークレット、DNS、
ファイアウォール、監視、アップグレード、ロールバックの準備は引き続き運用者の責任です。
使用前に[運用ガイド](docs/velum-node.md)を確認してください。

チャンネルを選び、対応するコマンドをそのまま実行してください。インストーラーが、
該当する最新の公開リリースを解決します。

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/main/scripts/install.sh && \
sh ./install.sh --channel stable --latest --add-to-path
```

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/main/scripts/install.sh && \
sh ./install.sh --channel beta --latest --add-to-path
```

再現可能なインストールでは、確認済みの固定タグからインストーラーを取得し、
正確なバージョンを指定してください。

```bash
INSTALLER_TAG='vX.Y.Z'
curl --fail --location --remote-name \
  "https://raw.githubusercontent.com/viloris-org/Velum/${INSTALLER_TAG}/scripts/install.sh"

sh ./install.sh --channel beta --version vX.Y.Z-beta --add-to-path
```

簡易コマンドは `main` から現在のインストーラーを取得し、`--latest` は可変参照です。
インストーラーはダウンロード前に解決したタグを表示します。インストールを記録または
再現する場合は、固定版を使用してください。対話型端末で実行した場合、インストーラーは
ただちに `velum setup` を起動し、初回設定を開始します。

設定、認証情報ファイル、PEM マテリアルを準備した後、現在のユーザーとしてリレーを
配置します。

```bash
velum config validate --config /srv/velum/config.toml
velum deploy --config /srv/velum/config.toml
velum status --format json --config /srv/velum/config.toml
```

## 現在の検証

このリポジトリでは Node 22.22.2 と Rust 1.97.0 を固定しています。`cargo-deny`
0.20.2 をインストールしたうえで、現在のすべての Foundation ゲートを実行するには、
次を実行してください。

```bash
cargo xtask test
```

アーキテクチャおよびドキュメントのチェックは、それぞれ `cargo xtask architecture`
および `cargo xtask docs` で個別に実行することもできます。

## 現在の非目標

- 検出不能またはブロック不能であると主張すること。
- 新しい暗号スイートや TLS の代替を設計すること。
- MASQUE、WireGuard、またはすべてのアプリケーションプロキシを置き換えること。
- 最初のプロトコルバージョンでマルチホップ匿名性を提供すること。
- トレーサー実験が成功する前にワイヤ形式を固定すること。

Velum は [Apache License 2.0](LICENSE) のもとでライセンスされています。貢献、
セキュリティ、サポート、リリースに関する期待事項は、それぞれ対応する
リポジトリポリシーで定められています。

## 免責事項

Velum は実験的な研究用ソフトウェアです。セキュリティ監査を受けておらず、
本番環境のセキュリティ、プライバシー、可用性、またはネットワーク制限の回避を
目的として依拠してはなりません。使用が許可された場所でのみ使用し、関連する
すべてのリスクを受け入れてください。
