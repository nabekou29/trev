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

use crate::app::App;

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

        // 左側: 現在のパス
        let path_str = self
            .app
            .tree
            .selected_node()
            .map(|n| n.path.to_string_lossy().to_string())
            .unwrap_or_default();

        let left_span = Span::styled(format!(" {} ", path_str), bg_style);

        // 右側: ヘルプヒント
        let help_str = " j/k:move  h/l:collapse/expand  Enter:select  q:quit ";
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
}
