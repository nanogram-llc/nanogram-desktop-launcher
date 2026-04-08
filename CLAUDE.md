# NANOGRAM Launcher

Tauri v2 + React + TypeScript + Rust によるクロスプラットフォーム対応デスクトップランチャー。

## 技術スタック

- **フロントエンド**: React 18 + TypeScript + Vite + Tailwind CSS
- **バックエンド**: Rust (Tauri v2)
- **DB**: SQLite (rusqlite bundled) — アプリ・フォルダ管理
- **設定**: `settings.json` (EXE隣に自動生成)
- **Windows固有**: `windows` crate (COM/Shell API, ToolHelp, HiDpi), `pelite` (PE解析)

## ビルド

```bash
# 開発
npx tauri dev

# リリースビルド (Dropbox上の場合はtargetを外に出す)
CARGO_TARGET_DIR="C:/tmp/lancher-target" npx tauri build

# TypeScriptチェックのみ
npx tsc --noEmit

# Rustビルドのみ
cd src-tauri && cargo build
```

- Dropbox同期がビルド中の `.rlib` をロックするため、`CARGO_TARGET_DIR` を同期外に指定すること
- ビルド前に `rm -rf dist` が必要な場合がある（Dropboxが `dist/assets` をロックする）
- EXEが起動中だとコピーできない。別名でコピーするか、終了後にコピーする

## アーキテクチャ

```
React (src/) → invoke() → Tauri IPC → Rust Commands (src-tauri/src/commands/) → SQLite/FS
```

- **2ウィンドウ構成**: ランチャー("main") と 設定("settings") は別ウィンドウ
  - App.tsx が `getCurrentWindow().label` で判定して分岐表示
  - 設定ウィンドウは `WebviewWindow` でオンデマンド作成、`decorations: true`, `alwaysOnTop: true`
  - ランチャーは `decorations: false`, `transparent: true`, `alwaysOnTop: true`
- **ウィンドウサイズ**: 設定の `pinned_columns` / `pinned_rows` から動的計算
  - 幅: `cols * 66 + 32` (グリッド幅 + padding + border)
  - 高さ: `totalRows * 70 + sectionCount * 24 + 108`
  - Rust側 `resize_window` コマンドで `set_size(LogicalSize)` 適用（`resizable: false` でも動作）
- **リアルタイム設定同期**: GeneralSettings → `emit("settings-changed")` → LauncherWindow が `listen` で受信
- **ホットキー**: Ctrl×2 → `GetAsyncKeyState` ポーリング（10ms間隔）
- **blur非表示**: ランチャー画面ではフォーカス喪失で自動非表示、設定ウィンドウ表示中は無効化
- **アイコン**: EXEから抽出 → PNG → disk cache → base64でIPC → メモリキャッシュ
- **DPI対応配置**: `move_and_show` で `GetDpiForMonitor` を使いターゲットモニターのDPIスケールで物理座標計算
- **重複起動防止**: `launch_app` 時に3段階チェック
  1. `recent_launches` (AppState) で5秒以内の同一exe起動をブロック（プロセス起動ラグ対策）
  2. `CreateToolhelp32Snapshot` で同名プロセスの実行中チェック、既存なら `EnumWindows` + `SetForegroundWindow` で前面復帰
  3. `.lnk` ショートカットは `resolve_single_lnk` でリンク先exeを解決してからプロセス名照合

## ディレクトリ構成

```
src/                          # React フロントエンド
  App.tsx                     # ウィンドウラベルで launcher/settings を分岐
  components/launcher/        # ランチャーUI (SearchBar, ResultsList, RecentStrip)
  components/settings/        # 設定UI (FolderManager, AppTable, GeneralSettings)
  lib/tauri.ts                # 全Rustコマンドのinvoke()ラッパー
  lib/types.ts                # 共有型定義

src-tauri/src/                # Rust バックエンド
  lib.rs                      # Tauriセットアップ (プラグイン, vibrancy, blur, tray, 初期サイズ)
  hotkey.rs                   # Ctrl×2検出(GetAsyncKeyState) + DPI対応モニター配置
  tray.rs                     # システムトレイメニュー
  commands/                   # Tauriコマンド (apps, folders, scan, icons, settings, window)
    window.rs                 # set_blur_hide, resize_window, get_exe_dir, calc_window_size
    apps.rs                   # launch_app(重複防止), is_process_running, bring_process_to_front
  db/                         # SQLite (init, models, queries)
  scanner/                    # プラットフォーム別スキャナー (windows/macos/linux)
  icons/                      # プラットフォーム別アイコン抽出
```

## 主要コマンド (Rust → フロントエンド)

| コマンド | 用途 |
|---|---|
| `get_apps(query?, limit)` | 検索 or 最近/頻出アプリ取得 (ランキング付き、item_type='app'のみ) |
| `launch_app(id)` | アプリ起動 + 重複チェック + 使用回数更新 |
| `add_folder(path)` | フォルダ追加 + 初回スキャン → `{folder, app_count}` |
| `rescan_folder(id, mode)` | `"add_new"` or `"add_all_except_excluded"` |
| `get_startmenu_paths()` | Windowsスタートメニューパス取得 |
| `set_blur_hide(bool)` | フォーカス喪失時の非表示制御 |
| `resize_window(cols, totalRows, sectionCount)` | ウィンドウサイズ動的変更 |

## グリッドレイアウト

全セクション（最近使った・よく使う・固定）が同一CSSグリッドを使用:
- `gridTemplateColumns: repeat(cols, 64px)`, `gap: 2px` (gap-0.5)
- `w-fit mx-auto` でセンタリング
- 最近/よく使う: 1行、表示数は `pinnedColumns` にキャップ
- 固定: `pinnedColumns × pinnedRows` のグリッド、空スロットはドロップ可能
- 固定エリアのD&D: mousedown/mousemove/mouseup カスタム実装（WebView2はHTML5 D&D非対応）

## 設定項目 (Settings)

| フィールド | 型 | デフォルト | 説明 |
|---|---|---|---|
| `pinned_columns` | number | 9 | 固定エリア列数 (5-15) |
| `pinned_rows` | number | 2 | 固定エリア行数 (1-5) |
| `sections` | SectionConfig[] | 3セクション全可視 | 表示セクションの順序と可視性 |
| `pinned_items` | string[] | [] | 固定アプリIDの配列（空文字=""で位置保持） |

## 検索ランキング

```
score = 名前完全一致(100) / 前方一致(80) / 部分一致(50) / 部分列(30)
      + タグ一致(40)
      + ln(launch_count + 1) * 10
      + 20 * exp(-hours_ago / 168)
```

## 検索履歴

- `localStorage` に `nanogram-search-history` キーで保存（最大100件）
- アプリ起動時にクエリを履歴に追加（重複は先頭に移動）
- `Ctrl+↑/↓` で履歴をナビゲート（`↑/↓` は検索結果の候補選択に使用）

## データ保存先

- **DB**: `{app_data_dir}/launcher.db` (アプリ・フォルダ情報)
- **アイコンキャッシュ**: `{app_data_dir}/icons/*.png`
- **設定**: `{exe_dir}/settings.json`
- **検索履歴**: `localStorage` (WebView2内)

## アイテムタイプ (item_type)

DBの `apps` テーブルは3種類のアイテムを格納:
- `"app"` — 通常のアプリ (.exe/.lnk)。`get_all_apps` / `get_excluded_apps` で取得
- `"file"` — ドロップ登録されたファイル。`get_items_by_type("file")` で取得
- `"folder"` — ドロップ登録されたフォルダ。`get_items_by_type("folder")` で取得

**重要**: `get_all_apps` / `get_excluded_apps` のSQLには `item_type = 'app' OR item_type IS NULL` フィルターが必要。これがないとファイル/フォルダがアプリ一覧に混入する。

## 楽観的更新 (Optimistic Update)

`handleLaunch` でアプリ起動後、DBからの再取得を待たずにフロントのステートを即時更新:
- `recentApps`: 起動アプリを先頭に挿入
- `frequentApps`: `launch_count` をインクリメントして再ソート
- `pinnedApps`: 該当アプリの統計を更新

これにより、次回ランチャー表示時に最新の「最近使った」「よく使う」が即座に反映される。

## 注意点

- `darkMode: "class"` (Tailwind) — `<html>` に `dark` クラスをJS側で付け外し
- alert() は使わない — アプリ内トースト通知を使用
- 設定ウィンドウ表示中は `blur_hide` を無効化、閉じたら再有効化
- Ctrl×2検出は `tauri-plugin-global-shortcut` では不可 → `GetAsyncKeyState` ポーリング使用
- WebView2 は HTML5 Drag & Drop API 非対応 → mouseイベントでカスタム実装が必要
- WebView2 は `WH_KEYBOARD_LL` フックと干渉 → `GetAsyncKeyState` ポーリングを使う
- `resizable: false` でも Rust側 `window.set_size()` は動作する（フロントエンドJSからは不可）
- マルチモニター環境: `GetDpiForMonitor` でターゲットモニターのDPIを取得し物理座標を計算
- `AppState.window_logical_size` に論理サイズを保持し、モニター間移動時のDPI変換に使用
- capabilities (`src-tauri/capabilities/default.json`) に `allow-create`, `allow-set-size`, `allow-emit`, `allow-listen` 等の権限が必要
