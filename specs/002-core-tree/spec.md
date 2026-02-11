# Feature Specification: コアツリーデータ構造

**Feature Branch**: `002-core-tree`
**Created**: 2026-02-10
**Status**: Draft
**Input**: User description: "コアツリーデータ構造 — TreeNode, Tree, ビルダー、ソート、遅延読み込み"

## User Scenarios & Testing

### User Story 1 - ファイルシステムからのツリー構築 (Priority: P1)

ユーザーが指定したディレクトリパスからファイルシステムを走査し、ツリーデータ構造を構築する。起動時はルート直下のみ読み込み（遅延読み込み）、ディレクトリ展開時に子ノードを非同期ロードする。`.gitignore` を尊重し、隠しファイルの表示/非表示を制御できる。

**Why this priority**: すべての UI 表示・ナビゲーションの基盤。ツリーがなければ何も表示できない。

**Independent Test**: tempdir にファイル構造を作成し、`TreeBuilder` で正しいツリーが得られることをユニットテストで検証。

**Acceptance Scenarios**:

1. **Given** ファイルとサブディレクトリを含むディレクトリ, **When** `TreeBuilder::build()` を呼ぶ, **Then** ルート直下のエントリを含む TreeNode ツリーが返り、サブディレクトリの children は `NotLoaded` 状態
2. **Given** `.gitignore` に `target/` が記載されている, **When** ビルドする, **Then** `target/` ディレクトリはツリーに含まれない
3. **Given** `show_hidden = false`, **When** ビルドする, **Then** `.` で始まるファイル/ディレクトリはツリーに含まれない
4. **Given** `show_hidden = true`, **When** ビルドする, **Then** `.` で始まるファイル/ディレクトリもツリーに含まれる
5. **Given** 存在しないパス, **When** ビルドする, **Then** 適切なエラーが返る
6. **Given** シンボリックリンクを含むディレクトリ, **When** ビルドする, **Then** シンボリックリンクが `is_symlink = true` として含まれる

---

### User Story 2 - 遅延読み込みとディレクトリ展開 (Priority: P1)

ディレクトリの展開時に、そのディレクトリの子ノードを読み込む。未読み込み (`NotLoaded`) → 読み込み中 (`Loading`) → 読み込み済み (`Loaded`) の状態遷移を管理する。UI は `Loading` 状態を表示できる。

**Why this priority**: 高速起動のために遅延読み込みは必須。大規模ディレクトリ (100k+ ファイル) での起動時間を数秒→数百ミリ秒に短縮する。

**Independent Test**: ディレクトリを展開し、子ノードが読み込まれて visible nodes に反映されることをユニットテストで検証。

**Acceptance Scenarios**:

1. **Given** `NotLoaded` 状態のディレクトリ, **When** 展開する, **Then** 状態が `Loading` に変わり、読み込み完了後 `Loaded` になる
2. **Given** `Loaded` 状態のディレクトリ, **When** 折り畳んで再度展開, **Then** 再読み込みは行わず即座に表示される
3. **Given** `Loading` 状態のディレクトリ, **When** 折り畳む, **Then** 読み込みは継続されるが visible nodes からは除外される
4. **Given** 読み込みエラー（パーミッション不足等）, **When** 展開する, **Then** エラーをユーザーに通知し、ディレクトリは折り畳み状態に戻る

---

### User Story 3 - ツリーのソート (Priority: P1)

ユーザーはツリーの表示順をソートキー（名前・サイズ・更新日時・種類・拡張子）、方向（昇順・降順）、ディレクトリ優先の有無で制御できる。ソートは各ディレクトリの children に再帰的に適用される。

**Why this priority**: ツリー構築と同等に基本的な表示機能。ユーザーの期待する順序でファイルが並ぶ必要がある。

**Independent Test**: 様々なファイルを持つツリーに対して各ソート設定を適用し、正しい順序になることをユニットテストで検証。

**Acceptance Scenarios**:

1. **Given** ファイルを含むツリー, **When** `SortOrder::Name` + `SortDirection::Asc` でソート, **Then** 名前の昇順（大文字小文字無視）で並ぶ
2. **Given** ファイルを含むツリー, **When** `SortOrder::Size` + `SortDirection::Desc` でソート, **Then** サイズの大きい順で並ぶ
3. **Given** ファイルとディレクトリが混在, **When** `directories_first = true` でソート, **Then** ディレクトリが先頭にまとまる
4. **Given** ネストしたディレクトリ, **When** ソートを適用, **Then** 各階層の children がすべてソートされている
5. **Given** `SortOrder::Modified` でソート, **When** `modified` が `None` のエントリがある, **Then** `None` は末尾に配置される

---

### User Story 4 - Visible Nodes の生成 (Priority: P1)

ツリーの展開/折り畳み状態に基づき、現在表示すべきノードをフラットなリスト（visible nodes）として効率的に取得できる。UI 描画はこの visible nodes を使用する。

**Why this priority**: UI 描画に必要不可欠。カーソル移動もこのフラットリスト上で行われる。

**Independent Test**: ツリーの展開/折り畳みを操作し、`visible_nodes()` の結果が正しいことをユニットテストで検証。

**Acceptance Scenarios**:

1. **Given** ルートのみ展開されたツリー, **When** `visible_nodes()` を呼ぶ, **Then** ルート直下のエントリのみ（depth=0）が返る
2. **Given** サブディレクトリを展開, **When** `visible_nodes()` を呼ぶ, **Then** そのサブディレクトリの children も含まれ適切な depth を持つ
3. **Given** 展開済みのディレクトリ, **When** 折り畳む, **Then** そのディレクトリ以下が visible nodes から消える
4. **Given** `NotLoaded` 状態のサブディレクトリ, **When** `visible_nodes()` を呼ぶ, **Then** そのディレクトリ自体は表示されるが children は含まれない
5. **Given** 10,000 エントリのツリー, **When** `visible_nodes()` を呼ぶ, **Then** 16ms 以内に完了する

---

### User Story 5 - カーソル管理 (Priority: P2)

ツリー内のカーソル位置を管理し、上下移動・先頭/末尾ジャンプ・半ページ移動を提供する。カーソルは visible nodes のインデックスとして管理される。

**Why this priority**: ナビゲーションの基盤だが、visible nodes が完成してから実装する方が自然。

**Independent Test**: カーソル操作を行い、正しい位置に移動することをユニットテストで検証。

**Acceptance Scenarios**:

1. **Given** カーソルが先頭, **When** 上に移動, **Then** カーソルは先頭のまま（バウンド処理）
2. **Given** カーソルが末尾, **When** 下に移動, **Then** カーソルは末尾のまま
3. **Given** 50 エントリ表示中, **When** 半ページ下（viewport_height=20）, **Then** カーソルが 10 行下に移動
4. **Given** カーソルがファイル上, **When** 展開操作, **Then** ファイルの「開く」アクションがトリガーされる
5. **Given** カーソルが折り畳みディレクトリ上, **When** 展開操作, **Then** ディレクトリが展開される（遅延読み込みが発動）
6. **Given** カーソルが展開ディレクトリ上, **When** 折り畳み操作, **Then** ディレクトリが折り畳まれる
7. **Given** カーソルがファイル上, **When** 折り畳み操作, **Then** 親ディレクトリにカーソルが移動する
8. **Given** ディレクトリの折り畳みでカーソル位置のノードが非表示になる, **When** 折り畳む, **Then** カーソルは折り畳んだディレクトリに移動する

---

### User Story 6 - バックグラウンド全ツリー走査（検索用）(Priority: P2)

検索機能のために、バックグラウンドで全ファイルシステムを走査し、全エントリの情報を収集する。UI 表示の遅延読み込みとは独立に動作する。走査結果は検索インデックスとして利用される。

**Why this priority**: 検索が遅延読み込み済みのノードにしか対応できないと、ユーザーは未展開のディレクトリ内のファイルを見つけられない。

**Independent Test**: バックグラウンド走査を開始し、全エントリがインデックスに含まれることをテストで検証。

**Acceptance Scenarios**:

1. **Given** 起動直後, **When** バックグラウンド走査が完了, **Then** 全ファイル/ディレクトリのパスと名前が検索インデックスに含まれる
2. **Given** 走査中, **When** ユーザーが検索を開始, **Then** その時点で走査済みのエントリのみが検索対象となる（部分結果を利用可能）
3. **Given** 走査完了後, **When** ユーザーが検索, **Then** 未展開のディレクトリ内のファイルも検索結果に含まれる
4. **Given** `.gitignore` 対象ファイル, **When** `show_ignored = false` で走査, **Then** 検索インデックスにも含まれない

---

### Edge Cases

- 空のディレクトリ → children が空の `Loaded(Vec::new())` として扱う
- パーミッションエラーで読めないディレクトリ → エラーをログ出力し、そのディレクトリをスキップ（ビルド全体は失敗させない）
- 循環シンボリックリンク → `ignore` crate が自動的に検出・スキップ
- ルートパスがファイル（ディレクトリではない）→ エラーを返す
- 非常に深いネスト（1000+レベル）→ `ignore::WalkBuilder` の depth 制限を設定可能に
- ソート中に `modified` が取得できないファイル → `None` として扱い、末尾に配置
- バックグラウンド走査中にディレクトリが削除される → エラーをスキップして走査を継続
- 展開と同時に同じディレクトリのバックグラウンド走査結果が到着 → バックグラウンド結果を遅延読み込み結果として統合

## Requirements

### Functional Requirements

- **FR-001**: `TreeNode` は name, path (絶対パス), is_dir, is_symlink, size, modified, children (ChildrenState), is_expanded を保持する
- **FR-002**: `ChildrenState` は `NotLoaded`, `Loading`, `Loaded(Vec<TreeNode>)` の 3 状態を持つ列挙型
- **FR-003**: `TreeBuilder` は `ignore::WalkBuilder` を使用し、`.gitignore` ルールを尊重する
- **FR-004**: `TreeBuilder` は `show_hidden` と `show_ignored` オプションで表示制御できる
- **FR-005**: 起動時のツリー構築はルート直下の 1 階層のみを読み込む（サブディレクトリの children は `NotLoaded`）
- **FR-006**: ディレクトリ展開時にその直下の 1 階層を非同期で読み込み、結果を TreeNode に反映する
- **FR-007**: ソートは `SortOrder` (Name, Size, Modified, Type, Extension), `SortDirection` (Asc, Desc), `directories_first` (bool) で制御
- **FR-008**: ソートは TreeNode の各階層の children に再帰的に適用
- **FR-009**: `Tree` は root, cursor, sort 設定を管理し、`visible_nodes()` で表示対象をフラット化して返す
- **FR-010**: `visible_nodes()` は展開済み且つ `Loaded` 状態のノードのみを走査し、各ノードの参照と depth を返す（clone なし）
- **FR-011**: カーソルは visible nodes のインデックスとして管理し、範囲外アクセスを防ぐバウンドチェックを持つ
- **FR-012**: バックグラウンド全ツリー走査は起動直後に開始し、全エントリのパスと名前を検索インデックスとして収集する
- **FR-013**: バックグラウンド走査は UI スレッドをブロックしない（tokio タスクとして実行）
- **FR-014**: バックグラウンド走査の結果は遅延読み込みのキャッシュとしても利用可能（展開時に走査済みなら IO を省略）

### Non-Functional Requirements

- **NFR-001**: `visible_nodes()` は 10,000 エントリのツリーで 16ms 以内に完了すること
- **NFR-002**: 1,000 ファイルのディレクトリでの初期ツリー構築（ルート直下のみ）は 100ms 以内に完了すること
- **NFR-003**: TreeNode は参照ベースのアクセスを基本とし、不要な clone を避けること
- **NFR-004**: `unsafe` コード禁止。すべてのエラーは `Result` で処理

### Key Entities

- **TreeNode**: ファイル/ディレクトリを表すノード。ChildrenState で遅延読み込みを管理
- **ChildrenState**: ディレクトリの子ノードの読み込み状態 (NotLoaded / Loading / Loaded)
- **Tree**: ツリー全体の状態管理。root, cursor, sort 設定を保持
- **VisibleNode**: UI 描画用のフラット化されたノード参照 (node: &TreeNode, depth: usize)
- **TreeBuilder**: ファイルシステムからツリーを構築するビルダー
- **SearchIndex**: バックグラウンド走査で構築される検索用インデックス（パス + 名前のリスト）
- **SortOrder / SortDirection**: ソート設定の列挙型

## Success Criteria

### Measurable Outcomes

- **SC-001**: `visible_nodes()` が 10,000 エントリツリーで 16ms 以内
- **SC-002**: 初期ツリー構築（ルート直下のみ）が 1,000 ファイルディレクトリで 100ms 以内
- **SC-003**: カーソル操作で panic / out-of-bounds が発生するケースがゼロ
- **SC-004**: `.gitignore` で指定されたファイルがツリーに含まれるケースがゼロ（`show_ignored = false` 時）
- **SC-005**: パーミッションエラーや不正なパスでアプリケーションがクラッシュするケースがゼロ
- **SC-006**: バックグラウンド走査が UI のフレームレート（60fps）に影響を与えないこと

## Assumptions

- Arena ベースのツリーは v1.0 スコープ外。再帰的 `Vec<TreeNode>` で開始し、測定に基づいて検討
- ファイル監視（fs_event による変更検知・自動リフレッシュ）は本 spec のスコープ外
- Git ステータスの統合は `009-git` feature で扱い、本 spec では TreeNode に git 関連フィールドを含まない
- 検索の UI・ファジーマッチングロジックは `007-search` feature で扱い、本 spec では SearchIndex の構築までを対象とする
