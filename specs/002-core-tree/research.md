# Research: コアツリーデータ構造

## R-001: ignore crate の WalkBuilder API

**Decision**: `ignore::WalkBuilder` を depth=1 で使用し、ルート直下のみ初期ビルド。展開時も depth=1 で子ディレクトリを読み込む。

**Rationale**: ignore crate は `.gitignore`, `.ignore`, グローバル gitignore を自動的に尊重する。`max_depth(1)` で遅延読み込みに対応可能。`WalkBuilder::new(path).max_depth(Some(1)).hidden(!show_hidden).build()` のパターン。

**Alternatives considered**:
- `std::fs::read_dir`: `.gitignore` を手動で処理する必要がある
- `walkdir` crate: `.gitignore` サポートなし
- `ignore::Walk` のフル走査 + フィルタ: 遅延読み込みに不適

## R-002: VisibleNode の lifetime 問題

**Decision**: `visible_nodes()` は `Vec<VisibleNode<'_>>` を返すメソッドとし、呼び出しごとに再構築する。キャッシュは dirty フラグで invalidation。

**Rationale**: self-referential struct (`visible_nodes` が `root` を借用) は Rust で安全に実現困難。都度計算は 10k エントリで 16ms 以内に収まる見込み（単純な DFS walk）。プロトタイプでの root clone の方がコスト大。

**Alternatives considered**:
- インデックスベース (`Vec<(usize, usize)>`): ノードへのパスを毎回辿る必要がありアクセスコスト増
- `unsafe` ポインタ: Constitution で禁止
- `ouroboros` / `self_cell` crate: 依存追加に見合うほど複雑でない

## R-003: バックグラウンド走査と遅延読み込みの統合

**Decision**: バックグラウンド走査は `ignore::WalkBuilder` のフル走査をバックグラウンドで実行し、結果を `SearchIndex` (パス + メタデータのリスト) として保持。遅延読み込みのキャッシュとしても利用可能。

**Rationale**: 走査結果は TreeNode 構造とは独立したフラットリスト。ディレクトリ展開時に SearchIndex に該当パスの子があれば IO を省略し、なければ通常の WalkBuilder で読み込む。

**Alternatives considered**:
- バックグラウンドで TreeNode 構造も構築: 遅延読み込みのツリーとの同期が複雑
- 走査なし（検索は読み込み済みのみ）: 未展開ディレクトリが検索できない

## R-004: tokio::task::spawn_blocking の使用

**Decision**: `ignore::Walk` は同期 API のため、`tokio::task::spawn_blocking` でラップする。遅延読み込み (1 ディレクトリの depth=1 走査) も同様。

**Rationale**: `ignore::Walk` のイテレータは内部で同期 IO を行う。tokio ランタイムのワーカースレッドをブロックしないよう `spawn_blocking` で別スレッドに逃がす。

**Alternatives considered**:
- `tokio::fs` を使った非同期実装: `ignore` crate の `.gitignore` 処理を再実装する必要がある
- 同期実行: UI がブロックされる

## R-005: ソートの case-insensitive 比較

**Decision**: 名前ソートは `unicase::UniCase` または `str::to_lowercase()` ベースの case-insensitive 比較。初期実装は `to_lowercase()` で開始し、パフォーマンス問題があれば `unicase` に切り替え。

**Rationale**: ファイルシステムブラウザとしてユーザーが期待する自然な順序。Unicode 対応を考慮すると `unicase` がベストだが、YAGNI 原則で簡易実装から始める。

**Alternatives considered**:
- 完全な case-sensitive ソート: ユーザー期待と異なる (`A` → `B` → `a` ではなく `A` → `a` → `B`)
- `natord` crate (自然順ソート): 将来追加の可能性あるが v1.0 ではスコープ外
