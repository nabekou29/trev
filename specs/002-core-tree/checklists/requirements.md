# Validation Checklist: コアツリーデータ構造

## Spec Completeness

- [x] すべてのユーザーストーリーに Acceptance Scenarios がある
- [x] 優先順位 (P1/P2) が明示されている
- [x] Edge Cases が網羅されている
- [x] NEEDS CLARIFICATION マーカーがゼロ
- [x] Success Criteria が測定可能である

## Functional Requirements

- [x] FR-001: TreeNode のフィールド定義
- [x] FR-002: ChildrenState の 3 状態
- [x] FR-003: ignore::WalkBuilder による .gitignore 尊重
- [x] FR-004: show_hidden / show_ignored オプション
- [x] FR-005: 起動時はルート直下 1 階層のみ
- [x] FR-006: 展開時の非同期 1 階層読み込み
- [x] FR-007: SortOrder / SortDirection / directories_first
- [x] FR-008: 再帰的ソート
- [x] FR-009: Tree 状態管理と visible_nodes()
- [x] FR-010: visible_nodes() は参照ベース
- [x] FR-011: カーソルバウンドチェック
- [x] FR-012: バックグラウンド全ツリー走査
- [x] FR-013: UI スレッド非ブロック
- [x] FR-014: 走査結果の遅延読み込みキャッシュ利用

## Constitution 準拠

- [x] 安全な Rust: unsafe 禁止、Result ベースのエラー処理
- [x] TDD: テスト可能な Acceptance Scenarios が定義されている
- [x] パフォーマンス設計: NFR-001/002 でパフォーマンス目標を明示
- [x] シンプルさ & YAGNI: Arena ベースツリー・ファイル監視を明示的にスコープ外
- [x] インクリメンタルデリバリー: P1 → P2 の段階的実装が可能

## リスク・依存

- [ ] バックグラウンド走査と遅延読み込みの統合タイミング → plan フェーズで設計
- [ ] SearchIndex の具体的なデータ構造 → plan フェーズで決定（Vec<PathBuf> vs HashMap 等）
