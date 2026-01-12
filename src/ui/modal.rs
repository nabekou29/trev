//! モーダルダイアログウィジェット
//!
//! 画面中央に浮かぶダイアログを表示する。

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
use ratatui::widgets::{
    Block,
    Borders,
    Clear,
    Widget,
};

use crate::app::{App, InputMode};

/// モーダルダイアログウィジェット
pub(crate) struct Modal<'a> {
    /// アプリケーション状態への参照
    app: &'a App,
}

impl<'a> Modal<'a> {
    /// 新しい `Modal` を作成する
    pub(crate) fn new(app: &'a App) -> Self {
        Self { app }
    }

    /// モーダルを表示すべきかどうか
    pub(crate) fn should_show(&self) -> bool {
        matches!(
            self.app.input_mode,
            InputMode::AddingFile
                | InputMode::AddingDirectory
                | InputMode::ConfirmDelete
                | InputMode::Renaming
                | InputMode::ConfirmPaste
        )
    }
}

impl Widget for &Modal<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.app.input_mode {
            InputMode::AddingFile | InputMode::AddingDirectory => {
                self.render_add_modal(area, buf);
            }
            InputMode::ConfirmDelete => {
                self.render_delete_modal(area, buf);
            }
            InputMode::Renaming => {
                self.render_rename_modal(area, buf);
            }
            InputMode::ConfirmPaste => {
                self.render_paste_conflict_modal(area, buf);
            }
            _ => {}
        }
    }
}

impl Modal<'_> {
    /// 中央配置の矩形を計算する
    fn centered_rect(&self, width: u16, height: u16, area: Rect) -> Rect {
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        Rect::new(x, y, width.min(area.width), height.min(area.height))
    }

    /// 追加モーダルを描画
    fn render_add_modal(&self, area: Rect, buf: &mut Buffer) {
        let title = match self.app.input_mode {
            InputMode::AddingFile => " New File ",
            InputMode::AddingDirectory => " New Directory ",
            _ => " New ",
        };

        let width = 50.min(area.width.saturating_sub(4));
        let height = 5;
        let modal_area = self.centered_rect(width, height, area);

        // 背景をクリア
        Clear.render(modal_area, buf);

        // ブロック（枠）を描画
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        // 親ディレクトリ表示
        let parent_str = self
            .app
            .add_target_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let parent_display = truncate_start(&parent_str, inner.width as usize - 2);
        let parent_line = Line::from(vec![Span::styled(
            format!(" {}/", parent_display),
            Style::default().fg(Color::DarkGray),
        )]);
        buf.set_line(inner.x, inner.y, &parent_line, inner.width);

        // 入力フィールド（カーソル位置を反映）
        let (before, after) = self.app.add_input.split_at_cursor();
        let input_line = Line::from(vec![
            Span::styled(" > ", Style::default().fg(Color::Green)),
            Span::raw(before),
            Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
            Span::raw(after),
        ]);
        buf.set_line(inner.x, inner.y + 1, &input_line, inner.width);

        // ヒント
        let hint_line = Line::from(vec![Span::styled(
            " Enter: create | Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )]);
        buf.set_line(inner.x, inner.y + 2, &hint_line, inner.width);
    }

    /// 削除確認モーダルを描画
    fn render_delete_modal(&self, area: Rect, buf: &mut Buffer) {
        let width = 50.min(area.width.saturating_sub(4));
        let height = 6;
        let modal_area = self.centered_rect(width, height, area);

        // 背景をクリア
        Clear.render(modal_area, buf);

        // ブロック（枠）を描画
        let block = Block::default()
            .title(" Delete ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        // 警告メッセージ
        let warning_line = Line::from(vec![Span::styled(
            " Are you sure you want to delete?",
            Style::default().fg(Color::Yellow),
        )]);
        buf.set_line(inner.x, inner.y, &warning_line, inner.width);

        // 対象ファイル表示
        let target_display = if self.app.delete_targets.len() == 1 {
            self.app
                .delete_targets
                .first()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "?".to_string())
        } else {
            format!("{} items", self.app.delete_targets.len())
        };
        let target_display = truncate_middle(&target_display, inner.width as usize - 4);
        let target_line = Line::from(vec![Span::styled(
            format!(" {} ", target_display),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )]);
        buf.set_line(inner.x, inner.y + 1, &target_line, inner.width);

        // ヒント
        let hint_line = Line::from(vec![Span::styled(
            " y: delete | n/Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )]);
        buf.set_line(inner.x, inner.y + 3, &hint_line, inner.width);
    }

    /// リネームモーダルを描画
    fn render_rename_modal(&self, area: Rect, buf: &mut Buffer) {
        let width = 50.min(area.width.saturating_sub(4));
        let height = 6;
        let modal_area = self.centered_rect(width, height, area);

        // 背景をクリア
        Clear.render(modal_area, buf);

        // ブロック（枠）を描画
        let block = Block::default()
            .title(" Rename ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        // 元のファイル名表示
        let original_name = self
            .app
            .rename_target
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let original_line = Line::from(vec![
            Span::styled(" From: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&original_name),
        ]);
        buf.set_line(inner.x, inner.y, &original_line, inner.width);

        // 入力フィールド（カーソル位置を反映）
        let (before, after) = self.app.rename_input.split_at_cursor();
        let input_line = Line::from(vec![
            Span::styled(" To:   ", Style::default().fg(Color::Cyan)),
            Span::raw(before),
            Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
            Span::raw(after),
        ]);
        buf.set_line(inner.x, inner.y + 1, &input_line, inner.width);

        // ヒント
        let hint_line = Line::from(vec![Span::styled(
            " Enter: rename | Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )]);
        buf.set_line(inner.x, inner.y + 3, &hint_line, inner.width);
    }

    /// ペーストコンフリクトモーダルを描画
    fn render_paste_conflict_modal(&self, area: Rect, buf: &mut Buffer) {
        let width = 55.min(area.width.saturating_sub(4));
        let height = 7;
        let modal_area = self.centered_rect(width, height, area);

        // 背景をクリア
        Clear.render(modal_area, buf);

        // ブロック（枠）を描画
        let block = Block::default()
            .title(" Conflict ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        if inner.height < 4 || inner.width < 10 {
            return;
        }

        // 警告メッセージ
        let warning_line = Line::from(vec![Span::styled(
            " File already exists:",
            Style::default().fg(Color::Yellow),
        )]);
        buf.set_line(inner.x, inner.y, &warning_line, inner.width);

        // 対象ファイル名
        let target_name = self
            .app
            .paste_conflict
            .as_ref()
            .and_then(|c| c.current_dest.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "?".to_string());
        let target_display = truncate_middle(&target_name, inner.width as usize - 4);
        let target_line = Line::from(vec![Span::styled(
            format!(" {} ", target_display),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )]);
        buf.set_line(inner.x, inner.y + 1, &target_line, inner.width);

        // 残りファイル数
        if let Some(conflict) = &self.app.paste_conflict {
            let remaining = conflict.remaining_sources.len();
            if remaining > 0 {
                let remaining_line = Line::from(vec![Span::styled(
                    format!(" ({} more files remaining)", remaining),
                    Style::default().fg(Color::DarkGray),
                )]);
                buf.set_line(inner.x, inner.y + 2, &remaining_line, inner.width);
            }
        }

        // ヒント
        let hint_line = Line::from(vec![Span::styled(
            " o: overwrite | r: rename | s: skip | Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )]);
        buf.set_line(inner.x, inner.y + 4, &hint_line, inner.width);
    }
}

/// 文字列を指定幅に収まるように先頭を省略する
fn truncate_start(s: &str, max_width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_width {
        s.to_string()
    } else if max_width <= 3 {
        "...".to_string()
    } else {
        let start = chars.len() - (max_width - 3);
        format!("...{}", chars[start..].iter().collect::<String>())
    }
}

/// 文字列を指定幅に収まるように中央を省略する
fn truncate_middle(s: &str, max_width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_width {
        s.to_string()
    } else if max_width <= 3 {
        "...".to_string()
    } else {
        let half = (max_width - 3) / 2;
        let start: String = chars[..half].iter().collect();
        let end: String = chars[chars.len() - half..].iter().collect();
        format!("{}...{}", start, end)
    }
}
