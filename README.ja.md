# trev

🌐 日本語 | [English](README.md)

[![CI](https://github.com/nabekou29/trev/actions/workflows/ci.yml/badge.svg)](https://github.com/nabekou29/trev/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![GitHub Release](https://img.shields.io/github/v/release/nabekou29/trev)](https://github.com/nabekou29/trev/releases)

高速な TUI ファイルビューア。ツリービュー、シンタックスハイライト付きプレビュー、Neovim 連携に対応。

Rust 製で高速かつ安全に動作します。

![screenshot](demo/screenshot.png)

<details>
<summary>デモ</summary>

![demo](demo/demo.gif)

</details>

## 特徴

- **ツリービュー** — ディレクトリ構造を展開・折りたたみで閲覧
- **シンタックスハイライト付きプレビュー** — syntect による組み込みハイライト、外部ツール不要
- **画像プレビュー** — Sixel/Kitty プロトコルによるインライン画像表示
- **外部コマンドプレビュー** — カスタムコマンド（`jq`、`glow`、`dust` など）でファイルを表示
- **ファジー検索** — nucleo によるインタラクティブなファイル絞り込み
- **ファイル操作** — コピー、切り取り、貼り付け、リネーム、削除、Undo/Redo
- **ファイルシステム監視** — 外部からの変更を自動検知して反映
- **セッション保存** — 再起動後もツリーの状態を復元
- **Neovim 連携** — IPC サーバーモード（[trev.nvim](https://github.com/nabekou29/trev.nvim)）
- **柔軟な設定** — YAML 設定ファイル、キーバインドのカスタマイズ、JSON Schema 対応

## インストール

### GitHub Releases

```sh
curl -LsSf https://github.com/nabekou29/trev/releases/latest/download/trev-installer.sh | sh
```

### Homebrew

```sh
brew install nabekou29/tap/trev
```

### Cargo

```sh
cargo install --git https://github.com/nabekou29/trev
```

### mise

```sh
mise use -g github:nabekou29/trev
# or
mise use -g cargo:trev@https://github.com/nabekou29/trev
```

### 動作要件

- [Nerd Fonts](https://www.nerdfonts.com/) 対応のターミナル（ファイルアイコン表示用）
- Sixel または Kitty グラフィックスプロトコル（画像プレビュー用、任意）

## クイックスタート

```sh
trev              # カレントディレクトリを開く
trev ~/projects   # 指定ディレクトリを開く
trev -a           # 隠しファイルを表示
trev --no-preview # プレビューパネルを無効化
```

### CLI オプション

| オプション                 | 説明                                       |
| -------------------------- | ------------------------------------------ |
| `-a, --show-hidden`        | 隠しファイルを表示                         |
| `--show-ignored`           | gitignore 対象ファイルを表示               |
| `--no-preview`             | プレビューパネルを無効化                   |
| `--sort-order <ORDER>`     | smart, name, size, mtime, type, extension  |
| `--sort-direction <DIR>`   | asc, desc                                  |
| `--no-directories-first`   | ディレクトリを先頭にソートしない           |
| `--icons / --no-icons`     | ファイルアイコンの表示制御                 |
| `--ipc`                    | IPC サーバーを有効化                       |
| `--restore / --no-restore` | セッション状態の復元制御                   |
| `--no-git`                 | Git 連携を無効化                           |
| `--reveal <PATH>`          | 起動時に指定パスを表示                     |
| `--config <PATH>`          | デフォルトの代わりに指定設定ファイルを使用 |
| `--config-override <PATH>` | 追加設定をマージ（エディタプラグイン向け） |

### サブコマンド

```sh
trev ctl reveal <PATH>    # 実行中の trev インスタンスでファイルを表示
trev ctl ping             # インスタンスの生存確認
trev ctl quit             # 実行中の trev インスタンスを停止
trev socket-path          # 実行中の trev インスタンスのソケットを一覧表示
trev schema               # 設定の JSON Schema を出力
trev completions <SHELL>  # シェル補完を生成（bash, zsh, fish など）
```

## デフォルトキーバインド

| キー                | 操作                             |
| ------------------- | -------------------------------- |
| `j` / `k`           | 下 / 上に移動                    |
| `l` / `h`           | 展開 / 折りたたみ                |
| `Enter`             | ルート変更（ディレクトリ）       |
| `Backspace`         | 親ディレクトリへ移動             |
| `g` / `G`           | 先頭 / 末尾にジャンプ            |
| `Ctrl+d` / `Ctrl+u` | 半ページ下 / 上                  |
| `/`                 | ファジー検索                     |
| `J` / `K`           | プレビューを下 / 上にスクロール  |
| `Tab` / `Shift+Tab` | プレビュープロバイダを切替       |
| `P`                 | プレビューの表示切替             |
| `Space`             | マークの切替                     |
| `a` / `A`           | ファイル / ディレクトリの作成    |
| `r`                 | リネーム                         |
| `y` / `x` / `p`     | コピー / 切り取り / 貼り付け     |
| `d` / `D`           | 削除 / ゴミ箱に移動              |
| `u` / `Ctrl+r`      | Undo / Redo                      |
| `c`                 | コピーメニュー（パス、名前など） |
| `.` / `I`           | 隠しファイル / ignored の切替    |
| `S` / `s`           | ソートメニュー / 方向の切替      |
| `e`                 | エディタで開く                   |
| `?`                 | ヘルプを表示                     |
| `q`                 | 終了                             |

詳細: [docs/ja/keybindings.md](docs/ja/keybindings.md)

## 設定

設定ファイル: `~/.config/trev/config.yml`

エディタの自動補完を有効にするには、先頭に以下を追加してください:

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/nabekou29/trev/main/config.schema.json
```

詳細: [docs/ja/configuration.md](docs/ja/configuration.md)

### 設定例

```yaml
sort:
  order: smart
  direction: asc
  directories_first: true

display:
  show_hidden: false
  show_preview: true
  show_icons: true
  columns:
    - git_status
    - size
    - modified_at

preview:
  max_lines: 1000
  providers:
    - name: "Pretty JSON"
      extensions: [json]
      priority: high
      command: jq
      args: ["."]
```

詳細: [docs/ja/configuration.md](docs/ja/configuration.md)

## Neovim 連携

[trev.nvim](https://github.com/nabekou29/trev.nvim) でサイドパネルやフローティングウィンドウモードでの Neovim 連携が可能です。

```lua
-- lazy.nvim
{
  "nabekou29/trev.nvim",
  cmd = { "Trev" },
  keys = {
    { "<leader>e", function() require("trev").show() end, desc = "Show trev" },
    { "<leader>E", function() require("trev").show({ position = "float" }) end, desc = "Show trev (float)" },
  },
  opts = {
    width = 60,
  },
}
```

詳細は [trev.nvim](https://github.com/nabekou29/trev.nvim) を参照してください。

## ライセンス

MIT
