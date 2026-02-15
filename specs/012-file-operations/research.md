# Research: 012-file-operations

**Date**: 2026-02-15

## 1. notify クレート — FS 変更検出

### Decision
`notify` (v8) + `notify-debouncer-mini` でカスタムデバウンスを実装。`RecommendedWatcher` + `RecursiveMode::NonRecursive` で個別ディレクトリを監視。

### Rationale
- notify はクロスプラットフォーム（inotify/FSEvents/kqueue/ReadDirectoryChangesW）で実績がある
- `NonRecursive` モードで展開中ディレクトリのみ監視可能
- `watch()` / `unwatch()` で動的にディレクトリの追加・削除が可能
- debouncer-mini は軽量で 250ms デバウンスに適する

### Key Findings

**非再帰監視**: `watcher.watch(path, RecursiveMode::NonRecursive)` で単一ディレクトリのみ監視。展開/折り畳みに連動して `watch()` / `unwatch()` を呼ぶ。

**デバウンス**: `notify-debouncer-mini` の `Config::default().with_timeout(Duration::from_millis(250))` でデバウンス間隔を設定。

**一時停止**: notify には pause/resume API がない。代わりに `AtomicBool` フラグで自己操作中のイベントをフィルタリングする。操作前にフラグをセット、完了後にクリア。イベントハンドラ内でフラグをチェックし、セット中はイベントを無視する。

**tokio 統合**: notify のイベントを `std::sync::mpsc` で受信し、`tokio::sync::mpsc` に転送するブリッジスレッドを使う。または `crossbeam-channel` + `tokio::task::spawn_blocking` でポーリング。

**プラットフォーム差異**:
- macOS FSEvents: 元々 Time Machine/Spotlight 用。精密なイベントではなく「何か変わった」指標。notify が差異を吸収
- Linux inotify: watch 数制限あり（デフォルト低め）。notify は上限到達時にポーリングにフォールバック
- kqueue (BSD/macOS fallback): ファイルごとにカーネルハンドルが必要
- Windows ReadDirectoryChangesW: ディレクトリ単位の監視

### Alternatives Considered
- **手動 polling**: 確実だが CPU 使用量が高い。250ms 間隔のポーリングは FS 監視としては非効率
- **tokio-notify**: 存在しない。notify + tokio ブリッジが標準的なアプローチ

---

## 2. trash クレート — OS標準ゴミ箱

### Decision
`trash` クレート (v5) を `D` キーの OS 標準ゴミ箱操作に使用。`trash::delete()` と `trash::delete_all()` を利用。

### Rationale
- クロスプラットフォーム: Linux (freedesktop.org spec)、macOS (NSFileManager)、Windows (IFileOperation)
- シンプルな API: `trash::delete(path)` のみ
- yazi、nushell、czkawka など多くのプロジェクトで採用実績あり

### Key Findings

**API**:
- `trash::delete(path: impl AsRef<Path>) -> Result<(), Error>` — 単一ファイル/ディレクトリ
- `trash::delete_all(paths: &[impl AsRef<Path>]) -> Result<(), Error>` — 複数ファイル
- ブロッキング API のため `tokio::task::spawn_blocking` で実行

**macOS 制限**: `os_limited` モジュール（list/restore）は macOS で利用不可。これが独自 trash 設計の動機（前回セッションで確認済み）。

**tokio 統合パターン** (yazi の実装を参考):
```rust
tokio::task::spawn_blocking(move || {
    trash::delete(path).map_err(io::Error::other)
}).await?
```

### Alternatives Considered
- **直接 OS API 呼び出し**: プラットフォームごとの実装が必要。trash クレートで十分
- **freedesktop-trash**: Linux のみ。クロスプラットフォーム不可

---

## 3. 独自 trash の実装パターン

### Decision
`~/.local/share/trev/trash/` (Linux) / `~/Library/Application Support/trev/trash/` (macOS) にファイルを Move して保管。undo 時に逆 Move で復元。

### Rationale
- vifm の実証済みパターン: trash = Move 操作 → undo = 逆 Move
- macOS の `trash` クレート制限を回避
- メタデータを OpGroup に内包（追加の管理ファイル不要）
- felix (Rust ファイルマネージャー) も同様のアプローチで実装済み

### Key Findings
- ファイル名: `{unix_timestamp_nanos}_{original_name}` で衝突回避
- `std::fs::rename()` が同一ファイルシステム内なら O(1)（メタデータ操作のみ）
- 異なるファイルシステム間は `fs::copy()` + `fs::remove_file()` にフォールバック必要
- `dirs::data_dir()` でプラットフォーム規約に従ったパスを取得

---

## 4. セッション永続化のアトミック書き込み

### Decision
write-rename パターンで JSON ファイルのアトミック書き込みを実現。

### Rationale
- クラッシュ時に中途半端なファイルが残ることを防止
- Unix/macOS では `rename()` はアトミック操作
- 多くの設定ファイル管理ツール (serde_json, confy) で採用されているパターン

### Key Findings
```
1. 一時ファイルに書き込み: {hash}.json.tmp
2. fsync() で永続化を保証
3. rename() で本番ファイルに置換: {hash}.json.tmp → {hash}.json
```
- Windows では `rename()` がアトミックでない場合がある → `tempfile` クレートの `NamedTempFile::persist()` がクロスプラットフォーム対応

---

## 5. FS 操作のエラーハンドリング

### Decision
`std::io::Error` を `anyhow::Error` にラップ。操作失敗時はユーザーにエラーメッセージを表示し、部分的に完了した操作はそのまま残す（ロールバックしない）。

### Rationale
- spec での決定: クラッシュ時は放置（ユーザー手動対応）
- シンプルさ優先: トランザクション管理のコストは TUI ツールに見合わない
- vifm も同様のアプローチ: 部分失敗はエラー通知のみ

### Key Findings
- `std::fs::copy()`: 失敗時に部分的なコピー先ファイルが残る可能性あり
- `std::fs::rename()`: 同一 FS 内ならアトミック、異なる FS 間ではエラー
- ディレクトリの再帰コピー: `walkdir` + 個別ファイルコピーで実装
- 権限エラー (`PermissionDenied`): 操作前のチェックは TOCTOU のため不採用。操作時のエラーを捕捉
