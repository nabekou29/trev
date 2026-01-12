# trev 機能ロードマップ

## 実装済み

- [x] ツリービュー（展開/折り畳み）
- [x] プレビュー（シンタックスハイライト with syntect/two-face）
- [x] Git ステータス表示（色分け）
- [x] ファイルアイコン（devicons/Nerd Fonts）
- [x] Vim 風キーバインド (j/k/h/l/gg/G/Enter/q)

---

## Phase 1: 基本機能強化

### ナビゲーション

- [x] `Ctrl-d/Ctrl-u` - ページスクロール
- [x] プレビュースクロール（J/K）
- [x] 横スクロール（長い行のプレビュー）

### 検索/フィルタ

- [x] `/` - FilterBox（インクリメンタルサーチ）
- [x] fuzzy 検索（nucleo）

### 表示トグル

- [x] `p` - Preview 表示トグル
- [x] `.` - 隠しファイル表示切替
- [x] `I` - ignored 表示切替

### 行表示改善

- [x] flags 表示（symlink / exe 等）
- [x] meta 表示（size / mtime）
- [x] 幅に応じた省略処理

### 並び順

- [x] 並び順の切り替え（`S` でサイクル）
  - name / size / mtime / type / extension
- [x] 昇順/降順トグル（`s`）
- [x] ディレクトリ優先オプション（デフォルト有効）
- [x] 設定ファイルでデフォルト指定

---

## Phase 2: Git 強化

- [ ] ~~ディレクトリサマリ（`M2 ?5` 形式）~~ スキップ
- [ ] `gd` - diff 表示トグル
- [ ] diff プレビュー（変更ファイルの差分表示）

---

## Phase 3: UI/UX

### オーバーレイ

- [ ] `?` - HelpOverlay
- [ ] `:` - CommandPalette
- [x] Modal（確認ダイアログ）

### レスポンシブレイアウト

- [x] 狭い画面で上下分割（幅 < 100）
- [x] 狭い TreeView でメタ情報非表示（幅 < 60）
- [x] 狭い画面でプレビューをデフォルト非表示

### 選択操作

- [x] `m` - マークトグル（複数選択）
- [x] `M` - マーク全解除
- [x] マークしたファイルのまとめて削除・コピー・カット・ペースト
- [x] コピー/カット中のファイル表示

### テーマ

- [ ] ターミナルテーマ検出（ダーク/ライト自動切替、yazi 方式: OSC 11）

---

## Phase 4: ファイル操作 ⭐

### 基本操作（重要）

- [x] ファイルを開く（$EDITOR / open）- `o` キー
- [x] **コピー** (`yy`)
- [x] **移動/カット** (`x`)
- [x] **ペースト** (`P`)
- [x] **削除** (`d` or `Delete`) - 確認付き
- [x] **リネーム** (`r`)
- [x] 確認モーダル（conflict 処理: rename/overwrite/skip）
- [x] **ファイル追加** (`a`)
- [x] **ディレクトリ追加** (`A`)
- [ ] Undo 機能（ゴミ箱 or 履歴）

### Tree コピー

- [ ] `Y` - Visible Tree Export（tree コマンド風）
- [ ] `:copy tree [--git|--ascii|--relative|--absolute]`

### ドラッグ & ドロップ

- [ ] ファイル追加（パス文字列入力として処理）
- [ ] copy/move 選択モーダル

### クリップボード

- [ ] パスコピー（arboard）

---

## Phase 5: Neovim 連携

### 単発モード（Picker）

- [x] `--emit` - 終了時に選択パスを出力
- [x] `--emit-format {lines|nul|json}` - 出力フォーマット
- [x] `--action {edit|split|vsplit|tabedit}` - 終了時アクション種別
- [x] `--reveal <path>` - 起動時に指定パスを選択

### 常駐モード（Daemon）

- [x] `--daemon` - IPC サーバー有効化
- [x] Unix Domain Socket による IPC
- [x] workspace_key 管理（Git root or directory name）
- [x] コマンドファイルによるエディタへの通知

### trev ctl サブコマンド

- [x] `trev ctl reveal <path>` - パスを選択
- [x] `trev ctl ping` - 疎通確認
- [x] `trev ctl quit` - 終了要求

### Neovim プラグイン (nvim-plugin/)

- [x] toggleterm.nvim ベースのサイドパネル
- [x] フローティングウィンドウ Picker
- [x] `TrevOpen`, `TrevToggle`, `TrevClose` コマンド
- [x] `reveal` 引数でカレントファイルを選択
- [x] `auto_reveal` - バッファ切替時に自動 reveal
- [x] コマンドファイル監視（edit/split/vsplit/tabedit）

---

## Phase 6: 表示モード

### Columns モード（Finder 風）

- [ ] パス階層ごとのカラム構成
- [ ] `t` - Tree ↔ Columns 切替
- [ ] current_path の維持

---

## Phase 7: 設定/カスタマイズ

### 設定ファイル

- [ ] 設定ファイル読み込み（`$XDG_CONFIG_HOME/trev/config.toml`）
  - directories クレートで XDG Base Directory 準拠
- [ ] テーマ設定（色、スタイル）
- [ ] デフォルト並び順
- [ ] 表示オプション（アイコン、meta、hidden 等）

### キーマップ

- [ ] キーマップのカスタマイズ
- [ ] Vim / Emacs / Custom プリセット
- [ ] 設定ファイルでキーバインド定義
- [ ] `:map` / `:unmap` コマンド

---

## Phase 8: 将来拡張

- [ ] 画像プレビュー（端末依存、フォールバック）
- [ ] Lua プラグイン（mlua）
- [ ] プレビューキャッシュ
- [ ] ブックマーク

---

## 調査済み

### シンタックスハイライト

- `syntect` + `two-face` を採用（bat プロジェクトの拡張シンタックス）
- TOML, TypeScript など追加言語をサポート

### ターミナルテーマ検出

- yazi 方式: OSC 11 (`\x1b]11;?\x07`) でターミナル背景色をクエリ
- BT.2100 輝度計算で明暗判定
- 代替: `terminal-light` クレート

### ファイルアイコン

- `devicons` クレートを採用
- Nerd Fonts 必須

### 使用 crate

| 用途      | crate                          | 状態         |
| --------- | ------------------------------ | ------------ |
| UI        | ratatui, crossterm             | ✅ 使用中    |
| FS        | ignore, walkdir                | ✅ 使用中    |
| Git       | 外部コマンド                   | ✅ 使用中    |
| 検索      | nucleo                         | ✅ 使用中    |
| IPC       | tokio (Unix Socket)            | ✅ 使用中    |
| CLI       | clap                           | ✅ 使用中    |
| 設定      | serde, directories             | ✅ 使用中    |
| Clipboard | arboard                        | 未実装       |
| Plugin    | mlua                           | 未実装       |
