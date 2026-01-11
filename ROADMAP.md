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

- [ ] `Ctrl-d/Ctrl-u` - ページスクロール
- [ ] プレビュースクロール（J/K）
- [ ] 横スクロール（長い行のプレビュー）

### 検索/フィルタ

- [ ] `/` - FilterBox（インクリメンタルサーチ）
- [ ] fuzzy 検索（nucleo）

### 表示トグル

- [ ] `p` - Preview 表示トグル
- [ ] `.` - 隠しファイル表示切替
- [ ] `I` - ignored 表示切替

### 行表示改善

- [ ] flags 表示（symlink / exe 等）
- [ ] meta 表示（size / mtime）
- [ ] 幅に応じた省略処理

### 並び順

- [ ] 並び順の切り替え（`S` or `:sort`）
  - name / size / mtime / type / extension
- [ ] 昇順/降順トグル
- [ ] ディレクトリ優先オプション
- [ ] 設定ファイルでデフォルト指定

---

## Phase 2: Git 強化

- [ ] ディレクトリサマリ（`M2 ?5` 形式）
- [ ] `gd` - diff 表示トグル
- [ ] diff プレビュー（変更ファイルの差分表示）

---

## Phase 3: UI/UX

### オーバーレイ

- [ ] `?` - HelpOverlay
- [ ] `:` - CommandPalette
- [ ] Modal（確認ダイアログ）

### 選択操作

- [ ] `Space` - 選択トグル
- [ ] `a` - 全選択
- [ ] `A` - 全解除

### テーマ

- [ ] ターミナルテーマ検出（ダーク/ライト自動切替、yazi 方式: OSC 11）

---

## Phase 4: ファイル操作 ⭐

### 基本操作（重要）

- [ ] ファイルを開く（$EDITOR / open）
- [ ] **コピー** (`c` or `yy`)
- [ ] **移動/カット** (`x` or `dd`)
- [ ] **ペースト** (`p`)
- [ ] **削除** (`d` or `Delete`)
- [ ] **リネーム** (`r`)
- [ ] 確認モーダル（conflict 処理: rename/overwrite/skip）
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

### 単発モード（MVP）

- [ ] `--emit {lines|nul|json}`
- [ ] `--action {open|split|vsplit|tabedit}`
- [ ] `s/v/T` キーバインド

### 常駐モード

- [ ] daemon モード（IPC 待ち受け）
- [ ] `reveal(path)` による UI 更新
- [ ] viewer_id / workspace_key 管理
- [ ] viewer registry

### viewerctl

- [ ] path → workspace 解決
- [ ] workspace → default viewer_id 解決
- [ ] IPC 命令送信（reveal / reload / quit）
- [ ] viewer 不在時の起動

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

### 想定 crate

| 用途      | crate                         |
| --------- | ----------------------------- |
| UI        | ratatui, crossterm            |
| FS        | ignore, walkdir, globset      |
| Git       | 外部コマンド（MVP）→ git2/gix |
| 検索      | nucleo                        |
| IPC       | interprocess, tokio           |
| CLI       | clap                          |
| 設定      | serde, directories            |
| Clipboard | arboard                       |
| Plugin    | mlua                          |
