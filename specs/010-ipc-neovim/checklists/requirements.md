# Validation Checklist: IPC / Neovim 統合

## Spec Completeness

- [x] すべてのユーザーストーリーに Acceptance Scenarios がある
- [x] 優先順位 (P1/P2/P3) が明示されている
- [x] Edge Cases が網羅されている
- [x] NEEDS CLARIFICATION マーカーがゼロ
- [x] Success Criteria が測定可能である

## Functional Requirements (Rust 側)

- [x] FR-001: --daemon フラグでの IPC サーバー起動
- [x] FR-002: ソケットパス (XDG_RUNTIME_DIR + フォールバック)
- [x] FR-003: JSON メッセージプロトコル (Request/Response/Notification)
- [x] FR-004: サポートメソッド (ping, reveal, quit)
- [x] FR-005: 通知 (open, selection_changed, closing)
- [x] FR-006: reveal の自動展開 + 遅延読み込み対応
- [x] FR-007: --action フラグと open 通知の連携
- [x] FR-008: --emit フラグ (フローティングモード)
- [x] FR-009: --emit-format (lines/nul/json)
- [x] FR-010: trev ctl サブコマンド
- [x] FR-011: 複数クライアント同時接続 + broadcast
- [x] FR-012: stale ソケットクリーンアップ

## Functional Requirements (Neovim プラグイン)

- [x] FR-NV-001: Lua プラグイン setup()
- [x] FR-NV-002: コマンド定義 (TrevOpen, TrevToggle, TrevClose, TrevReveal)
- [x] FR-NV-003: サイドパネルモード
- [x] FR-NV-004: フローティングモード
- [x] FR-NV-005: auto_reveal (BufEnter)
- [x] FR-NV-006: vim.uv.new_pipe() による双方向通信
- [x] FR-NV-007: open 通知の処理
- [x] FR-NV-008: 接続状態 API

## Constitution 準拠

- [x] 安全な Rust: 非同期 IPC、エラーハンドリング、stale ソケット処理
- [x] TDD: テスト可能な Request/Response シナリオ
- [x] パフォーマンス設計: NFR で遅延目標を明示
- [x] シンプルさ & YAGNI: プロトコルバージョニング・Windows 対応を明示的にスコープ外
- [x] インクリメンタルデリバリー: P1 (IPC基盤) → P2 (通知/サイドパネル) → P3 (フローティング/回復)

## リスク・依存

- [ ] vim.uv.new_pipe() の接続安定性 → Neovim 側の実装で検証
- [ ] toggleterm との統合方法 → plan フェーズで具体化
- [ ] workspace_key のハッシュ衝突 → 実用上は無視可能だが plan で確認
