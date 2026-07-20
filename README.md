# nanogram desktop LauncheR

Windows向けの高速デスクトップアプリランチャー。Tauri v2 + React + Rust で構築。

[![Release](https://img.shields.io/github/v/release/nanogram-inc/nanogram-desktop-launcher?display_name=tag&sort=semver)](https://github.com/nanogram-inc/nanogram-desktop-launcher/releases/latest)
[![Build](https://github.com/nanogram-inc/nanogram-desktop-launcher/actions/workflows/release.yml/badge.svg)](https://github.com/nanogram-inc/nanogram-desktop-launcher/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D4)](https://github.com/nanogram-inc/nanogram-desktop-launcher/releases/latest)

---

## 概要

nanogram desktop LauncheR は、キーボード操作を中心とした軽量・高速なアプリケーションランチャーです。Ctrl キーのダブルタップで即座に起動し、インクリメンタル検索でアプリを素早く見つけて起動できます。

フレームレスの透過ウィンドウでデスクトップに自然に溶け込み、フォーカスを失うと自動的に非表示になるため、作業の邪魔になりません。

## 特徴

- ⚡ **Ctrl×2（ダブルタップ）で瞬時に起動** — カスタムホットキー対応（Alt×2, Ctrl+Space など）
- 🔍 **インクリメンタル検索 + スマートランキング** — 使用頻度・最終使用日時・タグを加味して順位付け
- 📌 **固定エリア（ピン留め）** — アプリ・ファイル・フォルダ・コマンドをグリッドに配置、ドラッグで並び替え
- 🗂 **柔軟なアイテム登録** — フォルダスキャン / スタートメニュー一括取込 / ドラッグ&ドロップ / シェルコマンド登録
- 🕑 **最近使った・よく使うセクション** — 表示/非表示・順序をカスタマイズ可能
- 🖥 **マルチモニター & DPI 対応** — カーソルのあるモニター中央に正確に表示
- 🚫 **重複起動防止** — 起動済みアプリは既存ウィンドウを前面に復帰
- 💼 **ポータブル構成** — 全データを EXE 隣に保存、フォルダごとコピーで別 PC へ移行可能
- 🎨 **半透過 UI + ライト/ダークテーマ** — フォーカス喪失で自動非表示

## スクリーンショット

![nanogram desktop LauncheR のランチャー画面](docs/screenshot.avif)

## インストール

**動作環境**: Windows 10 / 11 (x64)

[**Releases**](https://github.com/nanogram-inc/nanogram-desktop-launcher/releases/latest) から用途に応じて選んでください。

| ファイル | 用途 |
|---|---|
| `*-setup.exe` | インストーラー（NSIS）。**通常はこちら** |
| `*.msi` | MSI 版（社内配布 / GPO 向け） |
| `*-portable.exe` | ポータブル版。EXE の隣に設定・DB・アイコンを作成するため、フォルダごとコピーすれば別PCへ移行できます |

> 未署名のため、初回起動時に SmartScreen の警告が表示される場合があります。
> 「詳細情報」→「実行」で起動できます。

### ソースからビルド

```bash
# 前提条件: Node.js 20+, Rust toolchain, Microsoft C++ Build Tools

npm install

# 開発モード
npx tauri dev

# リリースビルド
npx tauri build

# クラウド同期フォルダ内でビルドする場合は、target を同期外に逃がす
#（Dropbox 等が .rlib をロックしてビルドが失敗するため）
CARGO_TARGET_DIR="C:/tmp/launcher-target" npx tauri build
```

## クイックスタート

1. `nanogram desktop LauncheR.exe` を起動（システムトレイに常駐）
2. **Ctrl キーを素早く2回押す** とランチャーが表示
3. アプリ名を入力して検索、**Enter** で起動
4. 歯車アイコンまたは **Ctrl+,** で設定画面を開く

📖 **詳しい使い方・設定項目・トラブルシューティングは [ユーザーマニュアル (MANUAL.md)](MANUAL.md) を参照してください。**

## 技術スタック

| レイヤー | 技術 |
|----------|------|
| フロントエンド | React 18, TypeScript, Vite, Tailwind CSS |
| バックエンド | Rust, Tauri v2 |
| データベース | SQLite (rusqlite) |
| Windows API | windows crate (COM, Shell, ToolHelp, HiDpi) |
| アイコン解析 | pelite (PE解析), image crate |

## アーキテクチャ

```
┌─────────────────────────────────────────────┐
│  React Frontend (TypeScript)                │
│  ┌──────────┐ ┌──────────┐ ┌─────────────┐ │
│  │SearchBar │ │ResultList│ │ RecentStrip │ │
│  └──────────┘ └──────────┘ └─────────────┘ │
│                    │ invoke()                │
├────────────────────┼────────────────────────┤
│  Tauri IPC         │                        │
├────────────────────┼────────────────────────┤
│  Rust Backend      ▼                        │
│  ┌──────────┐ ┌──────────┐ ┌─────────────┐ │
│  │ Commands │ │ Scanner  │ │   Hotkey    │ │
│  └────┬─────┘ └──────────┘ │ (Polling)   │ │
│       │                    └─────────────┘ │
│  ┌────▼─────┐ ┌──────────┐                 │
│  │  SQLite  │ │Icon Cache│                 │
│  └──────────┘ └──────────┘                 │
└─────────────────────────────────────────────┘
```

## 更新履歴

各バージョンの変更内容とダウンロードは [Releases](https://github.com/nanogram-inc/nanogram-desktop-launcher/releases) を参照してください。
過去バージョンを含む履歴の一覧は [MANUAL.md の更新履歴](MANUAL.md#更新履歴) にも記載しています。

## コントリビュート

- バグ報告・機能提案は [Issues](https://github.com/nanogram-inc/nanogram-desktop-launcher/issues) からテンプレートに沿ってお願いします
- 脆弱性は Issue ではなく [Security Advisories](https://github.com/nanogram-inc/nanogram-desktop-launcher/security/advisories/new) から非公開でご報告ください（[SECURITY.md](SECURITY.md)）

## ライセンス

[MIT License](LICENSE) — Copyright (c) 2026 nanogram, llc.

## 作者

nanogram, llc.
