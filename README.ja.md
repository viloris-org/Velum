# Velum

[![Required CI](https://github.com/viloris-org/Velum/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci.yml)
[![CI Health](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml/badge.svg?branch=main)](https://github.com/viloris-org/Velum/actions/workflows/ci-health.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.97+](https://img.shields.io/badge/Rust-1.97%2B-orange.svg)](rust-toolchain.toml)
[![Flutter 3.44.0](https://img.shields.io/badge/Flutter-3.44.0-02569B?logo=flutter&logoColor=white)](https://flutter.dev)

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

## 現在の検証

このリポジトリでは Node 22.22.2 と Rust 1.97.0 を固定しています。`cargo-deny`
0.20.2 をインストールしたうえで、現在のすべての Foundation ゲートを実行するには、
次を実行してください。

```bash
cargo xtask test
```

アーキテクチャおよびドキュメントのチェックは、それぞれ `cargo xtask architecture`
および `cargo xtask docs` で個別に実行することもできます。

## サーバーデプロイ

チェックサムを検証するインストーラーで公開リリースをインストールします。`velum`
コマンドは `~/.local/bin` にインストールされ、そのディレクトリがシェルの `PATH` に
追加されます。リリース版には stable チャンネルを、プレリリース版には beta
チャンネルを選択してください。

> **どのチャンネルを使うべきですか？** `stable` は最新の安定版 `vX.Y.Z` を
> インストールし、利用可能な場合に推奨されます。`beta` は最新のプレリリースを
> インストールするため、未完成または変更された動作が含まれる場合があります。どちらも
> 変動する `--latest` を使用します。再現可能なインストールには `--version vX.Y.Z`
> または `--version vX.Y.Z-beta` を使用してください。

### Stable チャンネル

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/main/scripts/install.sh
sh ./install.sh --channel stable --latest --add-to-path
```

### Beta チャンネル

```bash
curl --fail --location --remote-name \
  https://raw.githubusercontent.com/viloris-org/Velum/main/scripts/install.sh
sh ./install.sh --channel beta --latest --add-to-path
```

新しいシェルを開き、Linux では relay を systemd ユーザーサービスとしてデプロイします。

```bash
velum setup --config ~/.config/velum/config.toml
velum config validate --config ~/.config/velum/config.toml
velum deploy --config ~/.config/velum/config.toml
```

`setup` は relay の設定と認証情報を作成し、TLS マテリアルを設定します。`deploy` は
それらのファイルを検証してから systemd ユーザーサービスを作成して開始します。デプロイ
済み relay の操作には、同じ `--config` パスで `velum status`、`velum drain`、
`velum shutdown` を使用します。ソースからビルドする場合は、`cargo build --release -p
velum-node --bin velum` を実行し、同じコマンドを使う前に `./target/release` を `PATH` に
追加してください。

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
