# Implementation Plan: Neovim IPC 連携

**Branch**: `013-neovim-ipc` | **Date**: 2026-02-16 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/013-neovim-ipc/spec.md`

## Summary

trev に JSON-RPC 2.0 over Unix Domain Socket の双方向通信基盤を実装し、Neovim プラグインとリアルタイムに連携する。TUI はサーバーとして UDS をリッスンし、Neovim プラグインがクライアントとして永続接続する。ファイルを開く、external command の通知、reveal、状態取得をすべて JSON-RPC で統一する。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: tokio (full, 既存), serde + serde_json (既存), dirs (既存), clap 4 (既存)
**Storage**: Unix Domain Socket (JSON-RPC 2.0)
**Testing**: cargo test, googletest 0.14, rstest 0.26, tempfile 3
**Target Platform**: Unix 系 (macOS, Linux) — Windows 非対応（Unix Domain Socket）
**Project Type**: single (Rust crate + Lua plugin)
**Performance Goals**: IPC コマンド応答 200ms 以内、reveal 500ms 以内、daemon 起動 1 秒以内
**Constraints**: 既存イベントループ（50ms poll）への統合、unsafe 禁止
**Scale/Scope**: 単一ワークスペースあたり 1 daemon

## Architecture

```
┌─────────────────────────────────────────────────┐
│                      TUI (Server)                │
│                                                  │
│  config.toml                                     │
│  ├─ [keys]              → 内部アクション         │
│  └─ [external_commands] → Neovim に通知          │
│                                                  │
│  JSON-RPC Server (<workspace_key>.sock)          │
│  ├─ reveal       : ファイル reveal               │
│  ├─ ping         : 疎通確認                      │
│  ├─ quit         : シャットダウン                │
│  └─ get_state    : 状態取得                      │
│                                                  │
│  Outgoing Notifications (→ Neovim)               │
│  ├─ open_file         : ファイルを開く            │
│  └─ external_command  : カスタムコマンド          │
└──────────────────────┬──────────────────────────┘
                       │ JSON-RPC 2.0 (bidirectional)
                       │ Unix Domain Socket
                       ▼
┌─────────────────────────────────────────────────┐
│                    Neovim (Client)               │
│                                                  │
│  trev.lua (plugin)                               │
│  ├─ on "open_file"    → vim.cmd("edit ...")      │
│  ├─ on "external_command"                        │
│  │   ├─ "find_files" → Telescope / fzf           │
│  │   ├─ "grep_project" → live_grep               │
│  │   └─ handlers は Neovim 側で自由に定義         │
│  │                                                │
│  ├─ autocmd BufEnter → JSON-RPC reveal 通知      │
│  └─ JSON-RPC request → get_state で状態取得      │
└─────────────────────────────────────────────────┘
```

### 通信フロー

| 方向 | 方式 | 用途 |
|------|------|------|
| TUI → Neovim | JSON-RPC notification | ファイルを開く (`open_file`) |
| TUI → Neovim | JSON-RPC notification | external_command 通知 |
| Neovim → TUI | JSON-RPC notification | BufEnter で reveal |
| Neovim → TUI | JSON-RPC request | 状態取得 (`get_state`) |
| CLI (`trev ctl`) → TUI | JSON-RPC request | reveal / ping / quit |

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. 安全な Rust | PASS | unsafe 不要。tokio UDS API はすべて safe。serde でシリアライゼーション。 |
| II. TDD | PASS | JSON-RPC プロトコル（シリアライズ/デシリアライズ）、workspace key 導出、reveal ロジックをテストファースト。 |
| III. パフォーマンス設計 | PASS | IPC は tokio 非同期、メインループの 50ms poll に IPC チャネルを追加統合。永続接続で接続コスト回避。 |
| IV. シンプルさ & YAGNI | PASS | JSON-RPC 2.0 は標準プロトコルで手動実装可能。`.cmd` ファイルハックを排除しシンプルに。 |
| V. インクリメンタルデリバリー | PASS | Phase 順: JSON-RPC 基盤 → reveal → daemon 統合 → ctl → external_commands → Neovim プラグイン |

## Project Structure

### Documentation (this feature)

```text
specs/013-neovim-ipc/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── ipc-protocol.md
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── ipc.rs               # IPC module root
├── ipc/
│   ├── types.rs         # JSON-RPC types, IpcCommand, EditorAction
│   ├── paths.rs         # workspace_key(), socket_path()
│   ├── server.rs        # UDS サーバー (永続接続管理、クライアント通知)
│   └── client.rs        # UDS クライアント (trev ctl 用)
├── app.rs               # メインループに IPC チャネル統合
├── app/
│   └── handler.rs       # IpcCommand ハンドリング追加
├── main.rs              # ctl サブコマンドのルーティング追加
├── config.rs            # [external_commands] セクション追加
└── state/
    └── tree.rs          # reveal_path() — 親を展開してカーソル移動

nvim-plugin/
├── lua/
│   └── trev/
│       └── init.lua     # Neovim プラグイン (setup, toggle, reveal, float, handlers)
└── plugin/
    └── trev.lua         # プラグインローダー
```

**Structure Decision**: 既存の単一プロジェクト構造を維持。IPC モジュールを `src/ipc/` サブモジュールに分割し、Neovim プラグインを `nvim-plugin/` に配置。

## Complexity Tracking

> 違反なし — 追加の正当化不要。
