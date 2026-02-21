# Research: Column Layout Design

## Decision 1: カラムフォーマット関数の配置

**Decision**: `src/ui/column.rs` に新モジュールとして配置

**Rationale**:
- `format_size()` と `format_mtime()` は純粋関数であり、独立したテストが容易
- `tree_view.rs` は既に 250 行以上あり、フォーマット・幅計算ロジックを分離する方が可読性が高い
- ratatui の Span 生成もここに集約（`render_column_spans()`）

**Alternatives considered**:
- `tree_view.rs` 内にインライン → テスト可能だがファイルが肥大化
- `src/format.rs` トップレベルモジュール → UI 以外に使う予定がなく YAGNI

## Decision 2: ファイルサイズフォーマット

**Decision**: 手動実装（外部クレートなし）

**Rationale**:
- `humansize` クレートは存在するが、表示幅の固定（5文字右寄せ）というカスタム要件に対して自前実装の方がシンプル
- ロジックは単純: B/K/M/G/T の閾値判定と `format!` のみ
- 依存クレートを増やさない（constitution IV: シンプルさ）

**Format spec**:
- `0` → `  0B`
- `1023` → `1023B`
- `1024` → ` 1.0K`
- `1536` → ` 1.5K`
- `1048576` → ` 1.0M`
- ディレクトリ → `   -`

## Decision 3: 更新日時フォーマット

**Decision**: `std::time::SystemTime` → `chrono` 不使用、`time` クレート不使用。手動で UNIX epoch 差分から計算

**Rationale**:
- FR-004 の5ティアは全て「現在時刻との差分」で決定可能
- ISO 形式（1ヶ月以上）のみ年月日が必要 → `SystemTime::duration_since(UNIX_EPOCH)` + 手動計算
- 外部日付クレートの追加は YAGNI（ソート用の mtime 比較は既に `SystemTime` の `cmp` で行われている）

**Alternatives considered**:
- `chrono` → 重い依存。ISO フォーマットだけのために不要
- `time` → 軽量だがまだ不要。将来的に必要になれば移行は容易

**注意**: ISO 形式 `YYYY-MM-DD` の生成には epoch seconds → year/month/day の変換が必要。これは閏年を考慮した計算になるが、`time` クレートの `OffsetDateTime::from_unix_timestamp` が最もクリーン。依存追加のトレードオフを検討。

**改訂**: `time` クレートを使用する。ISO 日付の手動計算は閏年・タイムゾーンの罠が多く、バグリスクが高い。`time = { version = "0.3", features = ["local-offset"] }` を追加。

## Decision 4: 名前切り詰め

**Decision**: Unicode 表示幅を考慮した切り詰め。`unicode-width` クレートを使用

**Rationale**:
- CJK文字は2文字幅を占める（例: 日本語ファイル名）
- バイト数やchar数での切り詰めは表示がずれる
- `unicode-width` は ratatui の依存として既に存在（再export or 直接参照）

**実装**:
- 表示幅が残り幅を超える場合、末尾を `…` で置換
- `unicode-width::UnicodeWidthStr::width()` で幅計算

## Decision 5: 設定のカラム指定方法

**Decision**: `display.columns` にカラムエントリのリストとして指定。各エントリは文字列（簡易形式）またはオブジェクト（オプション付き形式）。`mtime` は `modified_at` にリネーム。

**Rationale**:
- 順序と表示/非表示を1つのフィールドで制御
- リストに含まれるカラムのみ表示、順序はリスト順
- デフォルト: `[size, modified_at, git_status]`
- 将来のカラムごとオプション拡張に備え、文字列/オブジェクトの両形式をサポート
- `mtime` → `modified_at`: ユーザー向け設定名として直感的（内部実装では `Mtime` variant は `ModifiedAt` にリネーム）

**Serde 実装**: `ColumnEntry` を serde の `untagged` enum として定義。文字列 → `ColumnKind` 直結、オブジェクト → `kind` + オプションフィールド。

**Config YAML example**:
```yaml
display:
  # デフォルト（全て文字列形式）
  columns:
    - size
    - modified_at
    - git_status

  # modified_at を絶対日時表示に
  columns:
    - size
    - kind: modified_at
      format: absolute
    - git_status

  # git を左に、modified_at 非表示
  columns:
    - git_status
    - size

  # カラムなし
  columns: []
```

**Alternatives considered**:
- フラットリスト `[size, mtime, git_status]` のみ → カラムごとオプションが不可。将来の拡張性が低い
- 全エントリをオブジェクト強制 → 設定の冗長さが増す。文字列/オブジェクト混在がベスト

## Decision 6: git_status カラム統合

**Decision**: 既存の git status 表示を ColumnKind::GitStatus に統合

**Rationale**:
- 現在の `METADATA_WIDTH = 2` のハードコードを廃止
- `display.columns` に `git_status` が含まれる場合のみ表示
- `git_enabled: false` の場合、カラムリストから `git_status` を自動除外

**Migration**:
- デフォルト columns に `git_status` を含めるため、既存ユーザーの動作は変わらない
- `git_enabled: false` + `columns: [git_status]` → git_status は表示されない（サイレント除外）
