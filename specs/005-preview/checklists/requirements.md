# Validation Checklist: ファイルプレビュー

## Spec Completeness

- [x] すべてのユーザーストーリーに Acceptance Scenarios がある
- [x] 優先順位 (P1/P2/P3) が明示されている
- [x] Edge Cases が網羅されている
- [x] NEEDS CLARIFICATION マーカーがゼロ
- [x] Success Criteria が測定可能である

## Functional Requirements

- [x] FR-001: PreviewContent 列挙型のバリアント定義
- [x] FR-002: syntect + two-face によるシンタックスハイライト
- [x] FR-003: ハイライトテーマの設定変更
- [x] FR-004: 非同期プレビュー読み込み (tokio)
- [x] FR-005: CancellationToken によるキャンセル
- [x] FR-006: 大きなファイルの先頭のみ読み込み
- [x] FR-007: バイナリ判定
- [x] FR-008: 縦横スクロール
- [x] FR-009: ファイル切替時のスクロールリセット
- [x] FR-010: LRU キャッシュ
- [x] FR-011: PreviewProvider トレイト (can_handle, load, name, priority)
- [x] FR-012: PreviewRegistry によるプロバイダー収集・優先度選択
- [x] FR-013: プレビューモード切替（サイクル）
- [x] FR-014: ratatui-image による画像プレビュー
- [x] FR-015: 画像対応フォーマット・端末プロトコル・フォールバック
- [x] FR-016: 外部コマンドプロバイダーの can_handle 判定
- [x] FR-017: ansi-to-tui による ANSI カラー変換
- [x] FR-018: 外部コマンドタイムアウト
- [x] FR-019: ステータスバーにプロバイダー名表示

## Constitution 準拠

- [x] 安全な Rust: 非同期処理、エラーハンドリング
- [x] TDD: テスト可能な Acceptance Scenarios
- [x] パフォーマンス設計: NFR で応答時間・キャッシュを規定
- [x] シンプルさ & YAGNI: 文字エンコーディング対応を明示的にスコープ外
- [x] インクリメンタルデリバリー: P1 (テキスト) → P2 (バイナリ/ディレクトリ/切替) → P3 (画像/外部コマンド)

## リスク・依存

- [ ] ratatui-image の端末プロトコル自動検出の信頼性 → plan フェーズで検証
- [ ] resvg による SVG ラスタライズのパフォーマンス → plan フェーズでベンチマーク
- [ ] 001-user-config との統合（外部コマンド設定フォーマット）→ 001 の plan 完了後に具体化
