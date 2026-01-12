//! StatusBar ウィジェット
//!
//! 画面下部に現在の状態を表示するウィジェット。

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{
    Color,
    Modifier,
    Style,
};
use ratatui::text::{
    Line,
    Span,
};
use ratatui::widgets::Widget;

use crate::app::{App, InputMode};

/// StatusBar ウィジェット
///
/// 現在のパスや操作ヒントを表示する。
pub(crate) struct StatusBar<'a> {
    /// アプリケーション状態への参照
    app: &'a App,
}

impl<'a> StatusBar<'a> {
    /// 新しい `StatusBar` を作成する
    pub(crate) fn new(app: &'a App) -> Self {
        Self { app }
    }
}

impl Widget for &StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        // 背景色
        let bg_style = Style::default().bg(Color::DarkGray).fg(Color::White);

        // 背景を塗りつぶす
        for x in area.x..area.x + area.width {
            buf.set_string(x, area.y, " ", bg_style);
        }

        // 検索モードの場合は検索入力を表示
        if self.app.input_mode == InputMode::Searching {
            self.render_search_mode(area, buf, bg_style);
            return;
        }

        // 削除確認モードの場合
        if self.app.input_mode == InputMode::ConfirmDelete {
            self.render_delete_confirm_mode(area, buf);
            return;
        }

        // リネームモードの場合
        if self.app.input_mode == InputMode::Renaming {
            self.render_rename_mode(area, buf, bg_style);
            return;
        }

        // ペースト確認モードの場合
        if self.app.input_mode == InputMode::ConfirmPaste {
            self.render_paste_confirm_mode(area, buf);
            return;
        }

        // 追加モードの場合
        if matches!(self.app.input_mode, InputMode::AddingFile | InputMode::AddingDirectory) {
            self.render_add_mode(area, buf, bg_style);
            return;
        }

        // 通常モード
        self.render_normal_mode(area, buf, bg_style);
    }
}

impl StatusBar<'_> {
    /// 通常モードの描画
    fn render_normal_mode(&self, area: Rect, buf: &mut Buffer, bg_style: Style) {
        // 左側: 現在のパス
        let path_str = self
            .app
            .tree
            .selected_node()
            .map(|n| n.path.to_string_lossy().to_string())
            .unwrap_or_default();

        // フィルタがアクティブな場合は表示
        let filter_indicator = if !self.app.search_query.is_empty() {
            format!(" [filter: {}]", self.app.search_query)
        } else {
            String::new()
        };

        let left_span = Span::styled(format!(" {}{} ", path_str, filter_indicator), bg_style);

        // 右側: ヘルプヒント
        let help_str = " j/k:move  /:search  p:preview  S:sort  q:quit ";
        let help_span = Span::styled(help_str, bg_style.add_modifier(Modifier::DIM));

        // 左側を描画
        let left_line = Line::from(vec![left_span]);
        buf.set_line(area.x, area.y, &left_line, area.width);

        // 右側を描画（右寄せ）
        let help_width = help_str.len() as u16;
        if area.width > help_width {
            let help_x = area.x + area.width - help_width;
            let help_line = Line::from(vec![help_span]);
            buf.set_line(help_x, area.y, &help_line, help_width);
        }
    }

    /// 検索モードの描画
    fn render_search_mode(&self, area: Rect, buf: &mut Buffer, bg_style: Style) {
        let search_style = Style::default().bg(Color::Yellow).fg(Color::Black);

        // プロンプト
        let prompt = Span::styled(" / ", search_style);

        // クエリ
        let query = Span::styled(&self.app.search_query, bg_style);

        // カーソル（点滅効果は省略、アンダースコアで代用）
        let cursor = Span::styled("_", bg_style.add_modifier(Modifier::SLOW_BLINK));

        // ヒント
        let hint = Span::styled(
            " (Enter: confirm, Esc: cancel)",
            bg_style.add_modifier(Modifier::DIM),
        );

        let line = Line::from(vec![prompt, query, cursor, hint]);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    /// リネームモードの描画
    fn render_rename_mode(&self, area: Rect, buf: &mut Buffer, bg_style: Style) {
        let rename_style = Style::default().bg(Color::Cyan).fg(Color::Black);

        // プロンプト
        let prompt = Span::styled(" RENAME ", rename_style);

        // 入力
        let input = Span::styled(&self.app.rename_input, bg_style);

        // カーソル
        let cursor = Span::styled("_", bg_style.add_modifier(Modifier::SLOW_BLINK));

        // ヒント
        let hint = Span::styled(
            " (Enter: confirm, Esc: cancel)",
            bg_style.add_modifier(Modifier::DIM),
        );

        let line = Line::from(vec![prompt, input, cursor, hint]);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    /// 削除確認モードの描画
    fn render_delete_confirm_mode(&self, area: Rect, buf: &mut Buffer) {
        let warn_style = Style::default().bg(Color::Red).fg(Color::White);
        let text_style = Style::default().bg(Color::Red).fg(Color::White);

        // 警告マーク
        let warning = Span::styled(" DELETE ", warn_style.add_modifier(Modifier::BOLD));

        // 対象ファイル表示
        let target_display = if self.app.delete_targets.len() == 1 {
            self.app.delete_targets.first()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "?".to_string())
        } else {
            format!("{} files", self.app.delete_targets.len())
        };

        let target = Span::styled(format!(" {} ", target_display), text_style);

        // 確認プロンプト
        let prompt = Span::styled("(y/n) ", text_style.add_modifier(Modifier::BOLD));

        let line = Line::from(vec![warning, target, prompt]);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    /// 追加モードの描画
    fn render_add_mode(&self, area: Rect, buf: &mut Buffer, bg_style: Style) {
        let add_style = Style::default().bg(Color::Green).fg(Color::Black);

        // プロンプト
        let prompt_text = match self.app.input_mode {
            InputMode::AddingFile => " NEW FILE ",
            InputMode::AddingDirectory => " NEW DIR ",
            _ => " NEW ",
        };
        let prompt = Span::styled(prompt_text, add_style);

        // 入力
        let input = Span::styled(&self.app.add_input, bg_style);

        // カーソル
        let cursor = Span::styled("_", bg_style.add_modifier(Modifier::SLOW_BLINK));

        // ヒント
        let hint = Span::styled(
            " (Enter: create, Esc: cancel)",
            bg_style.add_modifier(Modifier::DIM),
        );

        let line = Line::from(vec![prompt, input, cursor, hint]);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    /// ペースト確認モードの描画
    fn render_paste_confirm_mode(&self, area: Rect, buf: &mut Buffer) {
        let warn_style = Style::default().bg(Color::Yellow).fg(Color::Black);
        let text_style = Style::default().bg(Color::Yellow).fg(Color::Black);

        // 警告マーク
        let warning = Span::styled(" CONFLICT ", warn_style.add_modifier(Modifier::BOLD));

        // 対象ファイル名
        let target_name = self
            .app
            .paste_conflict
            .as_ref()
            .and_then(|c| c.current_dest.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "?".to_string());

        let target = Span::styled(format!(" '{}' already exists ", target_name), text_style);

        // 確認プロンプト
        let prompt = Span::styled("(o:overwrite / r:rename / s:skip / Esc:cancel) ", text_style.add_modifier(Modifier::BOLD));

        let line = Line::from(vec![warning, target, prompt]);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}
