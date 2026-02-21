# Quickstart: Column Layout Design

## 動作確認

### P1: メタデータカラム表示

```sh
# デフォルト設定で起動 — サイズ・modified_at・git status が右端に表示される
cargo run -- /path/to/project

# アイコンなしで起動（カラムのスペースが増える）
cargo run -- --no-icons /path/to/project
```

期待される表示:
```
  ▸ src/               -    3d ago M
    main.rs         1.2K    2h ago
    lib.rs          3.4K 2024-01-15
    link.txt → target.txt  42B   30m ago
```

### P2: カラム設定

```yaml
# ~/.config/trev/config.yml — サイズのみ表示
display:
  columns:
    - size
```

```yaml
# カラムなし（現行の動作に近い）
display:
  columns: []
```

```yaml
# git を左に、順序変更
display:
  columns:
    - git_status
    - size
    - modified_at
```

```yaml
# modified_at を絶対日時表示に
display:
  columns:
    - size
    - kind: modified_at
      format: absolute
    - git_status
```

### P3: レイアウト設定

```yaml
preview:
  split: 40              # wide: ツリー40% / プレビュー60%
  narrow_split: 50       # narrow: 50/50
  narrow_threshold: 100  # 幅100以下で縦分割
```

## テスト

```sh
mise run test              # 全テスト
mise run lint              # clippy
cargo test column          # column モジュールのテストのみ
cargo test format_size     # サイズフォーマットのテストのみ
cargo test format_mtime    # modified_at フォーマットのテストのみ
```

## 設定スキーマの確認

```sh
cargo run -- schema | jq '.properties.display.properties.columns'
```
