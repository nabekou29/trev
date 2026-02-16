# Feature Specification: Neovim IPC 連携

**Feature Branch**: `013-neovim-ipc`
**Created**: 2026-02-16
**Status**: Draft
**Input**: User description: "Neovim IPC integration: Unix Domain Socket server/client in Rust daemon mode, Neovim Lua plugin with side panel (toggleterm or snacks), float picker (--emit), reveal (BufEnter auto-sync), ctl subcommands (reveal/ping/quit), and editor command file for opening files in Neovim (edit/split/vsplit/tabedit)"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - サイドパネルファイルブラウザ (Priority: P1)

Neovim ユーザーがエディタの横に trev を常駐サイドパネルとして開く。プロジェクトツリーを閲覧し、ファイルを選択すると Neovim のバッファで開かれる。サイドパネルは開いたままで、ファイルシステムの変更をリアルタイムに反映する。

**Why this priority**: コア機能 — Neovim ワークフローに組み込まれた常駐ファイルブラウザ。neo-tree/nvim-tree を trev のプレビューやファイル操作で置き換える。

**Independent Test**: Neovim で `:TrevToggle` を実行し、ファイルを選択する。ファイルがエディタスプリットで開き、サイドパネルは表示されたまま。

**Acceptance Scenarios**:

1. **Given** trev プラグインがインストールされた Neovim が開いている, **When** `:TrevToggle` を実行, **Then** 左側にサイドパネルが開き、daemon モードの trev でプロジェクトツリーが表示される。
2. **Given** trev サイドパネルが開いている, **When** ファイルに移動して Enter を押す, **Then** 隣接する Neovim ウィンドウでファイルが開かれる（設定されたアクション: edit/split/vsplit/tabedit）。
3. **Given** trev サイドパネルが開いている, **When** 再度 `:TrevToggle` を実行, **Then** サイドパネルが閉じる。
4. **Given** trev サイドパネルが開いている, **When** `:TrevClose` を実行, **Then** サイドパネルと daemon プロセスが終了する。

---

### User Story 2 - 現在のファイルを Reveal (Priority: P1)

Neovim で作業中、ユーザーが現在のファイルがプロジェクトツリーのどこにあるかを確認したい。サイドパネルがアクティブなバッファのファイルを自動的にハイライトするか、ユーザーが手動で reveal をトリガーする。

**Why this priority**: エディタとツリー間のファイル同期はサイドパネルの実用性に不可欠。これがないとツリーとエディタが切り離される。

**Independent Test**: Neovim でファイルを開き `:TrevReveal` を実行 — サイドパネルがそのファイルまでスクロールしてハイライトする。

**Acceptance Scenarios**:

1. **Given** trev サイドパネルが開いている, **When** `:TrevReveal` を実行, **Then** trev が現在のバッファのファイルをツリーで探してハイライトする。
2. **Given** trev サイドパネルが開いていて `auto_reveal` が有効, **When** 別のバッファに切り替える, **Then** trev が自動的に新しいファイルをツリーで reveal する。
3. **Given** trev サイドパネルが開いている, **When** `:TrevReveal /path/to/file.rs` を実行, **Then** 指定されたパスのファイルを reveal する。
4. **Given** ファイルが折りたたまれたディレクトリ内にある, **When** reveal がトリガーされる, **Then** 必要に応じて親ディレクトリを展開してファイルを表示する。

---

### User Story 3 - フロートピッカー (Priority: P2)

ユーザーが常駐サイドパネルなしで素早くファイルを選択したい。フローティングウィンドウで trev を開き、閲覧/検索してファイルを選択すると Neovim で開かれる。選択後にフローティングウィンドウは自動的に閉じる。

**Why this priority**: サイドパネルの軽量な代替手段 — ウィンドウレイアウトを変更せずに素早くファイルにアクセスできる。

**Independent Test**: `:TrevToggle float` を実行 — フローティングウィンドウが表示され、ファイルを選択するとウィンドウが閉じてファイルが開く。

**Acceptance Scenarios**:

1. **Given** Neovim が開いている, **When** `:TrevToggle float` を実行, **Then** `--emit` モードの trev で中央にフローティングウィンドウが開く。
2. **Given** フロートピッカーが開いている, **When** ファイルを選択して Enter を押す, **Then** フローティングウィンドウが閉じ、設定されたアクションでファイルが Neovim で開かれる。
3. **Given** フロートピッカーが開いている, **When** `q` または `Esc Esc` を押す, **Then** ファイルを開かずにフローティングウィンドウが閉じる。
4. **Given** `:TrevToggle float vsplit` を実行, **When** ファイルを選択, **Then** ファイルが垂直分割で開かれる。

---

### User Story 4 - CLI Daemon 制御 (Priority: P3)

ユーザーまたはスクリプトが `trev ctl` サブコマンドで実行中の trev daemon をコマンドラインから制御する。スクリプティングや外部ツール連携を可能にする。

**Why this priority**: Neovim プラグインと外部スクリプトの両方が依存する基盤。直接ユーザー向けではないため優先度は低め。

**Independent Test**: 一つのターミナルで `trev --daemon` を実行し、別のターミナルで `trev ctl ping` を実行 — 成功レスポンスを受け取る。

**Acceptance Scenarios**:

1. **Given** trev が daemon モードで実行中, **When** `trev ctl ping` を実行, **Then** 成功レスポンスが返される。
2. **Given** trev が daemon モードで実行中, **When** `trev ctl reveal /path/to/file` を実行, **Then** trev が指定されたファイルに移動してハイライトする。
3. **Given** trev が daemon モードで実行中, **When** `trev ctl quit` を実行, **Then** trev がグレースフルにシャットダウンする。
4. **Given** daemon が実行されていない, **When** `trev ctl ping` を実行, **Then** daemon が利用不可であることを示すエラーメッセージが表示される。
5. **Given** 複数の daemon が実行中, **When** `trev socket-path` を実行, **Then** 全ソケットパスが一覧表示される。

---

### Edge Cases

- daemon のソケットファイルがクラッシュしたプロセスから残っている場合は？ → daemon は起動時に古いソケットファイルを削除する。
- 同じワークスペースに2つの daemon が起動された場合は？ → PID ベースのソケットパスにより共存可能。`trev ctl` は glob で発見する。
- Neovim プラグインがツリーに存在しないファイルを reveal しようとした場合は？ → reveal コマンドがサイレントに失敗（warning でログ）し、ツリー状態は変更されない。
- サイドパネルのターミナルプロセスが外部から kill された場合は？ → プラグインが終了を検出し、状態をクリーンアップする（パネルを閉じた状態にマーク、JSON-RPC 接続を切断）。
- Git リポジトリのないディレクトリで Neovim を開いた場合は？ → ワークスペースキーがディレクトリ名にフォールバックする。
- Neovim プラグインが接続する前にファイルが選択された場合は？ → 通知先がないため破棄される（ログ出力）。ユーザーが再選択すれば良い。

## Requirements *(mandatory)*

### Functional Requirements

#### IPC サーバー (Daemon モード)

- **FR-001**: `--daemon` フラグで起動した場合、trev は Unix Domain Socket でコマンドをリッスンしなければならない。
- **FR-002**: ソケットパスは `$XDG_RUNTIME_DIR/trev/<workspace_key>-<pid>.sock`（`$TMPDIR` または `/tmp` にフォールバック）の規約に従い、プロセスごとに一意でなければならない。
- **FR-003**: ワークスペースキーは Git リポジトリのルートディレクトリ名から導出し、Git がない場合は作業ディレクトリ名にフォールバックしなければならない。
- **FR-004**: daemon は起動時に古いソケットファイルを削除しなければならない。
- **FR-005**: daemon は同時接続するクライアントを処理しなければならない。

#### IPC クライアント (CLI 制御)

- **FR-006**: `trev ctl reveal <path>` は daemon に reveal コマンドを送信し、指定されたファイルに移動してハイライトしなければならない。
- **FR-007**: `trev ctl ping` は daemon の可用性を確認し、成功/失敗を返さなければならない。
- **FR-008**: `trev ctl quit` は daemon のグレースフルシャットダウンを要求しなければならない。
- **FR-009**: 各 ctl サブコマンドは特定の daemon インスタンスをターゲットするためのオプション `--workspace` フラグを受け付けなければならない。
- **FR-022**: `trev socket-path` サブコマンドは、ランタイムディレクトリ内の既存ソケットファイルを一覧表示しなければならない。`--workspace` で絞り込みが可能でなければならない。

#### エディタ連携 (Daemon → Neovim)

- **FR-010**: daemon モードでファイルが選択された場合（Enter キー）、trev は接続中のクライアントに JSON-RPC notification でファイルオープン通知を送信しなければならない。
- **FR-011**: 通知にはアクション（edit/split/vsplit/tabedit）と絶対ファイルパスを含めなければならない。
- **FR-012**: デフォルトアクションは `--action` CLI フラグで設定可能でなければならない。
- **FR-019**: `config.toml` の `[external_commands]` セクションでキーバインドとコマンド名のマッピングを定義できなければならない。
- **FR-020**: external_command キーが押された場合、trev は接続中のクライアントに JSON-RPC notification でコマンド名を送信しなければならない。
- **FR-021**: daemon は接続中のクライアントの状態取得リクエスト（`get_state`）に応答しなければならない。

#### Neovim プラグイン

- **FR-013**: プラグインは設定オプション（trev パス、幅、auto-reveal、フロートサイズ）を受け付ける `setup()` 関数を提供しなければならない。
- **FR-014**: プラグインは `:TrevToggle`、`:TrevOpen`、`:TrevClose`、`:TrevReveal` ユーザーコマンドを提供しなければならない。
- **FR-015**: サイドパネルモードは toggleterm.nvim または snacks.nvim（利用可能な方）でターミナルウィンドウを管理しなければならない。
- **FR-016**: フロートピッカーモードは `--emit` 付きの trev を中央のフローティングウィンドウで起動し、終了時に選択されたファイルを Neovim で開かなければならない。
- **FR-017**: プラグインは JSON-RPC 接続を通じて trev からの通知（`open_file`, `external_command`）を受信し、対応するアクション（ファイルを開く、カスタムハンドラー呼び出し）を実行しなければならない。
- **FR-018**: `auto_reveal` が有効な場合、プラグインは `BufEnter` イベントで現在のバッファのファイルを trev で reveal しなければならない（特殊バッファとターミナルバッファはスキップ）。

### Key Entities

- **IPC Request**: クライアントから daemon に送信されるコマンド（reveal, ping, quit）。Unix Domain Socket 上で JSON としてシリアライズされる。
- **IPC Response**: daemon からクライアントへの成功/エラー応答。ステータスとオプションのエラーメッセージを含む。
- **Editor Notification**: daemon から Neovim プラグインへの JSON-RPC notification。`open_file`（アクション種別+ファイルパス）と `external_command`（コマンド名）を含む。
- **Workspace Key**: Git ルートまたは作業ディレクトリ名から導出される文字列識別子。ソケットの名前空間に使用される。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: ユーザーが trev サイドパネルを開き、ファイルを選択して Enter を押してから1秒以内に Neovim でファイルが開かれること。
- **SC-002**: reveal コマンドが500ms以内にツリー内の正しいファイルに移動すること。
- **SC-003**: daemon 起動（ソケット作成を含む）が1秒以内に完了すること。
- **SC-004**: CLI 制御コマンド（`ping`, `reveal`, `quit`）が200ms以内に完了すること。
- **SC-005**: フロートピッカーが1秒以内に起動して操作可能になること。
- **SC-006**: バッファ切り替え時の auto-reveal が知覚できない遅延（200ms未満）で動作すること。

## Assumptions

- この機能は Unix 系システムのみを対象とする（macOS, Linux）。Windows は非対応（Unix Domain Socket）。
- ユーザーは Nerd Fonts をインストールしている（既存の trev アイコンサポートと一貫）。
- `--daemon` フラグは CLI 引数に定義済みだが、まだ機能していない。
- `ctl` サブコマンドは CLI 引数に定義済みだが、まだ機能していない。
- daemon ↔ エディタ間の通信には JSON-RPC 2.0 over Unix Domain Socket の双方向永続接続を採用する。TUI がサーバー、Neovim プラグインがクライアント。
- toggleterm.nvim または snacks.nvim がユーザーの Neovim 設定で利用可能であること。どちらもない場合はプラグインがエラーを通知する。
