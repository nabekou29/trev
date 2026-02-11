# Research: TUI 基盤

## R-001: ratatui イベントループパターン

**Decision**: crossterm polling + tokio::sync::mpsc チャネル

**Rationale**:
- `crossterm::event::poll(Duration)` + `crossterm::event::read()` が最もシンプル
- ratatui 公式サンプル・多くの production アプリがこのパターンを採用
- 非同期結果は `mpsc::Receiver::try_recv()` でノンブロッキング受信
- `EventStream` + `tokio::select!` は IPC 統合時に必要になれば検討

**Alternatives considered**:
- `EventStream` (crossterm feature): 将来 IPC 追加時に移行可能だが、現段階では over-engineering
- std thread + channel: tokio 不要だが、将来の IPC 対応で結局 tokio 化が必要

## R-002: ターミナル初期化・復帰

**Decision**: `ratatui::init()` / `ratatui::restore()`

**Rationale**:
- ratatui 0.30 で追加されたヘルパー関数
- 自動的に raw mode、alternate screen、パニックフックを設定
- 手動の `crossterm::execute!` より安全・簡潔
- パニック時もターミナル復帰が保証される

**Alternatives considered**:
- 手動 `Terminal::new(CrosstermBackend)` + `enable_raw_mode`: ボイラープレートが多い
- `color_eyre` + install hooks: 依存追加が不要なので ratatui 標準で十分

## R-003: 非同期ディレクトリ読み込み

**Decision**: `tokio::spawn_blocking` + mpsc channel

**Rationale**:
- `ignore::WalkBuilder` は同期 API → blocking pool でオフロード
- `mpsc::channel` で (PathBuf, Vec<TreeNode>) を UI スレッドに送信
- メインループの `try_recv()` でノンブロッキング受信 → `set_children()` で反映
- TreeBuilder は既に同期関数として実装済み → そのまま再利用

**Alternatives considered**:
- `tokio::fs` で完全 async: ignore crate との統合が困難
- `std::thread::spawn`: tokio runtime が既にあるので spawn_blocking の方が自然

## R-004: スクロール/ビューポート管理

**Decision**: 自前のスクロールオフセット管理

**Rationale**:
- TreeState の `visible_nodes()` + `cursor()` からスクロールオフセットを算出
- `offset = max(0, cursor - viewport_height + margin)` のシンプルなロジック
- ratatui の `ListState` は使用しない (カスタムインデント・アイコン表示が必要)
- tree_view ウィジェットの `render()` で offset からの visible_nodes スライスを描画

**Alternatives considered**:
- ratatui `ListState`: 自動スクロールだが、ツリーのインデント・アイコン表示をカスタマイズしにくい
- `StatefulWidget` + `ScrollbarState`: 将来スクロールバー追加時に検討

## R-005: devicons アイコン表示

**Decision**: devicons 0.6 の `FileIcon::from(path)` でアイコン取得

**Rationale**:
- Cargo.toml に既に依存あり
- ファイル拡張子からアイコン文字 + 色を返す
- `--no-icons` 時はアイコン列をスキップして描画
- Nerd Fonts が必要だが、開発者向けツールでは一般的

**Alternatives considered**:
- 手動マッピング: メンテナンスコスト大
- `nerdfix` crate: devicons で十分
