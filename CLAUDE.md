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
- **ホットキー**: Ctrl×2/×3 → `GetAsyncKeyState` ポーリング（10ms間隔）
  - 「純タップ判定」: 押下中に他キーが押されたらタップと数えない（`Ctrl+C`→`Ctrl+V` の誤検知対策）
  - 押下開始時に `snapshot_keys` でベースラインを取り、差分のみを汚染とみなす
- **バックアップ**: `settings.json` / `launcher.db` / `icons/` を zip 化。手動 `backups/manual/` と自動 `backups/auto/`（7世代）を別管理
- **テーマ**: CSS変数 (`--accent` / `--surface-alpha` / `--text-main`) を `applyTheme()` が注入。背景画像は `theme/` にコピーして base64 で読み込み
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
  components/launcher/        # ランチャーUI (SearchBar, ResultsList, RecentStrip, ResultItem)
  components/settings/        # 設定UI (FolderManager, AppTable, GeneralSettings, CommandManager)
  lib/tauri.ts                # 全Rustコマンドのinvoke()ラッパー
  lib/types.ts                # 共有型定義

src-tauri/src/                # Rust バックエンド
  lib.rs                      # Tauriセットアップ (プラグイン, vibrancy, blur, tray, 初期サイズ, autostart同期)
  hotkey.rs                   # Ctrl×2/×3検出(GetAsyncKeyState) + 純タップ判定 + DPI対応配置
  tray.rs                     # システムトレイメニュー
  commands/                   # Tauriコマンド (apps, backup, folders, scan, icons, settings, window)
    window.rs                 # set_blur_hide, resize_window, get_exe_dir, calc_window_size,
                              #   set_vibrancy_theme, set_background_image, get_background_image_data
    backup.rs                 # create_backup, list_backups, restore_backup, delete_backup,
                              #   auto_backup_if_due (起動時に1日1回・7世代ローテーション)
    apps.rs                   # launch_app(重複防止), launch_app_as_admin, launch_command,
                              #   register_command, update_command, get_command_data
    settings.rs               # get_settings, update_settings, apply_autostart
  db/                         # SQLite (init, models, queries)
  scanner/                    # プラットフォーム別スキャナー (windows/macos/linux)
  icons/                      # プラットフォーム別アイコン抽出
```

## 主要コマンド (Rust → フロントエンド)

| コマンド | 用途 |
|---|---|
| `get_apps(query?, limit)` | 検索 or 最近/頻出アプリ取得 (ランキング付き、app/command を対象) |
| `launch_app(id)` | アプリ起動 + 重複チェック + 使用回数更新 |
| `launch_app_as_admin(id)` | `ShellExecuteW(runas)` でUAC昇格起動 (.lnk は解決済みexeに適用) |
| `launch_command(id)` | コマンドアイテム起動 (一時.batファイル経由、admin時はrunas) |
| `register_command(name, cmd, admin, keep_open)` | コマンドアイテム登録 |
| `update_command(id, cmd, admin, keep_open)` | コマンドアイテム更新 |
| `get_command_data(id)` | コマンドデータ取得 (編集用) |
| `reveal_in_explorer(id)` | 実行ファイルの場所をエクスプローラーで開く (.lnk は解決) |
| `create_backup()` / `restore_backup(path)` | バックアップ作成 / 復元 (復元前に自動退避、退避モード中は拒否) |
| `list_backups()` / `delete_backup(path)` | バックアップ一覧 / 削除 |
| `set_background_image(source)` | 背景画像を `theme/` にコピーして保存先パスを返す |
| `get_background_image_data(path)` | 背景画像を base64 データURI で取得 |
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
| `hotkey_taps` | number | 2 | 発火に必要な連続タップ数 (2-3) |
| `hotkey_interval_ms` | number | 400 | 連続タップとみなす間隔 (200-800) |
| `window_position` | string | "center" | "center" / "top-left" / "top-right" / "bottom-left" / "bottom-right" |
| `window_margin` | number | 40 | 四隅指定時の画面端からの余白 (論理px) |
| `auto_backup` | boolean | true | 起動時に1日1回バックアップ |
| `accent_color` | string | "blue" | プリセット5色 or "custom" |
| `accent_custom` | string | "#3b82f6" | `accent_color === "custom"` 時に使用 |
| `background_image` | string | "" | 背景画像パス（空=未使用） |
| `background_dim` | number | 40 | 背景画像に重ねる黒の濃度 (0-100) |
| `background_scale` | number | 100 | 背景画像の拡大率 (100-300) |
| `window_opacity` | number | 100 | ウィンドウ不透明度 (50-100)。100=不透明 |
| `text_color` | string | "auto" | "auto" / "light" / "dark" / "custom" |
| `text_custom` | string | "#ffffff" | `text_color === "custom"` 時に使用 |

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

## データ保存先 (ポータブル構成)

全データをEXE隣に置くことで、フォルダごとコピーするだけで別環境に移行可能:

- **DB**: `{exe_dir}/launcher.db`
- **アイコンキャッシュ**: `{exe_dir}/icons/*.png`
- **設定**: `{exe_dir}/settings.json`
- **検索履歴**: `localStorage` (WebView2内)

初回起動時に旧 `%APPDATA%\com.launcher.app\` からDBとアイコンを自動マイグレーション。

## アイテムタイプ (item_type)

DBの `apps` テーブルは4種類のアイテムを格納:
- `"app"` — 通常のアプリ (.exe/.lnk)。`get_all_apps` / `get_excluded_apps` で取得
- `"file"` — ドロップ登録されたファイル。`get_items_by_type("file")` で取得
- `"folder"` — ドロップ登録されたフォルダ。`get_items_by_type("folder")` で取得
- `"command"` — 登録済みシェルコマンド。`exe_path` に `cmd://<uuid>|<json>` 形式でデータを格納

**重要**: `get_all_apps` のSQLフィルターは `item_type = 'app' OR item_type = 'command' OR item_type IS NULL`。`"file"` / `"folder"` は除外、`"command"` は含める。

## コマンドアイテム

設定の「コマンド」タブから登録するシェルスクリプトアイテム:
- `exe_path` = `cmd://<uuid>|{"cmd":"...","admin":bool,"keep_open":bool}` (UNIQUE制約をuuidで回避)
- 起動時: 一時 `.bat` ファイル (`%TEMP%\nanogram_cmd_<uuid>.bat`) に書き出して実行
  - `keep_open: true` → bat末尾に `pause` を追加してウィンドウを保持
  - `admin: true` → `ShellExecuteW(runas, <bat>)` でUAC昇格
  - 通常 → `cmd /C start "" cmd /C <bat>` でデタッチ起動
- 検索対象に含まれる (ランチャー検索で名前が一致すると表示される)
- `ResultItem.tsx` でアイコンをターミナルSVGで表示、`displayPath` で最初の行を抜粋表示

## 楽観的更新 (Optimistic Update)

`handleLaunch` でアプリ起動後、DBからの再取得を待たずにフロントのステートを即時更新:
- `recentApps`: 起動アプリを先頭に挿入
- `frequentApps`: `launch_count` をインクリメントして再ソート
- `pinnedApps`: 該当アプリの統計を更新

これにより、次回ランチャー表示時に最新の「最近使った」「よく使う」が即座に反映される。

`FolderManager` のフォルダ追加も楽観的更新: `add_folder` が返す `result.folder` を即時 `setFolders` に追加。Tauriネイティブダイアログ後のWebView2非同期コンテキストでは `loadFolders()` の `setFolders` がre-renderをトリガーしないことがあるため。

## 右クリックメニュー

- **通常右クリック**: タグ編集 / 固定 / 除外 (+ フォルダアイテムはエクスプローラー/ターミナルで開く)
- **Ctrl+右クリック**: 上記に加え「🛡 管理者として実行」を最上部に表示 (app/file のみ)
  - `launch_app_as_admin` → `ShellExecuteW(runas, exe_path)`
  - `.lnk` はリンク先exeを解決してから `runas` 適用
  - 対象: 検索結果 (`ResultItem`) / 固定・最近・よく使う (`RecentStrip`) 全セクション
- メニュー位置は `useLayoutEffect` でペイント前にビューポートはみ出しを補正 (右端/下端で反転)

## 自動起動 (Autostart)

`tauri-plugin-autostart` を使用。設定変更時と起動時の両方で同期:
- 設定変更 (`update_settings`) → `apply_autostart(app, bool)` → `mgr.enable()` / `mgr.disable()`
- 起動時にも `apply_autostart` を呼び、EXEパスが変わっていてもレジストリエントリを更新
  - Dropbox経由で別PCにコピーした際のパス変更に対応

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
- ネイティブダイアログ後のWebView2非同期コンテキストでは `setFolders` 等のstate更新がre-renderをトリガーしないことがある → 楽観的更新で回避
- **`GetAsyncKeyState` の全域スキャンは不可** — 日本語IME環境では VK `0xF4` が常時「押下中」を返す。実キーの許可リスト (`SCAN_RANGES`) のみ走査すること
- **ランチャーのルート要素を完全透明にしない** — `transparent: true` のため alpha 0 だとヒットテストが素通りし、クリック・ホバーが効かなくなる（`--surface-alpha` は最低でも 0.46）
- **重ね順に注意** — 設定/閉じるボタンは `absolute z-20`、背景画像レイヤーは `-z-10`。コンテンツに `z-10` を付けるとボタンを覆ってクリック不能になる
- `move_and_show` の中央寄せは `GetWindowRect` の実測値を使う。論理サイズ×`GetDpiForMonitor` の推定は `devicePixelRatio` と食い違い、位置がずれる
