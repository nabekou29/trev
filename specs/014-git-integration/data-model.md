# Data Model: Git Integration

## Entities

### GitFileStatus

ファイル単体の Git ステータス。`git status --porcelain=v1` の2カラム (index, worktree) から導出。

```
enum GitFileStatus {
    Modified,       // ワーキングツリーで変更あり (Y=M)
    Staged,         // インデックスに変更あり (X=M|A|D|R, Y=空白)
    StagedModified, // ステージ済みだがさらにワーキングツリー変更あり (X≠空白, Y=M)
    Added,          // 新規追加・ステージ済み (X=A, Y=空白)
    Deleted,        // 削除済み (X=D or Y=D)
    Renamed,        // リネーム済み (X=R)
    Untracked,      // 未追跡 (??）
    Conflicted,     // マージコンフリクト (X=U or Y=U, or both)
}
```

**表示用マッピング** (仕様のインジケーター定義に対応):

| GitFileStatus | 表示文字 | 色 | 備考 |
|---------------|---------|-----|------|
| Modified | `M` | Yellow | Unstaged 変更 |
| Staged | `M` | Green | Staged のみ |
| StagedModified | `M` | Yellow | Unstaged 優先表示 (仕様) |
| Added | `A` | Green | 新規ステージ済み |
| Deleted | `D` | Red | ツリーに表示されない（集約用） |
| Renamed | `R` | Blue | リネーム先パスに表示 |
| Untracked | `?` | Magenta | 未追跡 |
| Conflicted | `!` | Red+Bold | コンフリクト |

### GitState

リポジトリ全体の Git ステータスを保持する構造体。

```
struct GitState {
    /// ファイルパス (絶対パス) → ステータスのマッピング。
    statuses: HashMap<PathBuf, GitFileStatus>,
}
```

**メソッド**:
- `file_status(path: &Path) -> Option<&GitFileStatus>`: ファイルの個別ステータス取得
- `dir_status(dir_path: &Path) -> Option<GitFileStatus>`: ディレクトリの集約ステータス (子孫の max priority)
- `from_porcelain(output: &str, repo_root: &Path) -> Self`: porcelain 出力からの構築

**ステータス優先度** (dir_status 集約用、高→低):
1. Conflicted
2. Modified / StagedModified
3. Added / Staged
4. Deleted
5. Renamed
6. Untracked

### GitConfig

Git 連携の設定。

```
struct GitConfig {
    /// Git 連携の有効/無効 (default: true)。
    enabled: bool,
}
```

`Config.git: GitConfig` として既存の設定構造に追加。

### GitStatusResult

非同期チャネルで送信される結果型。

```
struct GitStatusResult {
    /// 取得した Git ステータス (エラー時は None)。
    state: Option<GitState>,
}
```

`AppContext.git_tx: Sender<GitStatusResult>` を通じてイベントループに送信。

## State Transitions

```
GitState ライフサイクル:

  起動時:
    git.enabled=true && Git リポジトリ内
      → spawn_blocking で git status 取得
      → GitStatusResult 送信
      → AppState.git_state = Some(state)

  FS watcher イベント:
    process_watcher_events 内で git status 再取得をトリガー
      → 新しい GitState で置き換え

  R キー (tree.refresh):
    ツリー再構築 + git status 再取得をトリガー
      → 新しい GitState で置き換え

  git.enabled=false:
    AppState.git_state = None (一切取得しない)

  Git リポジトリ外:
    AppState.git_state = None
```

## Relationships

```
Config ─── GitConfig
              │
              ▼
AppState ─── Option<GitState>
              │
              ├── file_status(path) → tree_view 描画時に参照
              ├── dir_status(path)  → tree_view ディレクトリ集約
              └── (Arc<RwLock> 経由で ExternalCmdProvider が参照)

AppContext ─── git_tx: Sender<GitStatusResult>
              │
              ▼
Event Loop ── git_rx: Receiver<GitStatusResult>
              │
              ├── process_git_results() → AppState.git_state 更新
              └── process_watcher_events() → git status 再取得トリガー
```
