<!--
Sync Impact Report
- バージョン変更: 0.0.0 → 1.0.0
- バンプ理由: MAJOR - 初回 constitution 作成
- 追加された原則: I. 安全な Rust, II. テスト駆動開発, III. パフォーマンス設計, IV. シンプルさ & YAGNI, V. インクリメンタルデリバリー
- 追加されたセクション: 技術標準, 開発ワークフロー
- テンプレート更新状況:
  - .specify/templates/plan-template.md ✅ 更新不要 (Constitution Check セクションは汎用的)
  - .specify/templates/spec-template.md ✅ 更新不要
  - .specify/templates/tasks-template.md ✅ 更新不要
- フォローアップ TODO: なし
-->

# trev Constitution

## コア原則

### I. 安全な Rust

すべてのコードは安全な Rust で書き、厳格な lint を適用する:

- `unsafe` コードは**禁止** (`unsafe_code = "forbid"`)
- `unwrap()`, `expect()`, `panic!()`, 直接インデックスは**拒否**
- clippy の全グループ (`all`, `pedantic`, `nursery`, `cargo`) を `deny` レベルで適用
- エラー処理はアプリケーション層で `anyhow::Result`、ライブラリ型のエラーには `thiserror` を使用
- 失敗しうるすべての関数は `Result` を返す。サイレント失敗は許容しない

**根拠**: trev はユーザーのファイルシステムを操作するツール。安全性と信頼性は妥協できない。

### II. テスト駆動開発 (必須)

Kent Beck の Red-Green-Refactor サイクルをすべての機能開発で遵守する:

- テストは実装より先に書く
- テストは実装前に失敗すること (Red)
- 実装は最小限のコードでテストを通す (Green)
- テストが通る状態を維持しつつコードを整理する (Refactor)
- テストライブラリ: アサーションに `googletest`、パラメータ化テストに `rstest`、ファイルシステムテストに `tempfile`
- テストはソースファイル内に `#[cfg(test)]` モジュールとしてインラインで配置
- テストモジュールには `#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]` を付与

**根拠**: TDD はリグレッションを防ぎ、設計品質を担保する。プロトタイプではテストがあれば早期に発見できた問題があった。

### III. パフォーマンス設計

パフォーマンスに影響するパスは最初から効率的に設計する:

- 大きなデータ構造（特にツリーノード）の不要なクローンを避ける
- 読み取り専用アクセスには所有値 (`T`) より参照 (`&T`) を優先
- IO バウンドな処理（git status、ファイルプレビュー、IPC）はバックグラウンドタスク (`tokio`) で実行
- ディレクトリ内容は遅延読み込み（オンデマンドロード、起動時に全件読み込みしない）
- visible nodes の計算は 10k エントリのツリーで 16ms 以内に完了すること
- 起動は 1k ファイルのディレクトリで 500ms 以内に完了すること

**根拠**: TUI アプリケーションにユーザーは即座のフィードバックを期待する。プロトタイプにあったボトルネック（root clone、eager loading）を再発させない。

### IV. シンプルさ & YAGNI

すべてのコードはその存在を正当化できなければならない:

- まず動く最も単純な解決策から始め、測定に基づいてのみ最適化する
- 投機的な抽象化や早すぎる汎化をしない
- Arena ベースのツリー、画像プレビュー、Columns モードは将来の課題であり現在の要件ではない
- コメントは「なぜそうしないか」を説明する（コード自体が「何をするか」を語るべき）
- ドキュメント層: コード（How）、テスト（What）、コミット（Why）、コメント（Why not）

**根拠**: プロトタイプは複雑化した。製品版リビルドはシンプルさを維持する機会。

### V. インクリメンタルデリバリー

機能は小さく独立してテスト可能な単位で提供する:

- 各インクリメントはコンパイル・全テストパス・clippy パスの状態を維持する
- 機能はボトムアップで構築: データ構造 → ロジック → UI → 統合
- 各コミットは完全に動作する状態を表す
- メインブランチに "WIP" や壊れた中間状態を置かない

**根拠**: インクリメンタルデリバリーはリスクを低減し、継続的な検証を可能にする。

## 技術標準

- **言語**: Rust 2024 edition, nightly ツールチェイン
- **TUI**: ratatui 0.30, crossterm 0.29
- **非同期**: tokio (full features) - IPC とバックグラウンドタスク用
- **CLI**: clap 4 (derive マクロ)
- **シンタックスハイライト**: syntect 5, two-face 0.4
- **ファジー検索**: nucleo 0.5
- **ファイル走査**: ignore 0.4 (.gitignore 尊重)
- **シリアライゼーション**: serde + serde_json + toml
- **フォーマット**: rustfmt (`edition = "2024"`, `max_width = 100`, `group_imports = "StdExternalCrate"`, `imports_layout = "Vertical"`)
- **タスクランナー**: mise (build, test, lint, lint-fix, format, coverage)

## 開発ワークフロー

- すべての変更はコミット前に `mise run build`, `mise run test`, `mise run lint` を通すこと
- Conventional Commits を使用: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`
- コードレビューは厳格な clippy ルールで補助される（リンターが最初のレビュアー）
- フィーチャーブランチは `prototype` ブランチ（現在のメインブランチ）にマージ
- タスクコマンドとプロジェクトドキュメントは `.claude/CLAUDE.md` を参照

## ガバナンス

この constitution は trev プロジェクトの妥協できない基準を定義する。すべての実装判断はこれらの原則に準拠すること。

- 修正には変更内容と理由の明示的なドキュメントが必要
- 原則違反はコードコメントとコミットメッセージで正当化すること
- constitution のバージョンはセマンティックバージョニング (MAJOR.MINOR.PATCH) に従う

**Version**: 1.0.0 | **Ratified**: 2026-02-10 | **Last Amended**: 2026-02-10
