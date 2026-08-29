# 🍌 Banana: 汎用分散インフラ、P2P Swarm、OCI コンテナ構築エンジン

> 🌐 **Language Navigation / 多语言文档 / 多語言文檔 / ドキュメント言語:**
> [English](../../README.md) | [Tiếng Việt](../vi/README.md) | [日本語](README.md) | [简体中文](../zh-hans/README.md) | [繁體中文](../zh-hant/README.md)

[![Banana CI](https://github.com/requla11/banana/actions/workflows/ci.yml/badge.svg)](https://github.com/requla11/banana/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](../../LICENSE)
[![Rust 2024](https://img.shields.io/badge/Rust-2024%20Edition-orange.svg)](../../Cargo.toml)
[![MSRV: 1.88+](https://img.shields.io/badge/MSRV-1.88%2B-purple.svg)](../../Cargo.toml)

---

## 📖 プロジェクト概要

**Banana** は、現代のマルチツールチェーンビルドエコシステム（[Fish](https://github.com/requla11/fish) や [Apple](https://github.com/requla11/apple) など）向けに設計された、汎用配信・ネットワーキング・ソフトウェアサプライチェーンインフラストラクチャスイートです。

高性能な Rust で完全に構築され、中央集権型のクラウドサーバーや重量級のデーモンを必要とせずに独立して動作する 5 つのコアインフラ機能をカプセル化しています。

---

## 🚀 5 つのコアインフラ機能

```mermaid
graph TD
    BananaCLI["🍌 banana (多機能統合 CLI)"]
    
    subgraph "Banana 独立インフラエンジン"
        P2P["1. 🌐 banana-p2p<br/><b>P2P Swarm LAN キャッシュ</b><br/>(サーバー不要の Wi-Fi 成果物ストリーミング)"]
        OCI["2. 🐳 banana-oci<br/><b>Docker 不要の OCI ビルダー</b><br/>(dockerd なしで &lt;5MB の Distroless 構築)"]
        Ledger["3. 🐙 banana-ledger<br/><b>SLSA v1.0 Merkle 木台帳</b><br/>(改ざん防止サプライチェーン暗号署名)"]
        Telemetry["4. 🦞 banana-telemetry<br/><b>ハードウェア電力とグリーン計算</b><br/>(RAPL によるジュール測定と炭素排出量推定)"]
        AST["5. 🪸 banana-ast<br/><b>多言語対応 AST 構文解析エンジン</b><br/>(15+ 言語の高速依存関係グラフ抽出)"]
    end

    BananaCLI --> P2P
    BananaCLI --> OCI
    BananaCLI --> Ledger
    BananaCLI --> Telemetry
    BananaCLI --> AST
```

1. 🌐 **P2P Swarm LAN キャッシュメッシュ (`banana-p2p`)**:
   - mDNS によるゼロコンフィグ自動ピア検出と BLAKE3 チャンク分割により、ローカル Wi-Fi/LAN 内で成果物を超高速共有（クラウドサーバー費用 0 円）。
2. 🐳 **Docker 不要の OCI コンテナビルダー (`banana-oci`)**:
   - Docker Desktop、`dockerd`、root 権限を一切使わずに、rootfs ディレクトリを OCI v1.0 準拠のコンテナ tarball（5MB 未満の Distroless）に直接ビルド。
3. 🐙 **SLSA v1.0 サプライチェーン暗号台帳 (`banana-ledger`)**:
   - in-toto 真正性証明を検証し、成果物ハッシュを改ざん不可能な Merkle 木に記録し、Ed25519 鍵でルート署名を実施。
4. 🦞 **ハードウェアグリーンエネルギーと遠隔測定 (`banana-telemetry`)**:
   - CPU RAPL ハードウェアカウンターを通じてジュール単位の消費電力を精密測定し、CO2 排出量を算出。
5. 🪸 **多言語セマンティック AST 解析エンジン (`banana-ast`)**:
   - Rust、Python、TypeScript、JavaScript、Go、C++ の静的解析とシンボル依存関係トポロジーを高速生成。

---

## 🛠️ インストールとクイックスタート

```bash
# ソースからインストール
cargo install --path .

# ローカル P2P ノードの起動
banana p2p --addr 127.0.0.1:8080 --node-id node-alpha

# Docker なしで OCI コンテナをビルド
banana oci --rootfs ./dist --output ./distroless-app.tar --working-dir /app

# ビルド成果物を暗号台帳に記録
banana ledger --artifact my-app --hash blake3:9a12bc...

# 消費電力と炭素影響の測定
banana telemetry --tdp-watts 65.0 --grid-intensity 300.0

# ソースコードから関数・構造体シンボルを抽出
banana ast --file ./src/main.rs
```

---

## 📜 ライセンス

本プロジェクトは [Apache License 2.0](../../LICENSE) または [MIT License](../../LICENSE) のデュアルライセンスで提供されています。
