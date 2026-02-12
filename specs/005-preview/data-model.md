# Data Model: ファイルプレビュー

## 既存エンティティ (変更なし)

### TreeNode / TreeState / VisibleNode
`src/state/tree.rs` に定義済み。プレビュー機能からはカーソル位置のパスを取得するために参照。

### AppState / ScrollState
`src/app.rs` に定義済み。プレビュー状態を `AppState` に追加。

### Config
`src/config.rs` に定義済み。プレビュー設定セクションを追加。

## 新規エンティティ

### PreviewContent (`src/preview/content.rs`)

プレビューの内容を表す列挙型。各プロバイダーが `load()` で返す。

```rust
pub enum PreviewContent {
    /// シンタックスハイライト済みテキスト (syntect → ratatui Lines)
    HighlightedText {
        lines: Vec<Line<'static>>,
        language: String,
        truncated: bool,
    },
    /// プレーンテキスト (ハイライトなし)
    PlainText {
        lines: Vec<String>,
        truncated: bool,
    },
    /// ANSI カラー出力 (外部コマンド → ansi-to-tui)
    AnsiText {
        text: Text<'static>,
    },
    /// 画像 (ratatui-image StatefulProtocol)
    Image {
        protocol: Box<dyn StatefulProtocol>,
    },
    /// バイナリファイル情報
    Binary {
        size: u64,
    },
    /// ディレクトリ情報
    Directory {
        entry_count: usize,
        total_size: u64,
    },
    /// 読み込み中
    Loading,
    /// エラー
    Error {
        message: String,
    },
    /// 空ファイル
    Empty,
}
```

### PreviewState (`src/preview/state.rs`)

プレビューの表示状態を管理。

- `content: PreviewContent` — 現在表示中のコンテンツ
- `scroll_row: usize` — 縦スクロール位置
- `scroll_col: usize` — 横スクロール位置
- `current_path: Option<PathBuf>` — 現在プレビュー中のパス
- `cancel_token: CancellationToken` — 読み込みキャンセル用トークン
- `active_provider_index: usize` — 現在のプロバイダーインデックス
- `available_providers: Vec<String>` — 利用可能なプロバイダー名リスト

**メソッド**:
- `request_preview(path)` — 新しいファイルのプレビューをリクエスト (スクロールリセット、前のトークンキャンセル)
- `set_content(content)` — プレビュー内容を設定
- `scroll_down(n)` / `scroll_up(n)` — 縦スクロール
- `scroll_right(n)` / `scroll_left(n)` — 横スクロール
- `cycle_provider()` — 次のプロバイダーへ切替

### PreviewProvider trait (`src/preview/provider.rs`)

プレビュー生成の抽象インターフェース。

```rust
pub trait PreviewProvider: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> u32;
    fn can_handle(&self, path: &Path, is_dir: bool) -> bool;
    fn is_enabled(&self, available: &[&str]) -> bool { true }
    fn load(&self, path: &Path, ctx: &LoadContext) -> anyhow::Result<PreviewContent>;
}
```

### LoadContext (`src/preview/provider.rs`)

プロバイダーの `load()` に渡すコンテキスト。

- `max_lines: usize` — 読み込み最大行数 (デフォルト: 1000)
- `max_bytes: u64` — 読み込み最大バイト数 (デフォルト: 10MB)
- `cancel_token: CancellationToken` — キャンセルトークン

### PreviewRegistry (`src/preview/provider.rs`)

登録されたプロバイダーを管理し、ファイルごとの利用可能リストを返す。

- `providers: Vec<Box<dyn PreviewProvider>>` — 優先度順に格納

**メソッド**:
- `resolve(path, is_dir) -> Vec<&dyn PreviewProvider>` — `can_handle()` → `is_enabled()` でフィルタした有効プロバイダーリスト

### PreviewCache (`src/preview/cache.rs`)

LRU キャッシュで読み込み済みプレビューを保持。

- `cache: LruCache<CacheKey, PreviewContent>` — LRU キャッシュ (容量: 10)

**CacheKey**:
- `path: PathBuf` — ファイルパス
- `provider_name: String` — プロバイダー名

**メソッド**:
- `get(key) -> Option<&PreviewContent>`
- `put(key, content)`

### 具体プロバイダー

| プロバイダー | ファイル | Priority | is_enabled |
|------------|---------|----------|------------|
| TextPreviewProvider | `src/preview/providers/text.rs` | 30 | デフォルト `true` |
| ImagePreviewProvider | `src/preview/providers/image.rs` | 20 | デフォルト `true` |
| ExternalCmdProvider | `src/preview/providers/external.rs` | 10 | デフォルト `true` |
| FallbackProvider | `src/preview/providers/fallback.rs` | 100 | `available.len() <= 1` |

### PreviewConfig (`src/config.rs` に追加)

プレビュー関連の設定。

- `max_lines: usize` — 表示最大行数 (デフォルト: 1000)
- `max_bytes: u64` — 読み込み最大バイト数 (デフォルト: 10MB)
- `cache_size: usize` — LRU キャッシュサイズ (デフォルト: 10)
- `commands: Vec<ExternalCommand>` — 外部コマンド定義
- `command_timeout: u64` — 外部コマンドタイムアウト秒数 (デフォルト: 3)

**ExternalCommand**:
- `extensions: Vec<String>` — 対象拡張子
- `command: String` — コマンド名
- `args: Vec<String>` — 引数

### PreviewAction (`src/action.rs` に追加)

プレビュー関連のアクション。

```rust
pub enum PreviewAction {
    ScrollDown,
    ScrollUp,
    ScrollRight,
    ScrollLeft,
    HalfPageDown,
    HalfPageUp,
    CycleProvider,
}
```

## 状態遷移

### プレビュー読み込みフロー

```
None → [cursor move] → Loading → [load complete] → Loaded (content)
                                → [load error]    → Error
                                → [cancelled]     → (new Loading for next file)
```

### プロバイダー選択フロー

```
[file selected]
  → can_handle() を優先度順に評価 → 候補リスト
  → is_enabled(候補名一覧) でフィルタ → 最終リスト
  → index=0 のプロバイダーで load()
  → [Tab] → index = (index + 1) % len → 新プロバイダーで load()
```

## 既存エンティティへの変更

### AppState (`src/app.rs`)
- `+ preview_state: PreviewState`
- `+ preview_cache: PreviewCache`
- `+ preview_registry: PreviewRegistry`

### Config (`src/config.rs`)
- `+ preview: PreviewConfig`

### Action (`src/action.rs`)
- `+ Preview(PreviewAction)` バリアント追加
