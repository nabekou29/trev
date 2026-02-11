# Feature Specification: ファイルプレビュー

**Feature Branch**: `005-preview`
**Created**: 2026-02-10
**Status**: Draft
**Input**: User description: "ファイルプレビュー — シンタックスハイライト、画像、外部コマンド、非同期読み込み、プレビューモード切替"

## User Scenarios & Testing

### User Story 1 - テキストファイルのシンタックスハイライトプレビュー (Priority: P1)

ユーザーがツリーでファイルを選択すると、プレビューペインにシンタックスハイライト付きのファイル内容が表示される。言語はファイル拡張子から自動判定される。

**Why this priority**: プレビューの中核機能。ファイル内容の確認はファイルブラウザの最も基本的なニーズ。

**Independent Test**: テストファイルを読み込み、ハイライト結果が正しく生成されることをユニットテストで検証。

**Acceptance Scenarios**:

1. **Given** `.rs` ファイルを選択, **When** プレビューが表示される, **Then** Rust のシンタックスハイライトが適用されている
2. **Given** `.md` ファイルを選択, **When** プレビューが表示される, **Then** Markdown のシンタックスハイライトが適用されている
3. **Given** 拡張子のないファイル, **When** プレビューが表示される, **Then** プレーンテキストとして表示される
4. **Given** 対応していない拡張子, **When** プレビューが表示される, **Then** プレーンテキストとして表示される
5. **Given** 空のファイル, **When** プレビューが表示される, **Then** "(empty)" 等の表示がされる

---

### User Story 2 - 非同期プレビュー読み込みとキャンセル (Priority: P1)

プレビューの読み込みは非同期で行われ、UI をブロックしない。大きなファイルでもカーソル移動がスムーズに動作する。ファイル選択が変わった場合、前回の読み込みはキャンセルされる。

**Why this priority**: 同期読み込みでは大きなファイル選択時に UI がフリーズする。非同期化は P1 と同時に実装すべき。

**Independent Test**: 大きなファイルのプレビュー読み込み中にカーソルを素早く移動し、UI がブロックされないことを確認。

**Acceptance Scenarios**:

1. **Given** ファイルを選択, **When** プレビュー読み込み中, **Then** "Loading..." 表示がされ、カーソル移動は可能
2. **Given** ファイル A のプレビュー読み込み中, **When** ファイル B を選択, **Then** ファイル A の読み込みはキャンセルされ、ファイル B の読み込みが開始される
3. **Given** 10MB のテキストファイル, **When** プレビューが読み込まれる, **Then** 先頭部分のみ読み込んで表示し、UI は 16ms 以内に応答可能
4. **Given** プレビュー読み込み完了後, **When** 同じファイルを再選択, **Then** キャッシュから即座に表示される

---

### User Story 3 - プレビューのスクロール (Priority: P1)

ユーザーはプレビュー内容を縦横にスクロールできる。長い行やファイル全体を閲覧可能。

**Why this priority**: プレビューがスクロールできなければ、先頭しか見られず実用性が大幅に低下する。

**Independent Test**: プレビューが表示された状態でスクロールキーを押し、表示範囲が正しく変わることを確認。

**Acceptance Scenarios**:

1. **Given** 100 行のファイル（viewport=20行）, **When** 下スクロール, **Then** 表示範囲が下にシフトする
2. **Given** 長い行を含むファイル, **When** 右スクロール, **Then** 表示が右にシフトし、折り返されていない行の続きが見える
3. **Given** スクロール位置が先頭, **When** 上スクロール, **Then** スクロール位置は先頭のまま
4. **Given** スクロール位置が末尾, **When** 下スクロール, **Then** スクロール位置は末尾のまま
5. **Given** ファイル A でスクロール済み, **When** ファイル B を選択, **Then** スクロール位置は (0, 0) にリセットされる

---

### User Story 4 - バイナリファイルの検出と表示 (Priority: P2)

バイナリファイルを自動検出し、適切なメッセージを表示する。バイナリファイルの内容はテキストとしてプレビューしない。

**Why this priority**: バイナリファイルを無理にテキスト表示すると文字化けや UI 破壊が起きる。

**Independent Test**: バイナリファイルを選択し、適切なメッセージが表示されることを確認。

**Acceptance Scenarios**:

1. **Given** 画像プレビュー非対応の端末で PNG ファイルを選択, **When** プレビューが表示される, **Then** "Binary file (1.2 MB)" のようなメッセージが表示される
2. **Given** 実行可能バイナリを選択, **When** プレビューが表示される, **Then** バイナリファイルとして扱われる
3. **Given** UTF-8 テキストファイル, **When** プレビューが判定される, **Then** バイナリと誤判定されない

---

### User Story 5 - ディレクトリプレビュー (Priority: P2)

ディレクトリを選択した場合、そのディレクトリの概要情報（子エントリ数等）をプレビューとして表示する。

**Why this priority**: ディレクトリ選択時にプレビューが空だとユーザー体験が損なわれるが、ファイルプレビューより優先度は低い。

**Independent Test**: ディレクトリを選択し、概要情報が表示されることを確認。

**Acceptance Scenarios**:

1. **Given** ファイルとサブディレクトリを含むディレクトリ, **When** 選択, **Then** エントリ数が表示される
2. **Given** 空のディレクトリ, **When** 選択, **Then** "Empty directory" 等が表示される

---

### User Story 6 - プレビューモード切替 (Priority: P2)

1つのファイルに対して複数のプレビュープロバイダーが `can_handle()` で利用可能と判定された場合、ユーザーはキー操作でプレビューモードを切り替えられる。例えば SVG ファイルは ImageProvider と TextProvider の両方がサポートを表明し、ユーザーが自由に切り替えられる。

**Why this priority**: 複数のプレビュー方式がある場合に片方しか使えないのは不便。ただし各プレビュー方式が実装された後に統合する。

**Independent Test**: SVG ファイルを選択し、画像プレビューとテキストプレビューをキーで切り替えられることを確認。

**Acceptance Scenarios**:

1. **Given** SVG ファイルを選択（ImageProvider と TextProvider が both `can_handle() = true`）, **When** デフォルトプレビュー, **Then** 優先度が最も高いプロバイダー（ImageProvider）で表示される
2. **Given** SVG ファイルを画像プレビュー中, **When** プレビューモード切替キーを押す, **Then** 次の利用可能プロバイダー（TextProvider）に切り替わる
3. **Given** テキストプレビュー中, **When** 再度切替キーを押す, **Then** 画像プレビューにラップアラウンドで戻る
4. **Given** プレビュー方式が1つしかないファイル, **When** 切替キーを押す, **Then** 何も起きない（または通知）
5. **Given** 外部コマンドプロバイダーが `can_handle() = true` のファイル, **When** 切替キーを押す, **Then** ビルトイン ↔ 外部コマンドプレビューを切り替えられる
6. **Given** プレビューモードを切り替えた, **When** ステータスバーを見る, **Then** 現在のプロバイダー名（例: "Image", "Text", "bat"）が表示されている

---

### User Story 7 - 画像プレビュー (Priority: P3)

画像ファイル（PNG, JPEG, GIF, WebP, SVG 等）を選択すると、端末が対応するグラフィックスプロトコル（Sixel, Kitty Graphics, iTerm2）を使って画像がプレビューに表示される。対応していない端末ではフォールバック表示（Unicode halfblocks）を使用する。ImageProvider は `can_handle()` で画像フォーマットの対応を判定する。

**Why this priority**: 画像プレビューは差別化ポイントだが、テキストプレビューの安定後に実装すべき。

**Independent Test**: 画像ファイルを選択し、対応端末でインラインプレビューが表示されることを確認。

**Acceptance Scenarios**:

1. **Given** Kitty 端末で PNG ファイルを選択, **When** プレビューが表示される, **Then** Kitty Graphics プロトコルで画像が表示される
2. **Given** Sixel 対応端末で JPEG ファイルを選択, **When** プレビューが表示される, **Then** Sixel プロトコルで画像が表示される
3. **Given** グラフィックスプロトコル非対応端末, **When** 画像ファイルを選択, **Then** Unicode halfblocks でフォールバック表示される
4. **Given** SVG ファイルを選択, **When** ImageProvider が `can_handle()` を判定, **Then** `true` を返し、ラスタライズして画像として表示される
5. **Given** 対応していない画像フォーマット, **When** ImageProvider が `can_handle()` を判定, **Then** `false` を返し、次のプロバイダーにフォールバック

---

### User Story 8 - 外部コマンドによるプレビュー (Priority: P3)

ユーザーは設定ファイルで外部プレビューコマンドを定義できる。ExternalCmdProvider は設定された条件（拡張子、MIME タイプ等）に基づいて `can_handle()` を判定し、マッチした場合に外部コマンドを実行して ANSI カラー出力を ratatui テキストとしてレンダリングする。

**Why this priority**: カスタマイズ性の向上だが、ビルトインプレビューが安定してから実装すべき。

**Independent Test**: 設定で外部コマンドを定義し、対応ファイルで外部コマンドの出力が表示されることを確認。

**Acceptance Scenarios**:

1. **Given** `*.csv` に対して外部コマンドが設定されている, **When** CSV ファイルで ExternalCmdProvider が `can_handle()`, **Then** `true` を返す
2. **Given** 外部コマンドが ANSI カラーを含む出力をする, **When** プレビューが表示される, **Then** ANSI カラーが ratatui のスタイルとして正しくレンダリングされる
3. **Given** 外部コマンドが存在しない, **When** `can_handle()` 判定, **Then** `false` を返し、次のプロバイダーにフォールバック
4. **Given** 外部コマンドがタイムアウト（3秒超）, **When** プレビューが読み込まれる, **Then** コマンドが kill され、エラーとして次のプロバイダーにフォールバック

---

### Edge Cases

- 読み取り権限のないファイル → エラーメッセージをプレビューに表示
- ファイルが読み込み中に削除される → エラーを適切に処理し、エラーメッセージを表示
- 非常に大きなファイル（100MB+）→ 先頭の一定量のみ読み込み、"File truncated" を表示
- 非 UTF-8 テキストファイル（Shift-JIS 等）→ v1.0 では UTF-8 のみサポート。バイナリとして検出される可能性あり
- シンボリックリンク先のファイル → リンク先の内容をプレビュー
- FIFO やソケット等の特殊ファイル → "Special file" メッセージを表示
- 壊れた画像ファイル → ImageProvider が `load()` でエラー → 次のプロバイダーにフォールバック
- 外部コマンドが無限出力する → 出力量に上限を設けて切り捨て
- プレビューモード切替後にファイルが変更される → 新しいファイルのデフォルトプロバイダーにリセット
- すべてのプロバイダーが `can_handle() = false` → FallbackProvider が処理

## Requirements

### Functional Requirements

- **FR-001**: `PreviewContent` は以下のバリアントを持つ列挙型: `HighlightedText`, `PlainText`, `AnsiText`, `Image`, `Binary`, `Directory`, `Loading`, `Error`, `Empty`
- **FR-002**: シンタックスハイライトは `syntect` + `two-face` を使用し、ファイル拡張子から言語を自動判定する
- **FR-003**: ハイライトテーマは設定で変更可能（デフォルト: 端末背景に合わせた自動選択）
- **FR-004**: プレビュー読み込みは tokio タスクとして非同期実行し、UI スレッドをブロックしない
- **FR-005**: ファイル選択変更時、前回の読み込みタスクを `CancellationToken` でキャンセルする
- **FR-006**: 大きなファイルは先頭の一定量のみ読み込む（上限は設定可能、デフォルト: 1000 行）
- **FR-007**: バイナリ判定は先頭バイトの NUL 文字検出で行う
- **FR-008**: プレビューは縦横スクロール可能で、スクロール位置（行オフセット、列オフセット）を管理する
- **FR-009**: ファイル切り替え時にスクロール位置はリセットされる
- **FR-010**: プレビュー読み込み結果はキャッシュし、同じファイル・同じプロバイダーの再選択時は再読み込みしない（LRU キャッシュ）
- **FR-011**: `PreviewProvider` トレイトは `can_handle(path, metadata) -> bool`, `load(path, ctx) -> Result<PreviewContent>`, `name() -> &str`, `priority() -> u32` を定義
- **FR-012**: `PreviewRegistry` はファイルに対して `can_handle() = true` を返す全プロバイダーを優先度順に収集し、デフォルトは最高優先度のプロバイダーを使用
- **FR-013**: ユーザーはキー操作で利用可能なプロバイダーを順にサイクルできる（ラップアラウンド）
- **FR-014**: 画像プレビューは `ratatui-image` crate を使用し、端末のグラフィックスプロトコルを自動検出する
- **FR-015**: 画像プレビュー対応フォーマット: PNG, JPEG, GIF, WebP, SVG。対応端末: Sixel, Kitty Graphics, iTerm2。非対応端末: Unicode halfblocks フォールバック
- **FR-016**: 外部コマンドプロバイダーは設定ファイルで定義された条件（拡張子等）を `can_handle()` で判定する
- **FR-017**: 外部コマンドの ANSI カラー出力は `ansi-to-tui` crate で ratatui Text に変換する
- **FR-018**: 外部コマンドにはタイムアウト（デフォルト: 3秒）を設け、超過時は kill してエラーを返す
- **FR-019**: 現在のプロバイダー名はステータスバーに表示される

### Non-Functional Requirements

- **NFR-001**: プレビュー切替時の UI 応答は 16ms 以内（読み込み中は "Loading..." を即座に表示）
- **NFR-002**: シンタックスハイライトの処理は 1000 行のファイルで 50ms 以内
- **NFR-003**: プレビューキャッシュのメモリ使用量に上限を設ける（LRU、デフォルト: 最大 10 ファイル）
- **NFR-004**: 画像デコードはバックグラウンドで行い、UI をブロックしない

### Key Entities

- **PreviewContent**: プレビューの内容を表す列挙型
- **PreviewState**: 現在のプレビュー状態（content, scroll_offset, loading_path, cancel_token, active_provider_index）
- **PreviewProvider** (トレイト): プレビュー生成の抽象。`can_handle()` でサポート判定、`priority()` で優先順位、`load()` で内容生成、`name()` で表示名
  - **TextPreviewProvider**: syntect ベースのシンタックスハイライト（テキストファイル全般）
  - **ImagePreviewProvider**: ratatui-image ベースの画像プレビュー（画像フォーマット判定）
  - **ExternalCmdProvider**: 設定された外部コマンド（設定条件による判定）
  - **FallbackProvider**: バイナリ/ディレクトリ/空（常に `can_handle() = true`、最低優先度）
- **PreviewRegistry**: 登録されたプロバイダーを管理し、ファイルごとに利用可能なプロバイダーリストを返す
- **Highlighter**: syntect ベースのシンタックスハイライトエンジン（スレッドセーフ、再利用可能）
- **PreviewCache**: LRU キャッシュ（(パス, プロバイダー名) → PreviewContent）

## Success Criteria

### Measurable Outcomes

- **SC-001**: プレビュー切替時に UI がフリーズするケースがゼロ
- **SC-002**: シンタックスハイライトが正しく適用される主要言語カバー率 100%（Rust, Python, JS/TS, Go, C/C++, Java, JSON, YAML, TOML, Markdown）
- **SC-003**: バイナリファイルがテキストとして表示されるケースがゼロ
- **SC-004**: プレビューキャッシュにより同じファイル・同じプロバイダーの再表示が 1ms 以内
- **SC-005**: 画像プレビューがサポート端末（Kitty, iTerm2, Sixel 対応端末）で正しく表示される
- **SC-006**: 外部コマンドのタイムアウトやエラーでアプリケーションがクラッシュするケースがゼロ
- **SC-007**: 複数プロバイダーが利用可能なファイルで、切替操作が正しく動作する

## Assumptions

- UTF-8 のみサポート。他の文字エンコーディングは将来対応
- シンタックスハイライトは syntect のビルトインテーマ + two-face のビルトイン言語定義を使用
- 画像プレビューは `ratatui-image` + `image` crate に依存
- SVG のラスタライズは `resvg` crate を使用
- 外部コマンドプレビューの設定フォーマットは `001-user-config` feature の TOML/Lua 設定に依存
- プレビューモード切替のキーバインドは `001-user-config` feature のキーマップ設定に依存
