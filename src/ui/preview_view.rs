//! PreviewView ウィジェット
//!
//! ファイルのプレビューを表示するウィジェット。

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{
    Color,
    Style,
};
use ratatui::text::{
    Line,
    Span,
};
use ratatui::widgets::{
    Block,
    Borders,
    Widget,
};

use crate::highlight::HighlightedLine;
use crate::preview::{
    PreviewContent,
    PreviewState,
};

/// PreviewView ウィジェット
///
/// 選択されたファイルのプレビューを表示する。
pub(crate) struct PreviewView<'a> {
    /// プレビュー状態への参照
    state: &'a PreviewState,
    /// タイトル（ファイル名）
    title: Option<String>,
}

impl<'a> PreviewView<'a> {
    /// 新しい `PreviewView` を作成する
    pub(crate) fn new(state: &'a PreviewState) -> Self {
        Self { state, title: None }
    }

    /// タイトルを設定する
    pub(crate) fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }
}

impl Widget for &PreviewView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ボーダーを描画
        let title =
            self.title.as_ref().map_or_else(|| " Preview ".to_string(), |t| format!(" {} ", t));

        let block = Block::default().borders(Borders::ALL).title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        match &self.state.content {
            PreviewContent::Text { highlighted_lines, total_lines } => {
                self.render_text(inner, buf, highlighted_lines, *total_lines);
            }
            PreviewContent::Binary { size } => {
                self.render_binary(inner, buf, *size);
            }
            PreviewContent::Directory { children_count } => {
                self.render_directory(inner, buf, *children_count);
            }
            PreviewContent::Error(msg) => {
                self.render_error(inner, buf, msg);
            }
            PreviewContent::Empty => {
                self.render_empty(inner, buf);
            }
        }
    }
}

impl PreviewView<'_> {
    /// テキストコンテンツを描画する
    fn render_text(
        &self,
        area: Rect,
        buf: &mut Buffer,
        highlighted_lines: &[HighlightedLine],
        total_lines: usize,
    ) {
        let scroll_offset = self.state.scroll_offset;
        let line_num_width = total_lines.to_string().len();

        for (i, spans) in
            highlighted_lines.iter().skip(scroll_offset).take(area.height as usize).enumerate()
        {
            let y = area.y + i as u16;
            let line_num = scroll_offset + i + 1;

            // 行番号
            let num_str = format!("{:>width$} ", line_num, width = line_num_width);
            let num_style = Style::default().fg(Color::DarkGray);

            buf.set_string(area.x, y, &num_str, num_style);

            // コンテンツ（ハイライト付き）
            let content_x = area.x + num_str.len() as u16;
            let content_width = area.width.saturating_sub(num_str.len() as u16);

            if content_width > 0 {
                self.render_highlighted_line(buf, content_x, y, content_width, spans);
            }
        }
    }

    /// ハイライトされた行を描画する
    fn render_highlighted_line(
        &self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        max_width: u16,
        spans: &HighlightedLine,
    ) {
        let mut current_x = x;
        let mut remaining_width = max_width as usize;

        for span in spans {
            if remaining_width == 0 {
                break;
            }

            // テキストを残り幅で切り詰め
            let text: String = span.text.chars().take(remaining_width).collect();
            let text_width = text.len();

            buf.set_string(current_x, y, &text, span.style);

            current_x += text_width as u16;
            remaining_width = remaining_width.saturating_sub(text_width);
        }
    }

    /// バイナリファイルを描画する
    fn render_binary(&self, area: Rect, buf: &mut Buffer, size: u64) {
        let size_str = format_size(size);
        let message = format!("Binary file ({size_str})");

        let line = Line::from(vec![Span::styled(message, Style::default().fg(Color::DarkGray))]);

        let y = area.y + area.height / 2;
        let x = area.x + (area.width.saturating_sub(line.width() as u16)) / 2;

        buf.set_line(x, y, &line, area.width);
    }

    /// ディレクトリを描画する
    fn render_directory(&self, area: Rect, buf: &mut Buffer, children_count: usize) {
        let message = format!("Directory ({children_count} items)");

        let line = Line::from(vec![Span::styled(message, Style::default().fg(Color::Blue))]);

        let y = area.y + area.height / 2;
        let x = area.x + (area.width.saturating_sub(line.width() as u16)) / 2;

        buf.set_line(x, y, &line, area.width);
    }

    /// エラーを描画する
    fn render_error(&self, area: Rect, buf: &mut Buffer, msg: &str) {
        let line = Line::from(vec![Span::styled(
            format!("Error: {msg}"),
            Style::default().fg(Color::Red),
        )]);

        let y = area.y + area.height / 2;
        let x = area.x + (area.width.saturating_sub(line.width() as u16)) / 2;

        buf.set_line(x, y, &line, area.width);
    }

    /// 空の状態を描画する
    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let message = "No file selected";

        let line = Line::from(vec![Span::styled(message, Style::default().fg(Color::DarkGray))]);

        let y = area.y + area.height / 2;
        let x = area.x + (area.width.saturating_sub(line.width() as u16)) / 2;

        buf.set_line(x, y, &line, area.width);
    }
}

/// ファイルサイズを人間が読みやすい形式にフォーマットする
///
/// 浮動小数点演算を避けるため、整数演算で小数第1位まで計算する。
fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        let scaled = size * 10 / GB;
        format!("{}.{} GB", scaled / 10, scaled % 10)
    } else if size >= MB {
        let scaled = size * 10 / MB;
        format!("{}.{} MB", scaled / 10, scaled % 10)
    } else if size >= KB {
        let scaled = size * 10 / KB;
        format!("{}.{} KB", scaled / 10, scaled % 10)
    } else {
        format!("{size} B")
    }
}
