# Research: Git Integration

## R1: Git ステータス取得方法

**Decision**: `git status --porcelain=v1` の CLI 出力をパースする

**Rationale**:
- `git2` crate は libgit2 への C バインディングであり、ビルド依存性が増加し、`unsafe` コードを内部的に使用する
- CLI パースはシンプルで、git のバージョンアップにも追従しやすい
- porcelain v1 フォーマットは安定しており、パース容易:
  ```
  XY filename
  XY old_filename -> new_filename  (rename)
  ```
  - X = インデックス(staging)ステータス, Y = ワーキングツリーステータス
  - M=Modified, A=Added, D=Deleted, R=Renamed, ?=Untracked, U=Unmerged(Conflict)

**Alternatives considered**:
- `git2` crate: C バインディング依存、ビルド時間増加、`unsafe` 含む → 却下
- `gix` (gitoxide) crate: Pure Rust だが大きな依存、この用途にはオーバースペック → 却下
- `git status --porcelain=v2`: v1 より詳細だがパース複雑、必要以上の情報 → 却下

## R2: 非同期 Git ステータス取得パターン

**Decision**: `tokio::task::spawn_blocking` + mpsc チャネル (既存の children_rx/preview_rx パターンに準拠)

**Rationale**:
- 既存の `ChildrenLoadResult`、`PreviewLoadResult` と同じチャネルパターンを踏襲
- `spawn_blocking` 内で `Command::new("git").output()` を実行
- 結果は `GitStatusResult` として `git_tx` チャネルで送信
- イベントループの `process_watcher_events` 相当の位置で受信

**Alternatives considered**:
- `tokio::process::Command` (async): 追加の async runtime 依存なし（既に full features）だが、既存パターンとの一貫性を優先して spawn_blocking → 却下

## R3: ディレクトリ集約ロジック

**Decision**: `GitState` 内で `HashMap<PathBuf, GitFileStatus>` のファイルマッピングから、パスの prefix マッチでオンデマンド計算する

**Rationale**:
- 事前にディレクトリ→ステータスのマッピングを構築するとメモリと計算コストがかかる
- `visible_nodes` はレンダリング時にのみ呼ばれ、可視ノード数は画面サイズに制限される
- 各ディレクトリについて、その path を prefix とするエントリの max priority を計算する
- 1万ファイルでも可視ノード数は ~50 行程度なので、O(visible * total_files) は十分高速

**Alternatives considered**:
- 事前にディレクトリツリー全体のステータスを集約: メモリ効率が悪く、ツリー変更時の同期が複雑 → 却下
- ディレクトリの集約ステータスをキャッシュ: 最適化は必要になってから → YAGNI

## R4: Git ステータスのファイル名右側表示

**Decision**: `tree_view.rs` のファイル名描画後、行の右側にメタデータ領域として git ステータスインジケーターを表示

**Rationale**:
- ファイル名の右側にメタデータ領域を設ける（将来ファイルサイズ・最終変更日なども表示予定）
- セレクションマーカー（`●`/`◆`/`◇`）とは独立した位置のため、同時に表示可能
- Git ステータスは `M` (1文字) で右寄せまたはファイル名後に表示

## R5: ExternalCommand の git_status 条件

**Decision**: `ExternalCommand` に `git_status: Vec<String>` フィールドを追加し、`ExternalCmdProvider::can_handle` で Git ステータスを参照

**Rationale**:
- `can_handle` の既存シグネチャ `(&self, path: &Path, is_dir: bool)` に Git ステータス情報を渡す必要がある
- 方法: `PreviewProvider` trait の `can_handle` に git status パラメータを追加するか、provider 構築時に GitState の参照を渡す
- **選択**: `can_handle` シグネチャは変更せず、`ExternalCmdProvider` に `Arc<RwLock<Option<GitState>>>` を保持させ、`can_handle` 内で参照する
  - 理由: trait のシグネチャ変更は全 provider に影響するため影響範囲が大きい
  - `git_status` 条件がない command は従来通り git state を参照しない

**Alternatives considered**:
- `can_handle` シグネチャに `git_status: Option<&GitFileStatus>` を追加: 全 provider に影響、YAGNI → 却下
- 別の filtering 層を PreviewRegistry に追加: 複雑化 → 却下

## R6: `R` キーリフレッシュの実装

**Decision**: `TreeAction::Refresh` アクションを追加し、ハンドラでツリー再構築と Git ステータス再取得をトリガー

**Rationale**:
- 既存の `TreeAction` に `Refresh` バリアントを追加
- キーマップの `load_default_display` に `R` → `tree.refresh` を追加
- ハンドラで: (1) 現在のカーソルパスを保存 (2) ルートからツリーを再構築 (3) Git ステータス再取得 (4) カーソル位置を復元
