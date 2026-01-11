//! アプリケーション状態とイベントループ
//!
//! TUI アプリケーションの中核となる状態管理とイベント処理を提供する。

use std::path::PathBuf;

use crossterm::event::{
    self,
    Event,
    KeyCode,
    KeyEvent,
    KeyEventKind,
};
use ratatui::Frame;

use crate::cli::Args;
use crate::error::Result;
use crate::git::GitCache;
use crate::preview::PreviewState;
use crate::tree::builder::TreeBuilder;
use crate::tree::TreeState;
use crate::ui;

/// アプリケーションの実行状態
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) enum AppState {
    /// 実行中
    #[default]
    Running,
    /// 選択して終了
    Quit,
    /// キャンセル終了
    Cancelled,
}

/// 入力モード
///
/// `gg` などの複合キー入力に対応するための状態。
#[derive(Debug, Default)]
pub(crate) enum InputMode {
    /// 通常モード
    #[default]
    Normal,
    /// `g` が押された状態（`gg` 待ち）
    WaitingForG,
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
    input_mode: InputMode,
    /// 終了時に出力するパス
    pub(crate) selected_path: Option<PathBuf>,
}

impl App {
    /// 新しい `App` を作成する
    pub(crate) fn new(args: &Args) -> Result<Self> {
        // ツリーを構築
        let root = TreeBuilder::new()
            .show_hidden(args.show_hidden)
            .build(&args.path)
            .ok_or_else(|| crate::error::AppError::InvalidPath(args.path.clone()))?;

        let mut tree = TreeState::new(root);

        // Git キャッシュを構築
        let git_cache = GitCache::from_path(&args.path);

        // Git ステータスを反映
        tree.update_git_status(|path| git_cache.get_status(path));

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
            selected_path: None,
        })
    }

    /// アプリケーションが実行中かどうかを返す
    pub(crate) fn is_running(&self) -> bool {
        self.state == AppState::Running
    }

    /// 画面を描画する
    pub(crate) fn draw(&mut self, frame: &mut Frame<'_>) {
        // 表示領域に合わせてスクロールを調整
        let height = frame.area().height.saturating_sub(2) as usize; // ステータスバー分を引く
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
        let prev_selected = self.tree.selected();

        match (&self.input_mode, key.code) {
            // gg: 先頭へ
            (InputMode::WaitingForG, KeyCode::Char('g')) => {
                self.tree.select_first();
                self.input_mode = InputMode::Normal;
            }

            // g 単体押し -> gg 待ち
            (InputMode::Normal, KeyCode::Char('g')) => {
                self.input_mode = InputMode::WaitingForG;
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

            (_, KeyCode::Enter) => {
                self.confirm_selection();
            }

            (_, KeyCode::Char('q') | KeyCode::Esc) => {
                self.state = AppState::Cancelled;
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

    /// Git ステータスを再適用する
    fn refresh_git_status(&mut self) {
        self.tree
            .update_git_status(|path| self.git_cache.get_status(path));
    }
}
