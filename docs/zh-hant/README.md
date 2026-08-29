# 🍌 Banana: 通用分佈式基礎設施、P2P Swarm 與 OCI 容器構建引擎

> 🌐 **Language Navigation / 多语言文档 / 多語言文檔 / ドキュメント言語:**
> [English](../../README.md) | [Tiếng Việt](../vi/README.md) | [日本語](../ja/README.md) | [简体中文](../zh-hans/README.md) | [繁體中文](README.md)

[![Banana CI](https://github.com/requla11/banana/actions/workflows/ci.yml/badge.svg)](https://github.com/requla11/banana/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](../../LICENSE)
[![Rust 2024](https://img.shields.io/badge/Rust-2024%20Edition-orange.svg)](../../Cargo.toml)
[![MSRV: 1.88+](https://img.shields.io/badge/MSRV-1.88%2B-purple.svg)](../../Cargo.toml)

---

## 📖 項目概述

**Banana** 是專為現代多工具鏈構建生態系統（如 [Fish](https://github.com/requla11/fish) 和 [Apple](https://github.com/requla11/apple)）設計的通用分發、網絡與軟件供應鏈基礎設施套件。

Banana 完全採用高性能 Rust 編寫，封裝了 5 項核心基礎設施能力，無需中心化雲端服務器或繁重的系統守護進程即可獨立運行。

---

## 🚀 5 大核心基礎設施能力

```mermaid
graph TD
    BananaCLI["🍌 banana (多功能統一 CLI)"]
    
    subgraph "Banana 獨立基礎設施引擎"
        P2P["1. 🌐 banana-p2p<br/><b>P2P Swarm 局域網緩存</b><br/>(零服務器 Wi-Fi 產物高速分發)"]
        OCI["2. 🐳 banana-oci<br/><b>免 Docker OCI 鏡像構建器</b><br/>(無需 dockerd 構建 &lt;5MB 精簡鏡像)"]
        Ledger["3. 🐙 banana-ledger<br/><b>SLSA v1.0 Merkle 樹見證賬本</b><br/>(防篡改供應鏈密碼學簽名)"]
        Telemetry["4. 🦞 banana-telemetry<br/><b>硬件功耗與綠色計算遙測</b><br/>(基於 RAPL 測量焦耳與碳足跡)"]
        AST["5. 🪸 banana-ast<br/><b>多語言語義 AST 分析引擎</b><br/>(支持 15+ 語言的極速依賴圖解析)"]
    end

    BananaCLI --> P2P
    BananaCLI --> OCI
    BananaCLI --> Ledger
    BananaCLI --> Telemetry
    BananaCLI --> AST
```

1. 🌐 **P2P Swarm 局域網緩存網絡 (`banana-p2p`)**:
   - 基於 mDNS 的零配置節點自動發現，通過 BLAKE3 分塊在局域網內高速共享構建產物（0 雲端服務器費用）。
2. 🐳 **免 Docker OCI 容器構建器 (`banana-oci`)**:
   - 直接將 rootfs 目錄編譯為符合 OCI v1.0 規範的容器鏡像 tarball，無需安裝 Docker Desktop、`dockerd` 或 root 特權。
3. 🐙 **SLSA v1.0 密碼學溯源賬本 (`banana-ledger`)**:
   - 驗證 in-toto 溯源憑證，將產物哈希記錄至防篡改 Merkle 樹，並使用 Ed25519 密鑰對根哈希進行數字簽名。
4. 🦞 **硬件綠色能耗與遙測分析器 (`banana-telemetry`)**:
   - 通過 CPU RAPL 硬件計數器精確測量焦耳 (Joules) 級能量消耗，並預估碳排放影響。
5. 🪸 **多語言語義 AST 分析引擎 (`banana-ast`)**:
   - 支持 Rust、Python、TypeScript、JavaScript、Go、C++ 的極速靜態分析與符號依賴拓撲生成。

---

## 🛠️ 安裝與快速上手

```bash
# 從源碼編譯安裝
cargo install --path .

# 啟動局域網 P2P 節點
banana p2p --addr 127.0.0.1:8080 --node-id node-alpha

# 構建 OCI 容器鏡像 (無需 Docker)
banana oci --rootfs ./dist --output ./distroless-app.tar --working-dir /app

# 將構建產物記錄至密碼學安全賬本
banana ledger --artifact my-app --hash blake3:9a12bc...

# 測量能耗與碳足跡
banana telemetry --tdp-watts 65.0 --grid-intensity 300.0

# 提取源碼中的函數與結構體符號
banana ast --file ./src/main.rs
```

---

## 📜 開源協議

本項目採用 [Apache License 2.0](../../LICENSE) 或 [MIT License](../../LICENSE) 雙重許可。
