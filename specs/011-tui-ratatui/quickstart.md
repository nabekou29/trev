# Quickstart: TUI 基盤

## 起動

```bash
# カレントディレクトリを表示
cargo run

# 指定ディレクトリを表示
cargo run -- /path/to/dir

# アイコンなしで表示
cargo run -- --no-icons
```

## キー操作

| Key | Action |
|-----|--------|
| `j` / `↓` | カーソル下 |
| `k` / `↑` | カーソル上 |
| `l` / `→` | 展開 / ファイルオープン |
| `h` / `←` | 折り畳み / 親移動 |
| `Enter` | トグル (展開↔折り畳み) |
| `g` | 先頭 |
| `G` | 末尾 |
| `Ctrl+d` | 半ページ下 |
| `Ctrl+u` | 半ページ上 |
| `q` | 終了 |

## モジュール構成

```
main.rs                 # #[tokio::main], Args parse, app::run()
├── terminal.rs         # init() / restore() — ターミナル管理
├── event.rs            # handle_key_event() — キーイベント処理
├── app.rs              # run() — イベントループ本体
└── ui/
    ├── tree_view.rs    # render_tree() — ツリー描画
    └── status_bar.rs   # render_status() — ステータスバー描画
```

## Key Patterns

### ターミナル初期化

```rust
// ratatui 0.30 のヘルパーを使用
let mut terminal = ratatui::init();
// ... アプリケーションロジック ...
ratatui::restore();
```

### イベントループ

```rust
loop {
    terminal.draw(|frame| ui::render(frame, &state))?;

    if crossterm::event::poll(Duration::from_millis(50))? {
        match crossterm::event::read()? {
            Event::Key(key) => event::handle_key_event(key, &mut state, &children_tx),
            Event::Resize(_, _) => {} // 次フレームで再描画
            _ => {}
        }
    }

    // 非同期結果の受信
    while let Ok(result) = children_rx.try_recv() {
        state.set_children(&result.path, result.children);
    }

    if state.should_quit { break; }
}
```

### 非同期ディレクトリ読み込み

```rust
// expand_or_open/toggle_expand で NeedsLoad が返った場合
let tx = children_tx.clone();
let builder = TreeBuilder::new(show_hidden, show_ignored);
tokio::spawn_blocking(move || {
    let children = builder.load_children(&path);
    tx.blocking_send(ChildrenLoadResult { path, result: children });
});
```

### ツリー描画

```rust
// visible_nodes() からスクロール範囲を切り出して描画
let visible = state.visible_nodes();
let offset = compute_scroll_offset(state.cursor(), viewport_height, current_offset);
for (i, vnode) in visible.iter().skip(offset).take(viewport_height).enumerate() {
    let indent = "  ".repeat(vnode.depth);
    let icon = if show_icons { get_icon(&vnode.node.path) } else { "" };
    let line = format!("{indent}{icon} {name}", name = vnode.node.name);
    // highlight if i + offset == cursor
}
```

## テスト方針

- **ロジック**: event::handle_key_event のユニットテスト (TreeState の状態変化を検証)
- **UI**: 目視確認が主。将来的に ratatui の `TestBackend` でスナップショットテスト検討
- **統合**: `cargo run` で実際のディレクトリを表示して操作確認
