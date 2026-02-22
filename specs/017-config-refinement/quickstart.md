# Quickstart: 設定体系の整備

**Feature**: 017-config-refinement
**Date**: 2026-02-22

## 概要

この機能は3つの柱で構成される:

1. **P1: メニューアクションの直接キーバインド** — ソート・コピーメニュー内の個別操作を直接キーに割り当て可能にする
2. **P2: ユーザー定義メニュー** — 設定ファイルで自由にメニューを定義し、キーバインドで呼び出せるようにする
3. **P3: 設定名称の見直し** — 設定項目名とアクション名を一貫性のある、わかりやすい命名に変更する

## 実装順序

```
P3 (設定名リネーム) → P1 (個別アクション追加) → P2 (ユーザー定義メニュー)
```

**理由**: P3の名前変更は他の変更と同時に行うとコンフリクトが増えるため先に実施する。P1はAction enumの拡張で、P2のメニュー機能の基盤となる。

## 主要な変更ファイル

| ファイル | 変更内容 |
|---|---|
| `src/action.rs` | `SortAction`, `CopyAction`, `FilterAction` サブenum追加、アクション名リネーム、`OpenMenu` バリアント追加、`all_action_names()` ヘルパー |
| `src/config.rs` | フィールドリネーム、`MenuDefinition` 構造体追加、`menus` セクション追加、`KeyBindingEntry` に `menu` フィールド追加、スキーマ自動生成 |
| `src/input.rs` | `MenuAction::Custom` バリアント追加 |
| `src/app/keymap.rs` | `menu` フィールドの解決処理追加 |
| `src/app/handler/tree.rs` | ソートメニューの呼び出しを `SortAction` 経由に変更、直接ソートアクションのハンドラ追加 |
| `src/app/handler/file_op.rs` | コピーメニューの呼び出しを `CopyAction` 経由に変更、直接コピーアクションのハンドラ追加 |
| `src/app/handler/input.rs` | `MenuAction::Custom` のディスパッチ処理追加 |
| `README.md` | キーバインド表・設定例の更新 |

## ビルド・テスト

```bash
mise run build    # コンパイル確認
mise run test     # ユニットテスト
mise run lint     # clippy チェック
```

## 設定例（変更後）

### 個別ソートアクションをキーに割り当てる

```yaml
keybindings:
  universal:
    bindings:
      - key: "<C-1>"
        action: "tree.sort.by_name"
      - key: "<C-2>"
        action: "tree.sort.by_size"
      - key: "<C-3>"
        action: "tree.sort.by_mtime"
```

### ユーザー定義メニューを作成する

```yaml
menus:
  quick_actions:
    title: "Quick Actions"
    items:
      - key: "e"
        label: "Edit in VS Code"
        run: "code {path}"
      - key: "o"
        label: "Open in Finder"
        run: "open -R {path}"
      - key: "n"
        label: "Open in Neovim"
        notify: "open_file"

keybindings:
  universal:
    bindings:
      - key: "m"
        menu: "quick_actions"
```
