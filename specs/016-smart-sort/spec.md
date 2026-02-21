# Feature Specification: Smart Sort

**Feature Branch**: `016-smart-sort`
**Created**: 2026-02-22
**Status**: Draft
**Input**: User description: "ソート機能を整える。S キーでソートを選択するメニューを表示する。smart ソートを実装する。数字や大文字小文字をいい感じに扱う。xxx_test.go や xxx.test.ts, xxx.stories.ts などがいい感じに並ぶようにする。デフォルトは smart ソート。"

## Clarifications

### Session 2026-02-22

- Q: 3つ以上のドット区切りファイル（例: `foo.bar.test.ts`）の分解方法は？ → A: 最後の1セグメントを接尾辞とする（ベース `foo.bar`, 接尾辞 `.test.ts`）

## User Scenarios & Testing

### User Story 1 - Smart Sort as Default (Priority: P1)

ユーザーがディレクトリを開くと、ファイルが自然な順序で表示される。数字を含むファイル名は数値順に並び（`file2.txt` < `file10.txt`）、大文字小文字は区別せずに並ぶ。テストファイルやストーリーファイルは対応する実装ファイルの直後に表示される（例: `user.ts` → `user.test.ts` → `user.stories.ts`）。

**Why this priority**: ファイルツリーの見やすさはユーザー体験の根幹であり、smart sort がデフォルトとして機能することが最も重要。

**Independent Test**: 任意のディレクトリを開き、ファイルが自然な順序で表示されることを目視確認できる。数字付きファイル名、テストファイル、大文字小文字混在ファイルが正しく並ぶ。

**Acceptance Scenarios**:

1. **Given** `file1.txt`, `file10.txt`, `file2.txt` があるディレクトリ, **When** smart sort で表示, **Then** `file1.txt`, `file2.txt`, `file10.txt` の順に並ぶ
2. **Given** `README.md`, `api.ts`, `Api.ts` があるディレクトリ, **When** smart sort で表示, **Then** 大文字小文字を区別せず、安定した順序で並ぶ（例: `Api.ts`, `api.ts`, `README.md`）
3. **Given** `user.ts`, `user.test.ts`, `user.stories.ts`, `admin.ts` があるディレクトリ, **When** smart sort で表示, **Then** `admin.ts`, `user.ts`, `user.stories.ts`, `user.test.ts` の順に並ぶ（接尾辞なしが先頭、接尾辞ありはアルファベット順）
4. **Given** `handler.go`, `handler_test.go`, `server.go`, `server_test.go` があるディレクトリ, **When** smart sort で表示, **Then** `handler.go`, `handler_test.go`, `server.go`, `server_test.go` の順に並ぶ
5. **Given** `index.tsx`, `index.test.tsx`, `index.stories.tsx`, `index.module.css` があるディレクトリ, **When** smart sort で表示, **Then** `index.tsx`, `index.module.css`, `index.stories.tsx`, `index.test.tsx` の順に並ぶ（接尾辞なしが先頭、接尾辞ありはアルファベット順）

---

### User Story 2 - Sort Selection Menu (Priority: P2)

ユーザーが `S` キーを押すと、ソート方式を選択するメニューが表示される。メニューから smart, name, size, mtime, type, extension を選択でき、即座にツリー表示が更新される。

**Why this priority**: ユーザーが状況に応じてソート方式を切り替えられることは利便性向上に重要だが、デフォルトの smart sort が正しく機能していることが前提。

**Independent Test**: `S` キーを押してメニューが表示され、選択後にソート順が変わることを確認できる。

**Acceptance Scenarios**:

1. **Given** 通常モードでツリー表示中, **When** `S` キーを押す, **Then** ソート方式の選択メニューが表示される
2. **Given** ソートメニューが表示されている, **When** smart を選択, **Then** メニューが閉じ、smart sort でツリーが再描画される
3. **Given** ソートメニューが表示されている, **When** `Esc` を押す, **Then** メニューが閉じ、ソート方式は変わらない
4. **Given** ソートメニューが表示されている, **When** 現在のソートと異なる方式を選択, **Then** ツリーがその方式で即座に再ソートされる
5. **Given** ソートメニューが表示されている, **Then** 現在選択中のソート方式が視覚的にハイライトされている

---

### User Story 3 - Default Sort Order Change (Priority: P3)

デフォルトのソート方式が name から smart に変更される。設定ファイルで `sort.order: smart` を指定でき、CLI オプション `--sort-order smart` でも利用できる。既存の name, size, mtime, type, extension もそのまま利用可能。

**Why this priority**: 設定とCLIの対応は必要だが、ユーザーが明示的に設定変更しなくてもデフォルトで smart が使われるため、優先度は低い。

**Independent Test**: 設定未変更で起動し smart sort がデフォルトであること、`sort.order: name` と指定して従来の名前順ソートになることを確認できる。

**Acceptance Scenarios**:

1. **Given** 設定ファイルなし, **When** trev を起動, **Then** smart sort がデフォルトで適用される
2. **Given** 設定に `sort.order: name`, **When** trev を起動, **Then** 従来の名前順ソートが適用される
3. **Given** CLI で `--sort-order smart`, **When** trev を起動, **Then** smart sort が適用される

---

### Edge Cases

- 数字のみのファイル名（`1`, `2`, `10`）が正しく数値順に並ぶか？
- 非常に長い数字列（`file99999999999999999.txt`）がオーバーフローせず正しくソートされるか？
- Unicode ファイル名（日本語、絵文字）が smart sort でクラッシュせずソートされるか？
- 拡張子なしファイルの test/stories グルーピングはどうなるか？（例: `Makefile` は単独で並ぶ）
- `.test.ts` や `_test.go` のような接尾辞パターンに該当しない通常ファイル（例: `test_utils.ts`）が誤ってグルーピングされないか？
- 空ディレクトリやファイルが1つだけのディレクトリでソートメニューが正常に動作するか？
- ソートメニュー表示中に他のキー（`j`, `k` 以外）を押した場合の挙動

## Requirements

### Functional Requirements

- **FR-001**: システムは smart sort を提供しなければならない。smart sort は以下の規則で並べる:
  1. ディレクトリ先頭（`directories_first` 有効時）
  2. ファイル名を「ベース名」と「接尾辞パターン」に分解する（例: `user.test.ts` → ベース `user`, 接尾辞 `.test.ts`）
  3. ベース名が同じファイルをグループ化し、実装ファイル → テスト/ストーリー等の順に並べる
  4. 数字部分は数値として比較する（natural sort）
  5. 大文字小文字は区別しない（tie-break で原文順を使う）

- **FR-002**: 接尾辞パターンの認識規則:
  - ドット区切り: `xxx.yyy.{ext}` 形式のファイル名は、ベース `xxx` + 接尾辞 `.yyy.{ext}` に分解する（`yyy` の内容は限定しない）。3つ以上のドットがある場合は最後の1セグメント（拡張子の直前）を接尾辞とする
    - 例: `user.test.ts` → ベース `user`, 接尾辞 `.test.ts`
    - 例: `app.config.ts` → ベース `app`, 接尾辞 `.config.ts`
    - 例: `index.module.css` → ベース `index`, 接尾辞 `.module.css`
    - 例: `foo.bar.test.ts` → ベース `foo.bar`, 接尾辞 `.test.ts`
  - アンダースコア区切り: `xxx_test.{ext}` および `xxx_spec.{ext}` パターンをベース `xxx` + 接尾辞として認識する（Go, Elixir, Dart, Ruby 等で広く使われるパターン）
  - 接尾辞なしファイル（`xxx.{ext}`）はグループ内で先頭に並び、接尾辞ありファイルはアルファベット順で続く

- **FR-003**: `S` キーでソート選択メニューを表示しなければならない。メニュー項目は: smart, name, size, mtime, type, extension

- **FR-004**: ソートメニューは `j`/`k` または上下キーで項目を移動、`Enter` で選択、`Esc` でキャンセルできなければならない

- **FR-005**: 現在のソート方式がメニュー上で視覚的に区別されなければならない

- **FR-006**: デフォルトのソート方式を smart に変更しなければならない

- **FR-007**: 設定ファイルで `sort.order: smart` を指定でき、CLI オプション `--sort-order smart` でも利用できなければならない

- **FR-008**: 既存のソート方式（name, size, mtime, type, extension）は引き続き動作しなければならない

- **FR-009**: 現在の `S` キー（cycle sort order）の動作をソートメニュー表示に置き換えなければならない

### Key Entities

- **Sort Order**: ファイルの並び順を決定する方式。smart, name, size, mtime, type, extension のいずれか
- **Suffix Pattern**: ファイル名の接尾辞パターン。テスト、ストーリー等の種別を識別し、関連ファイルのグルーピングに使用する
- **Sort Menu**: ソート方式を選択するための一時的なオーバーレイUI

## Success Criteria

### Measurable Outcomes

- **SC-001**: 100,000 ファイルのフラットディレクトリでも smart sort が 1 秒以内に完了する
- **SC-002**: `S` キーを押してからメニュー表示まで体感遅延がない（16ms 以内のフレーム描画）
- **SC-003**: 既存の全ソート方式が smart sort 導入前と同一の結果を返す（回帰なし）
- **SC-004**: テストファイルの 95% 以上が対応する実装ファイルの直後に表示される（一般的なプロジェクト構造において）

## Assumptions

- ドットファイル（`.gitignore` 等）は通常ファイルと同じ smart sort 規則で並ぶ（特別扱いしない）
- グループ内の順序は: 接尾辞なし（実装ファイル）が先頭、接尾辞ありはアルファベット順
- ソートメニューは既存のモーダル UI パターン（rename, search 等）と同様のスタイルで表示する
- `s` キー（toggle sort direction）は従来通り維持する
- natural sort の数値比較は桁数に上限を設けず、任意精度で比較する
