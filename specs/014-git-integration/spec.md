# Feature Specification: Git Integration

**Feature Branch**: `014-git-integration`
**Created**: 2026-02-18
**Status**: Draft
**Input**: User description: "git 連携機能の仕様を策定"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - ファイルの Git ステータス表示 (Priority: P1)

ユーザーがツリービューでファイルを閲覧しているとき、各ファイルの Git ステータス（変更、新規、未追跡など）がファイル名の右側に色付き1文字インジケーターとして表示される。この領域は将来ファイルサイズや最終変更日なども並べて表示する予定のメタデータ領域である。

**インジケーター表示位置**:

ファイル名の右側、メタデータ領域に表示する。既存のセレクションマーカー（`●`/`◆`/`◇`）とは独立した位置のため、セレクション操作中でも Git ステータスは同時に表示される。

```
[selection][indent][dir_indicator][icon] name    [git_status]
```

表示例:
```
   src/                M   ← 子孫の集約ステータス (黄色の M)
     main.rs           M   ← unstaged modified (黄色の M)
     config.rs         A   ← staged added (緑の A)
     draft.txt         ?   ← untracked (マゼンタの ?)
     utils.rs              ← clean (表示なし)
```

**インジケーター定義**:

| ステータス | 文字 | 色 | 説明 |
|-----------|------|-----|------|
| Modified (unstaged) | `M` | Yellow | ワーキングツリーで変更あり |
| Modified (staged) | `M` | Green | インデックスにステージ済み |
| Added (staged) | `A` | Green | 新規ファイルをステージ済み |
| Deleted | `D` | Red | 削除済み（ディレクトリ集約のみ。個別ファイルはツリーに非表示） |
| Renamed | `R` | Blue | リネーム済み |
| Untracked | `?` | Magenta | Git 未追跡 |
| Conflicted | `!` | Red+Bold | マージコンフリクト |
| Clean | (空白) | — | インジケーターなし |

- Staged と Unstaged の両方がある場合（例: 一部ステージ済み）は、Unstaged 側を優先表示する
- Git リポジトリ外ではインジケーターカラムを表示しない

**Why this priority**: ファイルエクスプローラーにおける Git 連携の最も基本的かつ高頻度で利用される機能

**Independent Test**: Git リポジトリ内でファイルを変更し、ツリービューでステータスインジケーターが正しく表示されることを確認

**Acceptance Scenarios**:

1. **Given** Git リポジトリ内で `file.rs` を編集して未コミット状態, **When** ツリービューを表示, **Then** `file.rs` のファイル名右側に黄色の `M` が表示される
2. **Given** `git add file.rs` でステージ済み, **When** ツリービューを表示, **Then** `file.rs` のファイル名右側に緑色の `M` が表示される
3. **Given** 新規ファイル `new.rs` を作成し未追跡, **When** ツリービューを表示, **Then** `new.rs` のファイル名右側にマゼンタの `?` が表示される
4. **Given** Modified なファイルを mark 選択中, **When** ツリービューを表示, **Then** セレクションマーカー `●` と Git ステータス `M` が同時に表示される
5. **Given** Git リポジトリ外のディレクトリを開いている, **When** ツリービューを表示, **Then** ステータスインジケーターは表示されない

---

### User Story 2 - ディレクトリの Git ステータス集約表示 (Priority: P1)

ディレクトリには、展開/折りたたみ状態に関わらず、その子孫ファイルのうち最も優先度の高い Git ステータスが常に集約表示される。これにより、折りたたまれたディレクトリでも展開中のディレクトリでも内部の変更状況が把握できる。

**ステータス優先度（高→低）**: Conflicted > Modified > Added > Deleted > Renamed > Untracked > Clean

**Why this priority**: ディレクトリを折りたたんだ状態でも変更の所在を素早く把握するために必要

**Independent Test**: 子ファイルに変更があるディレクトリのインジケーターが正しく集約されることを確認

**Acceptance Scenarios**:

1. **Given** `src/` 配下に Modified なファイルが1つある, **When** `src/` が折りたたまれている, **Then** `src/` のディレクトリ名右側に黄色の `M` が表示される
2. **Given** `src/` 配下に Modified と Untracked のファイルがある, **When** `src/` を表示, **Then** 優先度の高い Modified（黄色 `M`）がディレクトリ名右側に表示される
3. **Given** `src/` 配下の全ファイルが Clean, **When** `src/` を表示, **Then** ステータスインジケーターは表示されない

---

### User Story 3 - Git ステータスの自動更新 (Priority: P1)

ファイルシステムの変更を検知したとき（既存の watcher 機能と連動）、Git ステータスも再取得して表示を更新する。ユーザーが別ターミナルで `git add` や `git commit` を行った場合にも反映される。

**Why this priority**: ステータスが古いまま表示されると誤解を招くため、自動更新は必須

**Independent Test**: 別ターミナルで `git add` を実行し、ツリービューのステータスが自動更新されることを確認

**Acceptance Scenarios**:

1. **Given** ツリービューを表示中, **When** 別ターミナルで `git add file.rs` を実行, **Then** `file.rs` のステータスが Unstaged Modified → Staged Modified に自動更新される
2. **Given** ツリービューを表示中, **When** 別ターミナルで新規ファイルを作成, **Then** ツリーが更新され Untracked インジケーターが表示される

---

### User Story 4 - ツリーの手動リフレッシュ (Priority: P1)

ユーザーが `R` キーを押すことで、ツリーの状態（ファイルシステムと Git ステータスの両方）を手動で再読み込みできる。自動更新が間に合わない場合のフォールバックとして機能する。

**Why this priority**: 自動更新だけに頼れないケース（ネットワークファイルシステム等）への対応として必要

**Independent Test**: `R` キーを押してツリーとステータスが最新状態に更新されることを確認

**Acceptance Scenarios**:

1. **Given** ツリービューを表示中, **When** `R` キーを押す, **Then** ツリー構造と Git ステータスが最新の状態に再読み込みされる
2. **Given** Git リポジトリ外, **When** `R` キーを押す, **Then** ツリー構造のみが再読み込みされる（エラーにならない）

---

### User Story 5 - カスタムプレビューの git_status 条件 (Priority: P2)

外部コマンドプレビュー（`ExternalCommand`）の設定に `git_status` 条件を追加できる。特定の Git ステータスのファイルにのみ適用されるプレビューコマンドを設定可能にする。

**設定例**:

```yaml
preview:
  commands:
    - name: "Git Diff"
      command: git
      args: ["diff", "--color=always", "{path}"]
      git_status: [modified]
      priority: high
    - name: "Git Diff Staged"
      command: git
      args: ["diff", "--staged", "--color=always", "{path}"]
      git_status: [staged]
      priority: high
```

**git_status で使用可能な値**: `modified`, `staged`, `added`, `deleted`, `renamed`, `untracked`, `conflicted`

**Why this priority**: ユーザーが Git diff をプレビューとして見られるようになり、専用プロバイダーを内蔵するよりも柔軟性が高い

**Independent Test**: `git_status: [modified]` 条件を設定し、変更ファイルでのみコマンドが実行されることを確認

**Acceptance Scenarios**:

1. **Given** `git_status: [modified]` のプレビューコマンドを設定, **When** Modified なファイルにカーソルを合わせる, **Then** そのコマンドがプレビューに使用される
2. **Given** `git_status: [modified]` のプレビューコマンドを設定, **When** Clean なファイルにカーソルを合わせる, **Then** そのコマンドは使用されず通常のプレビューが表示される
3. **Given** `git_status` 条件を設定していないコマンド, **When** 任意のファイルにカーソルを合わせる, **Then** 従来通り extensions 等の条件のみで判定される

---

### User Story 6 - Git 連携設定 (Priority: P2)

ユーザーが Git 連携機能全体の有効/無効を設定で切り替えられる。デフォルトは有効。無効にすると Git ステータスの取得・表示が一切行われない。

**設定例**:

```yaml
git:
  enabled: true
```

**Why this priority**: パフォーマンスへの影響を懸念するユーザーや、Git 非関連のワークフローで使用するユーザー向けの設定

**Independent Test**: `git.enabled: false` を設定し、ステータスインジケーターが表示されないことを確認

**Acceptance Scenarios**:

1. **Given** `git.enabled: true`（デフォルト）, **When** Git リポジトリ内で trev を起動, **Then** ステータスインジケーターが表示される
2. **Given** `git.enabled: false`, **When** Git リポジトリ内で trev を起動, **Then** ステータスインジケーターが表示されない

---

### Edge Cases

- Git リポジトリのルートがツリーのルートと異なる場合（サブディレクトリから起動した場合）はどうなるか？ → 正しく動作する。`git status` はリポジトリルートからの相対パスで処理される
- `.git` ディレクトリ自体はツリーに表示されるか？ → 既存の hidden files 設定に従う（デフォルトでは非表示）
- ベアリポジトリの場合はどうなるか？ → Git 連携を無効化し、通常のファイルツリーとして表示する
- 非常に大きなリポジトリ（10万ファイル以上）でのパフォーマンスは？ → バックグラウンドで非同期に Git ステータスを取得し、UI をブロックしない。取得完了まではインジケーターなしで表示する
- submodule を含むリポジトリの場合は？ → submodule のルートディレクトリのステータスのみ表示する（submodule 内部は再帰的に取得しない）
- `git_status` 条件とその他の条件（`extensions` 等）が同時に設定された場合は？ → AND 条件として扱う。両方を満たす場合のみコマンドが適用される
- 削除済みファイル（`git rm` 等）はツリーに表示されるか？ → 表示されない。ツリーはファイルシステムベースのため、存在しないファイルは表示しない。ただし親ディレクトリの集約ステータスには `D` として反映される

## Clarifications

### Session 2026-02-18

- Q: 削除済みファイル（`D` ステータス）のツリー表示はどうすべきか？ → A: ディレクトリ集約のみ。削除ファイルはツリーに表示しないが、親ディレクトリの集約ステータスには `D` を反映する。
- Q: 展開中のディレクトリにも集約ステータスを表示するか？ → A: 常に表示。展開/折りたたみに関わらずディレクトリには集約ステータスを表示する。
- Q: ステータスバーに Git ブランチ名を表示するか？ → A: 表示しない。ブランチ表示はこのツールの役割ではない。User Story 5 を削除。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: システムは Git リポジトリ内のファイルの Git ステータス（Modified, Added, Deleted, Renamed, Untracked, Conflicted, Staged）を取得し、ツリービューのファイル名右側のメタデータ領域に色付き1文字インジケーターとして表示しなければならない
- **FR-002**: Git ステータスインジケーターはセレクションマーカーと独立した位置に表示され、セレクション操作中でも同時に表示されなければならない
- **FR-003**: システムはディレクトリのステータスを子孫のうち最も優先度の高いステータスとして集約表示しなければならない
- **FR-004**: システムはファイルシステム変更検知時に Git ステータスを自動的に再取得しなければならない
- **FR-005**: `R` キーでツリーの状態（ファイルシステムと Git ステータス）を手動で再読み込みできなければならない
- **FR-006**: 外部コマンドプレビューに `git_status` 条件を追加し、特定のステータスを持つファイルにのみコマンドを適用できなければならない
- **FR-007**: `git.enabled` 設定で Git 連携機能全体の有効/無効を切り替えられなければならない（デフォルト: `true`）
- **FR-008**: Git ステータスの取得は非同期でバックグラウンドに行い、UI の応答性をブロックしてはならない
- **FR-009**: Git リポジトリ外ではステータスインジケーターを省略し、エラーを出さない

### Key Entities

- **GitStatus**: ファイルまたはディレクトリの Git ステータス。Modified / Added / Deleted / Renamed / Untracked / Conflicted / Staged / Clean の状態を持つ
- **GitState**: リポジトリ全体の Git 状態。ファイルパス-ステータスのマッピングを保持する
- **GitConfig**: Git 連携の設定。`enabled` フラグを含む

### Assumptions

- Nerd Font の有無に関わらず ASCII 文字（M, A, D, R, ?, !）を使用するため、特殊フォントは不要
- Git ステータスの取得は `git status --porcelain=v1` 相当の出力をパースする
- Staged と Unstaged が同時に存在する場合は Unstaged 側を優先表示する（ユーザーがまだ作業中であることを示す）

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: ユーザーが任意のファイルの Git ステータスをツリービューで即座に確認できる
- **SC-002**: 1万ファイル以下のリポジトリで Git ステータス取得が 500ms 以内に完了する
- **SC-003**: Git ステータスの自動更新が FS 変更検知から 1 秒以内に反映される
- **SC-004**: `R` キーによる手動リフレッシュが 1 秒以内にツリーを更新する
- **SC-005**: `git.enabled: false` 設定時、Git 関連の処理が一切実行されず、起動時間に影響を与えない
- **SC-006**: Git リポジトリ外での使用で、エラーや警告が表示されない
