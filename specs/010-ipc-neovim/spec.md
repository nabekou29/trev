# Feature Specification: IPC / Neovim 統合

**Feature Branch**: `010-ipc-neovim`
**Created**: 2026-02-10
**Status**: Draft
**Input**: User description: "IPC / Neovim 統合 — 双方向ソケット通信、reveal、daemon モード"

## User Scenarios & Testing

### User Story 1 - Daemon モードと IPC サーバー (Priority: P1)

ユーザーが `--daemon` フラグ付きで trev を起動すると、Unix Domain Socket 上で IPC サーバーが起動する。外部クライアント（Neovim プラグイン等）はこのソケットに接続して trev を制御できる。

**Why this priority**: Neovim 統合の基盤。IPC サーバーがなければ外部からの制御ができない。

**Independent Test**: `--daemon` で起動し、`trev ctl ping` で応答を確認できる。

**Acceptance Scenarios**:

1. **Given** `trev --daemon` で起動, **When** ソケットパスを確認, **Then** `$XDG_RUNTIME_DIR/trev/<workspace_key>.sock` にソケットファイルが存在する
2. **Given** daemon モードで起動中, **When** `trev ctl ping` を実行, **Then** `{"ok": true}` が返る
3. **Given** daemon モードで起動中, **When** `trev ctl quit` を実行, **Then** trev が正常終了する
4. **Given** `--daemon` なしで起動, **When** IPC ソケットを確認, **Then** ソケットは作成されない
5. **Given** 同じ workspace_key で 2 つ目の daemon を起動しようとする, **When** 起動, **Then** 既存のソケットを検出し、エラーメッセージを表示する
6. **Given** 前回の trev がクラッシュしてソケットファイルが残っている, **When** 新しい daemon を起動, **Then** stale なソケットを検出・削除して正常に起動する

---

### User Story 2 - 双方向 JSON-RPC 通信 (Priority: P1)

trev IPC サーバーはクライアントからのリクエストに応答（Request/Response）するだけでなく、trev からクライアントへの通知（Notification）も送信できる双方向通信をサポートする。プロトコルは JSON-RPC 2.0 風の JSON メッセージ。

**Why this priority**: 双方向通信がないと trev → Neovim の連携（ファイルを開く通知等）ができない。

**Independent Test**: テストクライアントでソケットに接続し、Request → Response と Notification の受信を確認。

**Acceptance Scenarios**:

1. **Given** クライアントが接続, **When** `{"id": 1, "method": "ping"}` を送信, **Then** `{"id": 1, "result": {"ok": true}}` が返る
2. **Given** クライアントが接続, **When** `{"id": 2, "method": "reveal", "params": {"path": "/foo/bar.rs"}}` を送信, **Then** trev が該当パスを選択し、`{"id": 2, "result": {"ok": true}}` が返る
3. **Given** クライアントが接続中, **When** ユーザーが trev でファイルを開く操作をする, **Then** クライアントに `{"method": "open", "params": {"action": "edit", "path": "/foo/bar.rs"}}` が通知される
4. **Given** 不正な JSON を送信, **When** サーバーが受信, **Then** エラーレスポンスを返し、接続は維持される
5. **Given** 未知のメソッドを送信, **When** サーバーが受信, **Then** "method not found" エラーレスポンスを返す

---

### User Story 3 - Reveal（パス選択）機能 (Priority: P1)

Neovim で編集中のファイルに対応するエントリを trev のツリーで自動選択（reveal）する。Neovim がバッファを切り替えるたびに trev のカーソルが追従する。

**Why this priority**: Neovim 連携の核心機能。エディタとファイルブラウザの同期はユーザー体験の要。

**Independent Test**: `trev ctl reveal /path/to/file` を実行し、trev のカーソルが該当ファイルに移動することを確認。

**Acceptance Scenarios**:

1. **Given** trev でディレクトリを表示中, **When** `reveal /foo/bar.rs` を受信, **Then** 必要な親ディレクトリを自動展開し、`bar.rs` にカーソルを移動
2. **Given** reveal 対象が未読み込みディレクトリ内にある, **When** `reveal` を受信, **Then** 必要なディレクトリを遅延読み込みし、読み込み完了後にカーソルを移動
3. **Given** reveal 対象のパスが存在しない, **When** `reveal` を受信, **Then** エラーレスポンスを返し、カーソルは移動しない
4. **Given** reveal 対象がツリーのルート外にある, **When** `reveal` を受信, **Then** エラーレスポンスを返す

---

### User Story 4 - ファイルオープン通知 (Priority: P2)

ユーザーが trev でファイルを「開く」操作（Enter / l）をすると、接続中のクライアントにファイルオープン通知が送信される。Neovim プラグインはこの通知を受けてファイルを開く。

**Why this priority**: reveal と対になる機能。trev → Neovim 方向の連携。

**Independent Test**: trev でファイルを開く操作をし、接続中のテストクライアントが通知を受信することを確認。

**Acceptance Scenarios**:

1. **Given** クライアント接続中 + `--action edit`, **When** ファイルを開く, **Then** `{"method": "open", "params": {"action": "edit", "path": "..."}}` が通知される
2. **Given** クライアント接続中 + `--action vsplit`, **When** ファイルを開く, **Then** `{"method": "open", "params": {"action": "vsplit", "path": "..."}}` が通知される
3. **Given** クライアント未接続, **When** ファイルを開く, **Then** 通知は送信されず、エラーにもならない
4. **Given** 複数のクライアントが接続中, **When** ファイルを開く, **Then** すべてのクライアントに通知が送信される

---

### User Story 5 - Neovim プラグイン（サイドパネルモード）(Priority: P2)

Neovim ユーザーは `:TrevToggle` コマンドで trev をサイドパネルとして開閉できる。trev は toggleterm（または類似の端末バッファ）内で daemon モードで起動し、バッファ切替時に自動 reveal が行われる。

**Why this priority**: Neovim からの最も一般的な使用パターン。

**Independent Test**: Neovim で `:TrevOpen` を実行し、サイドパネルに trev が表示され、バッファ切替で reveal が動作することを確認。

**Acceptance Scenarios**:

1. **Given** Neovim で `:TrevOpen`, **When** サイドパネルが開く, **Then** trev が daemon モードで起動し、現在のバッファのファイルが reveal される
2. **Given** trev サイドパネル表示中, **When** Neovim でバッファを切り替え, **Then** trev のカーソルが新しいバッファのファイルに追従
3. **Given** trev サイドパネル表示中, **When** `:TrevClose` を実行, **Then** サイドパネルが閉じ、trev プロセスが終了する
4. **Given** trev サイドパネル表示中, **When** `:TrevToggle` を実行, **Then** サイドパネルが閉じる
5. **Given** trev サイドパネルが閉じている, **When** `:TrevToggle` を実行, **Then** サイドパネルが開く
6. **Given** trev で開く操作, **When** Neovim に通知が届く, **Then** Neovim が指定された action (edit/split/vsplit/tabedit) でファイルを開く

---

### User Story 6 - Neovim プラグイン（フローティングモード）(Priority: P3)

Neovim ユーザーは `:TrevOpen float` コマンドで trev をフローティングウィンドウとして一時的に開き、ファイルピッカーとして使用できる。ファイル選択後に trev は自動で閉じ、選択したファイルが Neovim で開かれる。

**Why this priority**: ファイルピッカーとしての利用は便利だが、サイドパネルモードの後に実装すべき。

**Independent Test**: `:TrevOpen float` でフローティングウィンドウが開き、ファイル選択で Neovim にファイルが開かれることを確認。

**Acceptance Scenarios**:

1. **Given** `:TrevOpen float` を実行, **When** フローティングウィンドウが開く, **Then** trev が `--emit` モードで起動する
2. **Given** フローティング trev でファイルを選択, **When** Enter を押す, **Then** フローティングウィンドウが閉じ、Neovim でファイルが開かれる
3. **Given** フローティング trev で `q` / Esc, **When** 閉じる, **Then** ファイルは開かれずウィンドウのみ閉じる
4. **Given** フローティングウィンドウのサイズ, **When** 設定を確認, **Then** `width` / `height` の比率が設定通りである

---

### User Story 7 - 接続管理と障害回復 (Priority: P3)

IPC 接続の断絶や trev プロセスのクラッシュを検知し、適切に回復する。Neovim プラグインは接続状態をユーザーに表示する。

**Why this priority**: 堅牢性の向上。基本機能が安定した後に強化すべき。

**Independent Test**: trev プロセスを kill し、Neovim プラグインが切断を検知して再接続を試みることを確認。

**Acceptance Scenarios**:

1. **Given** Neovim プラグインが接続中, **When** trev プロセスがクラッシュ, **Then** プラグインが切断を検知し、ステータスラインに "disconnected" を表示
2. **Given** 接続が切断された状態, **When** `reveal` が要求される, **Then** 再接続を試み、成功すれば reveal を実行
3. **Given** trev が正常に起動している, **When** Neovim プラグインのステータスを確認, **Then** "connected" が表示される

---

### Edge Cases

- ソケットファイルのパーミッション問題 → エラーメッセージにソケットパスを含めて表示
- `$XDG_RUNTIME_DIR` が未設定 → `/tmp/trev/` にフォールバック
- 同時に複数クライアントが接続 → すべてのクライアントに通知を broadcast
- クライアントが応答なしでハングアップ → 書き込みタイムアウト後に接続を切断
- JSON メッセージが部分的に到着 → 改行区切りで完全なメッセージのみ処理
- ソケットパスが OS のパス長制限を超える → 短いパスにフォールバック
- Neovim プラグインと trev のバージョン不整合 → プロトコルバージョンを含めたハンドシェイク

## Requirements

### Functional Requirements

- **FR-001**: `--daemon` フラグで起動時、Unix Domain Socket 上で IPC サーバーを起動する
- **FR-002**: ソケットパスは `$XDG_RUNTIME_DIR/trev/<workspace_key>.sock`。`XDG_RUNTIME_DIR` 未設定時は `/tmp/trev/` にフォールバック
- **FR-003**: IPC プロトコルは改行区切りの JSON メッセージ。Request/Response パターン（id あり）と Notification パターン（id なし）をサポート
- **FR-004**: サポートするメソッド（Request/Response）: `ping`, `reveal`, `quit`
- **FR-005**: サポートする通知（Notification、trev → クライアント）: `open` (action + path), `selection_changed` (path), `closing`
- **FR-006**: `reveal` は指定パスまでの親ディレクトリを自動展開し、カーソルを移動する。遅延読み込みが必要な場合は読み込みを待つ
- **FR-007**: `--action` フラグで指定されたアクション（edit/split/vsplit/tabedit）が `open` 通知の `action` フィールドに使用される
- **FR-008**: `--emit` フラグで起動時、ファイル選択後に stdout にパスを出力して終了する（フローティングモード用）
- **FR-009**: `--emit-format` で出力フォーマットを制御: `lines` (デフォルト), `nul`, `json`
- **FR-010**: `trev ctl` サブコマンドで外部から IPC メッセージを送信できる: `reveal <path>`, `ping`, `quit`
- **FR-011**: 複数クライアントの同時接続をサポートし、通知はすべてのクライアントに broadcast
- **FR-012**: stale なソケットファイル（前回のクラッシュ等）を検出し、自動的にクリーンアップする

### Neovim プラグイン要件

- **FR-NV-001**: Lua プラグインとして提供。`require("trev").setup({...})` で初期化
- **FR-NV-002**: コマンド: `:TrevOpen [float] [reveal] [action]`, `:TrevToggle [float]`, `:TrevClose`, `:TrevReveal [path]`
- **FR-NV-003**: サイドパネルモード: toggleterm（または端末バッファ）内で trev を daemon モードで起動
- **FR-NV-004**: フローティングモード: フローティングウィンドウ内で trev を `--emit` モードで起動
- **FR-NV-005**: `auto_reveal` オプション（デフォルト: true）が有効な場合、`BufEnter` autocommand でバッファのファイルパスを自動 reveal
- **FR-NV-006**: `vim.uv.new_pipe()` でソケットに接続し、双方向通信を行う
- **FR-NV-007**: `open` 通知を受けたら、指定された action でファイルを開く
- **FR-NV-008**: 接続状態（connected/disconnected）をユーザーが取得可能な API を提供

### Non-Functional Requirements

- **NFR-001**: IPC メッセージの往復遅延は 10ms 以内（ローカル通信）
- **NFR-002**: reveal 操作（遅延読み込み含む）は 500ms 以内に完了
- **NFR-003**: IPC サーバーは UI スレッドと独立して動作し、UI のフレームレートに影響を与えない

### Key Entities

- **IpcServer**: tokio ベースの Unix Domain Socket サーバー。接続管理と Request/Response/Notification の処理
- **IpcClient**: `trev ctl` サブコマンド用の IPC クライアント
- **IpcMessage**: JSON メッセージの型定義（Request, Response, Notification）
- **Workspace**: IPC ソケットのスコープを管理（workspace_key によるマルチインスタンス対応）

## Success Criteria

### Measurable Outcomes

- **SC-001**: `trev ctl ping` の応答が 10ms 以内
- **SC-002**: `reveal` が（遅延読み込み含めて）500ms 以内にカーソルを移動
- **SC-003**: IPC サーバー起動が trev の起動時間に追加する遅延は 10ms 以下
- **SC-004**: クライアント切断時にサーバーがクラッシュするケースがゼロ
- **SC-005**: Neovim でのバッファ切替から trev の reveal 完了まで 100ms 以内（ローカル環境）
- **SC-006**: Neovim プラグインのセットアップが 10 行以内の Lua コードで完了

## Assumptions

- Neovim 0.9+ を対象（`vim.uv` API が利用可能）
- Unix Domain Socket を使用するため、Windows 対応は別途検討が必要（Named Pipe への切替）
- プロトコルバージョニングは v1.0 後に検討。初期実装ではバージョンチェックなし
- Neovim プラグインは trev リポジトリ内に同梱（`lua/trev/` ディレクトリ）
- trev の複数インスタンスは workspace_key で区別。デフォルトの workspace_key は起動ディレクトリのハッシュ
