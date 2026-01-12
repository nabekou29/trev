//! アプリケーション状態とイベントループ
//!
//! TUI アプリケーションの中核となる状態管理とイベント処理を提供する。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crossterm::event::{
    self,
    Event,
    KeyCode,
    KeyEvent,
    KeyEventKind,
    KeyModifiers,
};
use ratatui::Frame;

use crate::cli::Args;
use crate::config::Config;
use crate::error::Result;
use crate::git::GitCache;
use crate::input::InputBuffer;
use crate::preview::PreviewState;
use crate::tree::{SortConfig, TreeState};
use crate::tree::builder::TreeBuilder;
use crate::ui;

/// アプリケーションの実行状態
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) enum AppState {
    /// 実行中
    #[default]
    Running,
    /// ファイルを外部エディタで開く
    OpenFile(PathBuf),
    /// 選択して終了
    Quit,
    /// キャンセル終了
    Cancelled,
}

/// 入力モード
///
/// `gg` などの複合キー入力に対応するための状態。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) enum InputMode {
    /// 通常モード
    #[default]
    Normal,
    /// `g` が押された状態（`gg` 待ち）
    WaitingForG,
    /// `y` が押された状態（`yy` 待ち）
    WaitingForY,
    /// 検索モード
    Searching,
    /// 削除確認モード
    ConfirmDelete,
    /// リネームモード
    Renaming,
    /// ペースト確認モード（コンフリクト）
    ConfirmPaste,
    /// ファイル追加モード
    AddingFile,
    /// ディレクトリ追加モード
    AddingDirectory,
}

/// ペースト時のコンフリクト情報（複数ファイル対応）
#[derive(Debug, Clone)]
pub(crate) struct PasteConflict {
    /// 残りのコピー/カット元パス
    pub(crate) remaining_sources: Vec<PathBuf>,
    /// 現在コンフリクト中のソースパス
    pub(crate) current_src: PathBuf,
    /// 現在コンフリクト中のコピー/カット先パス
    pub(crate) current_dest: PathBuf,
    /// コピー先ディレクトリ
    pub(crate) dest_dir: PathBuf,
    /// 操作の種類
    pub(crate) operation: ClipboardOperation,
}

/// クリップボード操作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipboardOperation {
    /// コピー
    Copy,
    /// カット（移動）
    Cut,
}

/// アプリケーション
///
/// TUI アプリケーションの全体状態を管理する。
#[derive(Debug)]
pub(crate) struct App {
    /// 実行状態
    pub(crate) state: AppState,
    /// ツリー状態
    pub(crate) tree: TreeState,
    /// プレビュー状態
    pub(crate) preview: PreviewState,
    /// Git キャッシュ
    pub(crate) git_cache: GitCache,
    /// 入力モード
    pub(crate) input_mode: InputMode,
    /// 検索入力バッファ
    pub(crate) search_input: InputBuffer,
    /// 検索開始前のクエリ（キャンセル時の復元用）
    search_query_backup: String,
    /// 削除確認中のパス（複数対応）
    pub(crate) delete_targets: Vec<PathBuf>,
    /// リネーム対象のパス
    pub(crate) rename_target: Option<PathBuf>,
    /// リネーム入力バッファ
    pub(crate) rename_input: InputBuffer,
    /// 追加先ディレクトリ
    pub(crate) add_target_dir: Option<PathBuf>,
    /// 追加入力バッファ
    pub(crate) add_input: InputBuffer,
    /// クリップボード（コピー/カット対象）- 複数ファイル対応
    pub(crate) clipboard: Option<(Vec<PathBuf>, ClipboardOperation)>,
    /// ペースト確認中のコンフリクト情報
    pub(crate) paste_conflict: Option<PasteConflict>,
    /// マークされたファイルパス
    pub(crate) marked_paths: HashSet<PathBuf>,
    /// 終了時に出力するパス
    pub(crate) selected_path: Option<PathBuf>,
    /// ルートパス
    root_path: PathBuf,
    /// 隠しファイルを表示するか
    pub(crate) show_hidden: bool,
    /// gitignore されたファイルを表示するか
    pub(crate) show_ignored: bool,
    /// プレビューを表示するか
    pub(crate) show_preview: bool,
    /// ツリー表示領域の高さ
    view_height: usize,
    /// ソート設定
    pub(crate) sort_config: SortConfig,
}

impl App {
    /// 新しい `App` を作成する
    pub(crate) fn new(args: &Args) -> Result<Self> {
        // 設定ファイルを読み込む
        let config = Config::load();

        // ソート設定を設定ファイルから初期化
        let sort_config = SortConfig {
            order: config.sort_order,
            direction: config.sort_direction,
            directories_first: config.directories_first,
        };

        // 隠しファイル表示は CLI 引数が優先
        let show_hidden = args.show_hidden || config.show_hidden;

        // ツリーを構築
        let root = TreeBuilder::new()
            .show_hidden(show_hidden)
            .show_ignored(config.show_ignored)
            .sort_config(sort_config)
            .build(&args.path)
            .ok_or_else(|| crate::error::AppError::InvalidPath(args.path.clone()))?;

        let mut tree = TreeState::new(root);

        // Git キャッシュを構築
        let git_cache = GitCache::from_path(&args.path);

        // Git ステータスを反映
        tree.update_git_status(
            |path| git_cache.get_status(path),
            |path| git_cache.get_directory_summary(path),
        );

        // プレビュー状態を初期化
        let mut preview = PreviewState::new();
        // 初期選択のプレビューを読み込む
        if let Some(node) = tree.selected_node() {
            preview.load(&node.path);
        }

        Ok(Self {
            state: AppState::Running,
            tree,
            preview,
            git_cache,
            input_mode: InputMode::Normal,
            search_input: InputBuffer::new(),
            search_query_backup: String::new(),
            delete_targets: Vec::new(),
            rename_target: None,
            rename_input: InputBuffer::new(),
            add_target_dir: None,
            add_input: InputBuffer::new(),
            clipboard: None,
            paste_conflict: None,
            marked_paths: HashSet::new(),
            selected_path: None,
            root_path: args.path.clone(),
            show_hidden,
            show_ignored: config.show_ignored,
            show_preview: config.show_preview,
            view_height: 20, // 初期値、draw で更新される
            sort_config,
        })
    }

    /// アプリケーションが実行中かどうかを返す
    pub(crate) fn is_running(&self) -> bool {
        matches!(self.state, AppState::Running | AppState::OpenFile(_))
    }

    /// ファイルを開く要求があるかを確認し、パスを取得する
    pub(crate) fn take_open_file_request(&mut self) -> Option<PathBuf> {
        if let AppState::OpenFile(path) = &self.state {
            let path = path.clone();
            self.state = AppState::Running;
            Some(path)
        } else {
            None
        }
    }

    /// 画面を描画する
    pub(crate) fn draw(&mut self, frame: &mut Frame<'_>) {
        // 表示領域に合わせてスクロールを調整
        let height = frame.area().height.saturating_sub(2) as usize; // ステータスバー分を引く
        self.view_height = height;
        self.tree.adjust_scroll(height);

        ui::render(frame, self);
    }

    /// イベントを処理する
    pub(crate) fn handle_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            // KeyEventKind::Press のみ処理（リピート防止）
            if key.kind == KeyEventKind::Press {
                self.handle_key(key);
            }
        }
        Ok(())
    }

    /// キーイベントを処理する
    fn handle_key(&mut self, key: KeyEvent) {
        // 検索モードの場合は専用ハンドラへ
        if self.input_mode == InputMode::Searching {
            self.handle_search_key(key);
            return;
        }

        // 削除確認モードの場合は専用ハンドラへ
        if self.input_mode == InputMode::ConfirmDelete {
            self.handle_delete_confirm_key(key);
            return;
        }

        // リネームモードの場合は専用ハンドラへ
        if self.input_mode == InputMode::Renaming {
            self.handle_rename_key(key);
            return;
        }

        // ペースト確認モードの場合は専用ハンドラへ
        if self.input_mode == InputMode::ConfirmPaste {
            self.handle_paste_confirm_key(key);
            return;
        }

        // 追加モードの場合は専用ハンドラへ
        if matches!(self.input_mode, InputMode::AddingFile | InputMode::AddingDirectory) {
            self.handle_add_key(key);
            return;
        }

        let prev_selected = self.tree.selected();

        match (&self.input_mode, key.code) {
            // gg: 先頭へ
            (InputMode::WaitingForG, KeyCode::Char('g')) => {
                self.tree.select_first();
                self.input_mode = InputMode::Normal;
            }

            // yy: コピー
            (InputMode::WaitingForY, KeyCode::Char('y')) => {
                self.copy_selected();
                self.input_mode = InputMode::Normal;
            }

            // g 単体押し -> gg 待ち
            (InputMode::Normal, KeyCode::Char('g')) => {
                self.input_mode = InputMode::WaitingForG;
            }

            // y 単体押し -> yy 待ち
            (InputMode::Normal, KeyCode::Char('y')) => {
                self.input_mode = InputMode::WaitingForY;
            }

            // 検索モード開始
            (InputMode::Normal, KeyCode::Char('/')) => {
                self.input_mode = InputMode::Searching;
                // 現在のクエリをバックアップして、新しい検索を開始
                self.search_query_backup = self.search_input.text().to_string();
                self.search_input.clear();
                self.apply_filter();
            }

            // 通常モードのキー
            (_, KeyCode::Char('j') | KeyCode::Down) => {
                self.tree.select_next();
                self.input_mode = InputMode::Normal;
            }

            (_, KeyCode::Char('k') | KeyCode::Up) => {
                self.tree.select_prev();
                self.input_mode = InputMode::Normal;
            }

            // Ctrl-d: 半ページ下
            (_, KeyCode::Char('d')) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.tree.page_down(self.view_height);
                self.input_mode = InputMode::Normal;
            }

            // Ctrl-u: 半ページ上
            (_, KeyCode::Char('u')) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.tree.page_up(self.view_height);
                self.input_mode = InputMode::Normal;
            }

            (_, KeyCode::Char('l') | KeyCode::Right) => {
                self.expand_or_enter();
                self.refresh_git_status();
                self.input_mode = InputMode::Normal;
            }

            (_, KeyCode::Char('h') | KeyCode::Left) => {
                self.tree.collapse_selected();
                self.refresh_git_status();
                self.input_mode = InputMode::Normal;
            }

            (_, KeyCode::Char('G')) => {
                self.tree.select_last();
                self.input_mode = InputMode::Normal;
            }

            // プレビュー縦スクロール
            (_, KeyCode::Char('J')) => {
                self.preview.scroll_down(1);
                self.input_mode = InputMode::Normal;
            }

            (_, KeyCode::Char('K')) => {
                self.preview.scroll_up(1);
                self.input_mode = InputMode::Normal;
            }

            // プレビュー横スクロール
            (_, KeyCode::Char('L')) => {
                self.preview.scroll_right(4);
                self.input_mode = InputMode::Normal;
            }

            (_, KeyCode::Char('H')) => {
                self.preview.scroll_left(4);
                self.input_mode = InputMode::Normal;
            }

            (_, KeyCode::Enter) => {
                self.confirm_selection();
            }

            // ファイルを開く
            (_, KeyCode::Char('o')) => {
                self.open_selected_file();
                self.input_mode = InputMode::Normal;
            }

            (_, KeyCode::Char('q') | KeyCode::Esc) => {
                self.state = AppState::Cancelled;
            }

            // 隠しファイル表示トグル
            (_, KeyCode::Char('.')) => {
                self.toggle_hidden();
                self.input_mode = InputMode::Normal;
            }

            // ソート順切り替え
            (_, KeyCode::Char('S')) => {
                self.cycle_sort_order();
                self.input_mode = InputMode::Normal;
            }

            // ソート方向トグル
            (_, KeyCode::Char('s')) => {
                self.toggle_sort_direction();
                self.input_mode = InputMode::Normal;
            }

            // フィルタクリア
            (_, KeyCode::Char('n')) => {
                self.clear_filter();
                self.input_mode = InputMode::Normal;
            }

            // プレビュー表示トグル
            (_, KeyCode::Char('p')) => {
                self.show_preview = !self.show_preview;
                self.input_mode = InputMode::Normal;
            }

            // ignored ファイル表示トグル
            (_, KeyCode::Char('I')) => {
                self.toggle_ignored();
                self.input_mode = InputMode::Normal;
            }

            // 削除
            (_, KeyCode::Char('d') | KeyCode::Delete) => {
                self.start_delete();
            }

            // リネーム
            (_, KeyCode::Char('r')) => {
                self.start_rename();
            }

            // カット
            (_, KeyCode::Char('x')) => {
                self.cut_selected();
                self.input_mode = InputMode::Normal;
            }

            // ペースト
            (InputMode::Normal, KeyCode::Char('P')) => {
                self.paste();
                self.input_mode = InputMode::Normal;
            }

            // マークトグル
            (_, KeyCode::Char('m')) => {
                self.toggle_mark();
                self.input_mode = InputMode::Normal;
            }

            // マーク全解除
            (InputMode::Normal, KeyCode::Char('M')) => {
                self.marked_paths.clear();
                self.input_mode = InputMode::Normal;
            }

            // ファイル追加
            (InputMode::Normal, KeyCode::Char('a')) => {
                self.start_add_file();
            }

            // ディレクトリ追加
            (InputMode::Normal, KeyCode::Char('A')) => {
                self.start_add_directory();
            }

            _ => {
                self.input_mode = InputMode::Normal;
            }
        }

        // 選択が変更されたらプレビューを更新
        if self.tree.selected() != prev_selected {
            self.update_preview();
        }
    }

    /// リネームモードでのキーイベントを処理する
    fn handle_rename_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            // リネーム確定
            (KeyCode::Enter, _) => {
                self.execute_rename();
                self.input_mode = InputMode::Normal;
            }

            // キャンセル
            (KeyCode::Esc, _) => {
                self.rename_target = None;
                self.rename_input.clear();
                self.input_mode = InputMode::Normal;
            }

            // C-h: バックスペース
            (KeyCode::Char('h'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.rename_input.backspace();
            }

            // バックスペース
            (KeyCode::Backspace, _) => {
                self.rename_input.backspace();
            }

            // C-w: 単語削除
            (KeyCode::Char('w'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.rename_input.delete_word();
            }

            // C-u: 行頭まで削除
            (KeyCode::Char('u'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.rename_input.delete_to_start();
            }

            // C-a / Home: 行頭へ
            (KeyCode::Char('a'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.rename_input.move_home();
            }
            (KeyCode::Home, _) => {
                self.rename_input.move_home();
            }

            // C-e / End: 行末へ
            (KeyCode::Char('e'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.rename_input.move_end();
            }
            (KeyCode::End, _) => {
                self.rename_input.move_end();
            }

            // M-b / M-Left: 前の単語へ
            (KeyCode::Char('b'), m) if m.contains(KeyModifiers::ALT) => {
                self.rename_input.move_word_left();
            }
            (KeyCode::Left, m) if m.contains(KeyModifiers::ALT) => {
                self.rename_input.move_word_left();
            }

            // M-f / M-Right: 次の単語へ
            (KeyCode::Char('f'), m) if m.contains(KeyModifiers::ALT) => {
                self.rename_input.move_word_right();
            }
            (KeyCode::Right, m) if m.contains(KeyModifiers::ALT) => {
                self.rename_input.move_word_right();
            }

            // 左右カーソル
            (KeyCode::Left, _) => {
                self.rename_input.move_left();
            }
            (KeyCode::Right, _) => {
                self.rename_input.move_right();
            }

            // 文字入力
            (KeyCode::Char(c), _) => {
                self.rename_input.insert(c);
            }

            _ => {}
        }
    }

    /// 削除確認モードでのキーイベントを処理する
    fn handle_delete_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            // 削除確定
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.execute_delete();
                self.input_mode = InputMode::Normal;
            }

            // キャンセル
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.delete_targets.clear();
                self.input_mode = InputMode::Normal;
            }

            _ => {}
        }
    }

    /// ペースト確認モードでのキーイベントを処理する
    fn handle_paste_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            // 上書き（overwrite）
            KeyCode::Char('o') | KeyCode::Char('O') => {
                self.execute_paste_overwrite_and_continue();
                // paste_files が終わると Normal になるか、次のコンフリクトで ConfirmPaste のまま
                if self.input_mode != InputMode::ConfirmPaste {
                    self.input_mode = InputMode::Normal;
                }
            }

            // リネーム（rename）
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.execute_paste_rename_and_continue();
                if self.input_mode != InputMode::ConfirmPaste {
                    self.input_mode = InputMode::Normal;
                }
            }

            // スキップ（現在のファイルのみ）
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.skip_paste_and_continue();
                if self.input_mode != InputMode::ConfirmPaste {
                    self.input_mode = InputMode::Normal;
                }
            }

            // キャンセル（全てキャンセル）
            KeyCode::Esc => {
                self.paste_conflict = None;
                self.input_mode = InputMode::Normal;
            }

            _ => {}
        }
    }

    /// 検索モードでのキーイベントを処理する
    fn handle_search_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            // 検索確定
            (KeyCode::Enter, _) => {
                self.apply_filter();
                self.input_mode = InputMode::Normal;
            }

            // 検索キャンセル（元のフィルタを復元）
            (KeyCode::Esc, _) => {
                self.search_input.set(std::mem::take(&mut self.search_query_backup));
                self.apply_filter();
                self.input_mode = InputMode::Normal;
            }

            // C-h: バックスペース
            (KeyCode::Char('h'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.search_input.backspace();
                self.apply_filter();
            }

            // バックスペース
            (KeyCode::Backspace, _) => {
                self.search_input.backspace();
                self.apply_filter();
            }

            // C-w: 単語削除
            (KeyCode::Char('w'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.search_input.delete_word();
                self.apply_filter();
            }

            // C-u: 行頭まで削除
            (KeyCode::Char('u'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.search_input.delete_to_start();
                self.apply_filter();
            }

            // C-a / Home: 行頭へ
            (KeyCode::Char('a'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.search_input.move_home();
            }
            (KeyCode::Home, _) => {
                self.search_input.move_home();
            }

            // C-e / End: 行末へ
            (KeyCode::Char('e'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.search_input.move_end();
            }
            (KeyCode::End, _) => {
                self.search_input.move_end();
            }

            // M-b / M-Left: 前の単語へ
            (KeyCode::Char('b'), m) if m.contains(KeyModifiers::ALT) => {
                self.search_input.move_word_left();
            }
            (KeyCode::Left, m) if m.contains(KeyModifiers::ALT) => {
                self.search_input.move_word_left();
            }

            // M-f / M-Right: 次の単語へ
            (KeyCode::Char('f'), m) if m.contains(KeyModifiers::ALT) => {
                self.search_input.move_word_right();
            }
            (KeyCode::Right, m) if m.contains(KeyModifiers::ALT) => {
                self.search_input.move_word_right();
            }

            // 左右カーソル
            (KeyCode::Left, _) => {
                self.search_input.move_left();
            }
            (KeyCode::Right, _) => {
                self.search_input.move_right();
            }

            // 文字入力
            (KeyCode::Char(c), _) => {
                self.search_input.insert(c);
                self.apply_filter();
            }

            _ => {}
        }
    }

    /// フィルタを適用する
    fn apply_filter(&mut self) {
        if self.search_input.is_empty() {
            self.tree.clear_filter();
        } else {
            self.tree.set_filter(self.search_input.text());
        }
        self.update_preview();
    }

    /// フィルタをクリアする
    fn clear_filter(&mut self) {
        self.search_input.clear();
        self.tree.clear_filter();
        self.update_preview();
    }

    /// プレビューを更新する
    fn update_preview(&mut self) {
        if let Some(node) = self.tree.selected_node() {
            self.preview.load(&node.path);
        }
    }

    /// 選択ノードを展開または子に入る
    fn expand_or_enter(&mut self) {
        // ディレクトリなら展開を試みる
        if !self.tree.expand_selected() {
            // 展開できなかった場合（既に展開済み or ファイル）
            // ディレクトリで既に展開済みなら最初の子を選択
            if let Some(node) = self.tree.selected_node()
                && node.expanded
            {
                self.tree.select_next();
            }
        }
    }

    /// 選択を確定して終了
    fn confirm_selection(&mut self) {
        if let Some(node) = self.tree.selected_node() {
            self.selected_path = Some(node.path.clone());
        }
        self.state = AppState::Quit;
    }

    /// 選択中のファイルを外部エディタで開く
    fn open_selected_file(&mut self) {
        if let Some(node) = self.tree.selected_node() {
            // ディレクトリの場合は展開/折り畳み
            if node.kind == crate::tree::NodeKind::Directory {
                self.expand_or_enter();
                return;
            }
            // ファイルの場合は外部エディタで開く
            self.state = AppState::OpenFile(node.path.clone());
        }
    }

    /// 削除確認を開始する
    fn start_delete(&mut self) {
        let paths = self.get_marked_or_selected_paths();
        if !paths.is_empty() {
            self.delete_targets = paths;
            self.input_mode = InputMode::ConfirmDelete;
        }
    }

    /// 削除を実行する
    fn execute_delete(&mut self) {
        let targets = std::mem::take(&mut self.delete_targets);
        let mut any_deleted = false;

        for path in targets {
            let result = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };

            if result.is_ok() {
                any_deleted = true;
            }
        }

        if any_deleted {
            // マークをクリア
            self.marked_paths.clear();
            // ツリーを再構築
            self.rebuild_tree();
        }
    }

    /// リネームを開始する
    fn start_rename(&mut self) {
        if let Some(node) = self.tree.selected_node() {
            self.rename_target = Some(node.path.clone());
            // 現在のファイル名を初期値として設定
            self.rename_input.set(node.name.clone());
            self.input_mode = InputMode::Renaming;
        }
    }

    /// リネームを実行する
    fn execute_rename(&mut self) {
        if let Some(old_path) = self.rename_target.take() {
            let new_name = self.rename_input.take();

            // 空の名前は無効
            if new_name.is_empty() {
                return;
            }

            // 新しいパスを構築
            if let Some(parent) = old_path.parent() {
                let new_path = parent.join(&new_name);

                // リネーム実行
                if std::fs::rename(&old_path, &new_path).is_ok() {
                    // ツリーを再構築
                    self.rebuild_tree();
                    // 新しいパスを選択
                    self.tree.select_path(&new_path);
                }
            }
        }
    }

    /// マークまたは選択中のファイルをコピーとしてクリップボードに登録
    fn copy_selected(&mut self) {
        let paths = self.get_marked_or_selected_paths();
        if !paths.is_empty() {
            self.clipboard = Some((paths, ClipboardOperation::Copy));
            self.marked_paths.clear();
        }
    }

    /// マークまたは選択中のファイルをカット（移動）としてクリップボードに登録
    fn cut_selected(&mut self) {
        let paths = self.get_marked_or_selected_paths();
        if !paths.is_empty() {
            self.clipboard = Some((paths, ClipboardOperation::Cut));
            self.marked_paths.clear();
        }
    }

    /// 追加モードでのキーイベントを処理する
    fn handle_add_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            // 追加確定
            (KeyCode::Enter, _) => {
                self.execute_add();
                self.input_mode = InputMode::Normal;
            }

            // キャンセル
            (KeyCode::Esc, _) => {
                self.add_target_dir = None;
                self.add_input.clear();
                self.input_mode = InputMode::Normal;
            }

            // C-h: バックスペース
            (KeyCode::Char('h'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.add_input.backspace();
            }

            // バックスペース
            (KeyCode::Backspace, _) => {
                self.add_input.backspace();
            }

            // C-w: 単語削除
            (KeyCode::Char('w'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.add_input.delete_word();
            }

            // C-u: 行頭まで削除
            (KeyCode::Char('u'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.add_input.delete_to_start();
            }

            // C-a / Home: 行頭へ
            (KeyCode::Char('a'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.add_input.move_home();
            }
            (KeyCode::Home, _) => {
                self.add_input.move_home();
            }

            // C-e / End: 行末へ
            (KeyCode::Char('e'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.add_input.move_end();
            }
            (KeyCode::End, _) => {
                self.add_input.move_end();
            }

            // M-b / M-Left: 前の単語へ
            (KeyCode::Char('b'), m) if m.contains(KeyModifiers::ALT) => {
                self.add_input.move_word_left();
            }
            (KeyCode::Left, m) if m.contains(KeyModifiers::ALT) => {
                self.add_input.move_word_left();
            }

            // M-f / M-Right: 次の単語へ
            (KeyCode::Char('f'), m) if m.contains(KeyModifiers::ALT) => {
                self.add_input.move_word_right();
            }
            (KeyCode::Right, m) if m.contains(KeyModifiers::ALT) => {
                self.add_input.move_word_right();
            }

            // 左右カーソル
            (KeyCode::Left, _) => {
                self.add_input.move_left();
            }
            (KeyCode::Right, _) => {
                self.add_input.move_right();
            }

            // 文字入力
            (KeyCode::Char(c), _) => {
                self.add_input.insert(c);
            }

            _ => {}
        }
    }

    /// ファイル追加モードを開始する
    fn start_add_file(&mut self) {
        let target_dir = self.get_current_directory();
        self.add_target_dir = Some(target_dir);
        self.add_input.clear();
        self.input_mode = InputMode::AddingFile;
    }

    /// ディレクトリ追加モードを開始する
    fn start_add_directory(&mut self) {
        let target_dir = self.get_current_directory();
        self.add_target_dir = Some(target_dir);
        self.add_input.clear();
        self.input_mode = InputMode::AddingDirectory;
    }

    /// 現在のディレクトリを取得（選択がディレクトリならそれ、ファイルなら親）
    fn get_current_directory(&self) -> PathBuf {
        if let Some(node) = self.tree.selected_node() {
            if node.kind == crate::tree::NodeKind::Directory {
                node.path.clone()
            } else {
                node.path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.root_path.clone())
            }
        } else {
            self.root_path.clone()
        }
    }

    /// ファイル/ディレクトリを追加する
    fn execute_add(&mut self) {
        let Some(target_dir) = self.add_target_dir.take() else {
            return;
        };
        let name = self.add_input.take();

        if name.is_empty() {
            return;
        }

        // 末尾が / の場合はディレクトリとして扱う
        let is_directory = name.ends_with('/') || self.input_mode == InputMode::AddingDirectory;
        let name = name.trim_end_matches('/');

        let new_path = target_dir.join(name);

        // 既に存在する場合は何もしない
        if new_path.exists() {
            return;
        }

        let result = if is_directory {
            // ディレクトリ作成（中間ディレクトリも作成）
            std::fs::create_dir_all(&new_path)
        } else {
            // ファイル作成（親ディレクトリがなければ先に作成）
            if let Some(parent) = new_path.parent() {
                if !parent.exists() {
                    if std::fs::create_dir_all(parent).is_err() {
                        return;
                    }
                }
            }
            std::fs::File::create(&new_path).map(|_| ())
        };

        if result.is_ok() {
            self.rebuild_tree();
            self.tree.select_path(&new_path);
        }
    }

    /// 選択中のファイルのマークをトグル
    fn toggle_mark(&mut self) {
        if let Some(node) = self.tree.selected_node() {
            let path = node.path.clone();
            if self.marked_paths.contains(&path) {
                self.marked_paths.remove(&path);
            } else {
                self.marked_paths.insert(path);
            }
            // マーク後に次の項目へ移動
            self.tree.select_next();
        }
    }

    /// マークされたパスまたは選択中のパスを取得
    fn get_marked_or_selected_paths(&self) -> Vec<PathBuf> {
        if !self.marked_paths.is_empty() {
            self.marked_paths.iter().cloned().collect()
        } else if let Some(node) = self.tree.selected_node() {
            vec![node.path.clone()]
        } else {
            vec![]
        }
    }

    /// クリップボードの内容を現在のディレクトリにペースト
    fn paste(&mut self) {
        let Some((src_paths, operation)) = self.clipboard.clone() else {
            return;
        };

        if src_paths.is_empty() {
            return;
        }

        // コピー先ディレクトリを決定
        let dest_dir = if let Some(node) = self.tree.selected_node() {
            if node.kind == crate::tree::NodeKind::Directory {
                node.path.clone()
            } else {
                node.path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| self.root_path.clone())
            }
        } else {
            self.root_path.clone()
        };

        // 複数ファイルのペースト処理を開始
        self.paste_files(src_paths, dest_dir, operation);
    }

    /// 複数ファイルのペースト処理
    fn paste_files(&mut self, mut sources: Vec<PathBuf>, dest_dir: PathBuf, operation: ClipboardOperation) {
        while let Some(src_path) = sources.pop() {
            let file_name = src_path.file_name().unwrap_or_default();
            let dest_path = dest_dir.join(file_name);

            // 同じパスの場合はスキップ
            if src_path == dest_path {
                continue;
            }

            // コンフリクトチェック
            if dest_path.exists() {
                // 確認モードに移行
                self.paste_conflict = Some(PasteConflict {
                    remaining_sources: sources,
                    current_src: src_path,
                    current_dest: dest_path,
                    dest_dir,
                    operation,
                });
                self.input_mode = InputMode::ConfirmPaste;
                return;
            }

            // 操作を実行
            self.execute_paste_single(&src_path, &dest_path, operation);
        }

        // 全ファイルの処理完了
        if operation == ClipboardOperation::Cut {
            self.clipboard = None;
        }
        self.rebuild_tree();
    }

    /// 単一ファイルのペースト操作を実行する（リビルドなし）
    fn execute_paste_single(&mut self, src_path: &Path, dest_path: &Path, operation: ClipboardOperation) {
        let result = match operation {
            ClipboardOperation::Copy => copy_recursive(src_path, dest_path),
            ClipboardOperation::Cut => std::fs::rename(src_path, dest_path),
        };

        if result.is_err() {
            // エラー時はログ出力（将来的にはユーザーに通知）
            tracing::warn!("Failed to paste: {:?} -> {:?}", src_path, dest_path);
        }
    }

    /// 上書きペーストを実行し、残りのファイルを続行
    fn execute_paste_overwrite_and_continue(&mut self) {
        if let Some(conflict) = self.paste_conflict.take() {
            // 既存のファイル/ディレクトリを削除
            let remove_result = if conflict.current_dest.is_dir() {
                std::fs::remove_dir_all(&conflict.current_dest)
            } else {
                std::fs::remove_file(&conflict.current_dest)
            };

            if remove_result.is_ok() {
                self.execute_paste_single(&conflict.current_src, &conflict.current_dest, conflict.operation);
            }

            // 残りのファイルを処理
            self.paste_files(conflict.remaining_sources, conflict.dest_dir, conflict.operation);
        }
    }

    /// リネームしてペーストを実行し、残りのファイルを続行
    fn execute_paste_rename_and_continue(&mut self) {
        if let Some(conflict) = self.paste_conflict.take() {
            // 新しい名前を生成
            let new_dest_path = generate_unique_path(&conflict.current_dest);
            self.execute_paste_single(&conflict.current_src, &new_dest_path, conflict.operation);

            // 残りのファイルを処理
            self.paste_files(conflict.remaining_sources, conflict.dest_dir, conflict.operation);
        }
    }

    /// スキップして残りのファイルを続行
    fn skip_paste_and_continue(&mut self) {
        if let Some(conflict) = self.paste_conflict.take() {
            // 残りのファイルを処理
            self.paste_files(conflict.remaining_sources, conflict.dest_dir, conflict.operation);
        }
    }

    /// Git ステータスを再適用する
    fn refresh_git_status(&mut self) {
        self.tree.update_git_status(
            |path| self.git_cache.get_status(path),
            |path| self.git_cache.get_directory_summary(path),
        );
    }

    /// 隠しファイル表示をトグルする
    fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.rebuild_tree();
    }

    /// gitignore されたファイル表示をトグルする
    fn toggle_ignored(&mut self) {
        self.show_ignored = !self.show_ignored;
        self.rebuild_tree();
    }

    /// ソート順を次に切り替える
    fn cycle_sort_order(&mut self) {
        self.sort_config.order = self.sort_config.order.next();
        self.rebuild_tree();
    }

    /// ソート方向をトグルする
    fn toggle_sort_direction(&mut self) {
        self.sort_config.direction = self.sort_config.direction.toggle();
        self.rebuild_tree();
    }

    /// ツリーを再構築する
    fn rebuild_tree(&mut self) {
        // 現在の状態を保存
        let current_path = self.tree.selected_node().map(|n| n.path.clone());
        let expanded_paths = self.tree.get_expanded_paths();

        // ツリーを再構築
        if let Some(root) = TreeBuilder::new()
            .show_hidden(self.show_hidden)
            .show_ignored(self.show_ignored)
            .sort_config(self.sort_config)
            .build(&self.root_path)
        {
            self.tree = TreeState::new(root);

            // 展開状態を復元
            self.tree.expand_paths(&expanded_paths);

            self.refresh_git_status();

            // フィルタを再適用
            if !self.search_input.is_empty() {
                self.tree.set_filter(self.search_input.text());
            }

            // 可能なら以前の選択を復元
            if let Some(path) = current_path {
                self.tree.select_path(&path);
            }

            self.update_preview();
        }
    }
}

/// ファイルまたはディレクトリを再帰的にコピーする
fn copy_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dest)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            copy_recursive(&src_path, &dest_path)?;
        }
    } else {
        std::fs::copy(src, dest)?;
    }
    Ok(())
}

/// コンフリクト時に一意のパスを生成する
///
/// `file.txt` -> `file_1.txt` -> `file_2.txt` ...
/// `dir` -> `dir_1` -> `dir_2` ...
fn generate_unique_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let extension = path.extension().and_then(|s| s.to_str());

    let mut counter = 1;
    loop {
        let new_name = if let Some(ext) = extension {
            format!("{}_{}.{}", stem, counter, ext)
        } else {
            format!("{}_{}", stem, counter)
        };

        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}
