//! TreeView ウィジェット
//!
//! ファイルツリーを表示するウィジェット。

use std::path::Path;

use devicons::FileIcon;
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
    Widget,
};

use crate::git::GitStatus;
use crate::tree::{
    NodeKind,
    TreeState,
};

/// TreeView ウィジェット
///
/// ファイルツリーを表示する。
pub(crate) struct TreeView<'a> {
    /// ツリー状態への参照
    state: &'a TreeState,
}

impl<'a> TreeView<'a> {
    /// 新しい `TreeView` を作成する
    pub(crate) fn new(state: &'a TreeState) -> Self {
        Self { state }
    }
}

impl Widget for &TreeView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ボーダーを描画
        let block = Block::default().borders(Borders::ALL).title(" Files ");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let visible_nodes = self.state.visible_nodes();
        let selected = self.state.selected();
        let scroll_offset = self.state.scroll_offset();

        // 表示範囲内のノードを描画
        for (i, node) in
            visible_nodes.iter().skip(scroll_offset).take(inner.height as usize).enumerate()
        {
            let y = inner.y + i as u16;
            let is_selected = scroll_offset + i == selected;

            // インデント
            let indent = "  ".repeat(node.depth);

            // 展開/折り畳みインジケーター
            let expand_indicator = match (node.kind, node.expanded, node.has_children) {
                (NodeKind::Directory, true, _) => "▼ ",
                (NodeKind::Directory, false, true) => "▶ ",
                (NodeKind::Directory, false, false) => "  ",
                _ => "  ",
            };

            // ファイル/フォルダアイコン
            let (file_icon, icon_color) = get_file_icon(&node.path, node.kind);

            // Git ステータスバッジ
            let git_badge =
                node.git_status.map(|s| format!(" {}", s.as_char())).unwrap_or_default();

            // スタイル
            let style = if is_selected {
                Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                match node.kind {
                    NodeKind::Directory => Style::default().fg(Color::Blue),
                    NodeKind::Symlink => Style::default().fg(Color::Magenta),
                    NodeKind::File => Style::default(),
                }
            };

            // Git ステータスの色
            let git_style = if is_selected {
                style
            } else {
                match node.git_status {
                    Some(GitStatus::Modified) => Style::default().fg(Color::Yellow),
                    Some(GitStatus::Added) => Style::default().fg(Color::Green),
                    Some(GitStatus::Deleted) => Style::default().fg(Color::Red),
                    Some(GitStatus::Untracked) => Style::default().fg(Color::Gray),
                    Some(GitStatus::Conflicted) => {
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                    }
                    Some(GitStatus::Ignored) => Style::default().fg(Color::DarkGray),
                    Some(GitStatus::Renamed) => Style::default().fg(Color::Blue),
                    None => Style::default(),
                }
            };

            // アイコンのスタイル
            let icon_style = if is_selected { style } else { Style::default().fg(icon_color) };

            // 行を構築
            let line = Line::from(vec![
                Span::raw(indent),
                Span::styled(expand_indicator, style),
                Span::styled(format!("{} ", file_icon), icon_style),
                Span::styled(&node.name, style),
                Span::styled(git_badge, git_style),
            ]);

            // 描画（幅を超えないようにクリップ）
            let line_width = line.width();
            let available_width = inner.width as usize;

            if line_width <= available_width {
                buf.set_line(inner.x, y, &line, inner.width);
            } else {
                // 幅を超える場合は切り詰め
                let mut x = inner.x;
                for span in line.spans {
                    let span_width = span.width();
                    let remaining = (inner.x + inner.width).saturating_sub(x) as usize;

                    if remaining == 0 {
                        break;
                    }

                    let content: String = span.content.chars().take(remaining).collect();

                    buf.set_string(x, y, &content, span.style);
                    x += span_width.min(remaining) as u16;
                }
            }
        }
    }
}

/// ファイルアイコンと色を取得する
fn get_file_icon(path: &Path, kind: NodeKind) -> (char, Color) {
    match kind {
        NodeKind::Directory => {
            // フォルダアイコン
            ('\u{f07b}', Color::Blue) // nf-fa-folder
        }
        NodeKind::Symlink => {
            // シンボリックリンクアイコン
            ('\u{f0c1}', Color::Magenta) // nf-fa-link
        }
        NodeKind::File => {
            // devicons からアイコンを取得
            let file_icon = FileIcon::from(path);
            let icon = file_icon.icon;
            let color = parse_hex_color(file_icon.color).unwrap_or(Color::White);
            (icon, color)
        }
    }
}

/// 16進数カラー文字列を ratatui の Color に変換する
fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
    let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
    let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;

    Some(Color::Rgb(r, g, b))
}
