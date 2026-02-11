# Validation Checklist: TOML + Lua ユーザー設定

## Spec Completeness

- [x] すべてのユーザーストーリーに Acceptance Scenarios がある
- [x] 優先順位 (P1/P2/P3) が明示されている
- [x] Edge Cases が網羅されている
- [x] NEEDS CLARIFICATION マーカーがすべて解消されている
- [x] Success Criteria が測定可能である

## Functional Requirements

- [x] FR-001: 設定ファイル探索パス (XDG_CONFIG_HOME → ~/.config/trev/)
- [x] FR-002: TOML セクション定義 (sort, display, theme, keymap)
- [x] FR-003: Lua 設定ファイル (init.lua) の役割
- [x] FR-004: 優先順位 (CLI > Lua > TOML > デフォルト)
- [x] FR-005: 構文エラーの報告 (ファイル名・行番号)
- [x] FR-006: 未知キーの警告 (起動は妨げない)
- [x] FR-007: Lua タイムアウト (3秒)
- [x] FR-008: 設定可能項目の列挙
- [x] FR-009: Lua API スコープ (設定・キーバインドのみ、callback で拡張)

## Constitution 準拠

- [x] 安全な Rust: エラー処理は Result ベース、サイレント失敗なし
- [x] TDD: テスト可能な Acceptance Scenarios が定義されている
- [x] パフォーマンス設計: SC-003 で読み込み遅延 50ms 以下を規定
- [x] シンプルさ & YAGNI: ホットリロード等を明示的にスコープ外
- [x] インクリメンタルデリバリー: P1 → P2 → P3 の段階的実装が可能

## リスク・依存

- [ ] Lua ランタイムの crate 選定 (mlua vs rlua) が未決定 → plan フェーズで決定
- [ ] キーマッププリセット (vim/emacs) の具体的なバインド定義 → plan フェーズで決定
