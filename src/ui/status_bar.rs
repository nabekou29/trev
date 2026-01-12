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

        // モーダルで表示されるモードの場合は通常表示
        // (モーダル側で詳細を表示するため)

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
        let filter_indicator = if !self.app.search_input.is_empty() {
            format!(" [filter: {}]", self.app.search_input.text())
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

        // クエリ（カーソル位置を反映）
        let (before, after) = self.app.search_input.split_at_cursor();
        let query_before = Span::styled(before, bg_style);
        let cursor = Span::styled("_", bg_style.add_modifier(Modifier::SLOW_BLINK));
        let query_after = Span::styled(after, bg_style);

        // ヒント
        let hint = Span::styled(
            " (Enter: confirm, Esc: cancel)",
            bg_style.add_modifier(Modifier::DIM),
        );

        let line = Line::from(vec![prompt, query_before, cursor, query_after, hint]);
        buf.set_line(area.x, area.y, &line, area.width);
    }

}
