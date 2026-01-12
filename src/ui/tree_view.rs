//! TreeView ウィジェット
//!
//! ファイルツリーを表示するウィジェット。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

use crate::app::ClipboardOperation;
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
    /// マークされたパス
    marked_paths: &'a HashSet<PathBuf>,
    /// クリップボード（コピー/カット中のパス）
    clipboard: Option<&'a (Vec<PathBuf>, ClipboardOperation)>,
}

impl<'a> TreeView<'a> {
    /// 新しい `TreeView` を作成する
    pub(crate) fn new(
        state: &'a TreeState,
        marked_paths: &'a HashSet<PathBuf>,
        clipboard: Option<&'a (Vec<PathBuf>, ClipboardOperation)>,
    ) -> Self {
        Self { state, marked_paths, clipboard }
    }
}

/// メタ情報を非表示にする幅の閾値
const HIDE_META_WIDTH_THRESHOLD: u16 = 60;

impl Widget for &TreeView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ボーダーを描画
        let block = Block::default().borders(Borders::ALL).title(" Files ");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        // 幅が狭い場合はメタ情報を非表示
        let show_meta = inner.width >= HIDE_META_WIDTH_THRESHOLD;

        let visible_nodes = self.state.visible_nodes();
        let selected = self.state.selected();
        let scroll_offset = self.state.scroll_offset();

        // 表示範囲内のノードを描画
        for (i, node) in
            visible_nodes.iter().skip(scroll_offset).take(inner.height as usize).enumerate()
        {
            let y = inner.y + i as u16;
            let is_selected = scroll_offset + i == selected;
            let is_marked = self.marked_paths.contains(&node.path);

            // クリップボード状態を判定
            let clipboard_state = self.clipboard.and_then(|(paths, op)| {
                if paths.contains(&node.path) {
                    Some(*op)
                } else {
                    None
                }
            });

            // マークインジケーター（クリップボード状態も表示）
            let mark_indicator = match (is_marked, clipboard_state) {
                (true, _) => "● ",              // マーク中
                (false, Some(ClipboardOperation::Copy)) => "◎ ",  // コピー中
                (false, Some(ClipboardOperation::Cut)) => "✂ ",   // カット中
                (false, None) => "  ",
            };

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

            // Git ステータスバッジの幅
            let git_badge_width = node.git_status.map(|_| 2).unwrap_or(0); // " X" = 2文字

            // ファイルフラグ（実行可能、シンボリックリンク）
            let flags = get_file_flags(node.kind, node.is_executable, &node.symlink_target);

            // メタ情報（サイズ、更新日時）- 幅が狭い場合は非表示
            let meta = if show_meta {
                format_meta(node.kind, node.size, node.mtime)
            } else {
                String::new()
            };

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

            // フラグのスタイル
            let flags_style = if is_selected {
                style
            } else {
                Style::default().fg(Color::Cyan)
            };

            // メタ情報のスタイル
            let meta_style = if is_selected {
                style
            } else {
                Style::default().fg(Color::DarkGray)
            };

            // マークスタイル（状態に応じて色を変える）
            let mark_style = if is_selected {
                style
            } else {
                match clipboard_state {
                    Some(ClipboardOperation::Copy) => Style::default().fg(Color::Green),
                    Some(ClipboardOperation::Cut) => Style::default().fg(Color::Yellow),
                    None if is_marked => Style::default().fg(Color::Magenta),
                    None => Style::default(),
                }
            };

            // 利用可能な幅
            let available_width = inner.width as usize;

            // 固定部分の幅を計算（マーク + インデント + 展開インジケータ + アイコン + スペース）
            let prefix_width = mark_indicator.chars().count() + indent.len() + expand_indicator.len() + 2; // icon + space

            // メタ情報の幅（表示する場合は固定幅 + 先頭スペース）
            let meta_width = if meta.is_empty() { 0 } else { META_WIDTH + 1 };

            // ファイル名＋フラグ＋Gitバッジに使える幅
            let content_available = available_width.saturating_sub(prefix_width + meta_width);

            // フラグとGitバッジの幅
            let suffix_width = flags.len() + git_badge_width;

            // ファイル名に使える幅
            let name_available = content_available.saturating_sub(suffix_width);

            // ファイル名を省略（必要に応じて）
            let truncated_name = truncate_name(&node.name, name_available);

            // 左側コンテンツの実際の幅
            let left_content_width =
                prefix_width + truncated_name.chars().count() + flags.len() + git_badge_width;

            // メタ情報を右寄せするためのパディング
            let padding_width = if meta.is_empty() {
                0
            } else {
                available_width.saturating_sub(left_content_width + META_WIDTH)
            };
            let padding = " ".repeat(padding_width);

            // 残り幅でメタ情報を調整（幅が足りない場合は非表示）
            let meta_display = if left_content_width + meta_width <= available_width {
                meta.clone()
            } else {
                String::new()
            };

            // 残り幅でフラグを調整
            let flags_display = if prefix_width + truncated_name.chars().count() + flags.len()
                <= available_width
            {
                flags.clone()
            } else {
                String::new()
            };

            // 行を構築
            let mut spans = vec![
                Span::styled(mark_indicator, mark_style),
                Span::raw(indent),
                Span::styled(expand_indicator, style),
                Span::styled(format!("{} ", file_icon), icon_style),
                Span::styled(truncated_name, style),
                Span::styled(flags_display, flags_style),
            ];

            // Git バッジを追加
            if let Some(status) = node.git_status {
                spans.push(Span::styled(format!(" {}", status.as_char()), git_style));
            }

            spans.push(Span::raw(padding));
            spans.push(Span::styled(meta_display, meta_style));

            let line = Line::from(spans);

            buf.set_line(inner.x, y, &line, inner.width);
        }
    }
}

/// ファイル名を指定幅に収まるように省略する
///
/// 幅が足りない場合は末尾に "…" を付けて切り詰める
fn truncate_name(name: &str, max_width: usize) -> String {
    let name_width = name.chars().count();

    if name_width <= max_width {
        name.to_string()
    } else if max_width <= 1 {
        "…".to_string()
    } else {
        let truncated: String = name.chars().take(max_width - 1).collect();
        format!("{}…", truncated)
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

/// メタ情報の固定幅（サイズ6 + スペース1 + 日付10）
const META_WIDTH: usize = 17;

/// メタ情報をフォーマットする（右寄せ固定幅）
///
/// ファイルの場合: `  4.2K 2024/01/15`（17文字固定）
/// ディレクトリの場合: 空文字列
fn format_meta(kind: NodeKind, size: u64, mtime: Option<SystemTime>) -> String {
    // ディレクトリの場合はメタ情報を表示しない
    if kind == NodeKind::Directory {
        return String::new();
    }

    let size_str = format_size(size);
    let mtime_str = mtime.map(format_mtime).unwrap_or_else(|| "----------".to_string());

    format!("{} {}", size_str, mtime_str)
}

/// ファイルサイズを固定幅6文字でフォーマットする
///
/// - `  892B` (バイト)
/// - `  4.2K` (キロバイト)
/// - `123.4M` (メガバイト)
/// - `  1.0G` (ギガバイト)
fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        let scaled = size * 10 / GB;
        format!("{:>5}G", format!("{}.{}", scaled / 10, scaled % 10))
    } else if size >= MB {
        let scaled = size * 10 / MB;
        format!("{:>5}M", format!("{}.{}", scaled / 10, scaled % 10))
    } else if size >= KB {
        let scaled = size * 10 / KB;
        format!("{:>5}K", format!("{}.{}", scaled / 10, scaled % 10))
    } else {
        format!("{:>5}B", size)
    }
}

/// 更新日時を yyyy/MM/dd 形式でフォーマットする（10文字固定）
fn format_mtime(mtime: SystemTime) -> String {
    let duration = mtime.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();

    // 簡易的な日時計算（UTC）
    let days = secs / 86400;
    let years = 1970 + days / 365;
    let remaining_days = days % 365;
    let months = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;

    format!("{:04}/{:02}/{:02}", years, months.min(12), day.min(31))
}

/// ファイルフラグ文字列を取得する
///
/// - 実行可能ファイル: `*`
/// - シンボリックリンク: ` -> target`
fn get_file_flags(kind: NodeKind, is_executable: bool, symlink_target: &Option<String>) -> String {
    let mut flags = String::new();

    // 実行可能ファイル
    if is_executable && kind == NodeKind::File {
        flags.push('*');
    }

    // シンボリックリンク
    if kind == NodeKind::Symlink
        && let Some(target) = symlink_target
    {
        flags.push_str(" -> ");
        flags.push_str(target);
    }

    flags
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "    0B");
        assert_eq!(format_size(892), "  892B");
        assert_eq!(format_size(1023), " 1023B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "  1.0K");
        assert_eq!(format_size(1024 * 4 + 204), "  4.1K"); // ~4.2K
        assert_eq!(format_size(1024 * 999), "999.0K");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "  1.0M");
        assert_eq!(format_size(1024 * 1024 * 123 + 1024 * 1024 / 2), "123.5M");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "  1.0G");
        assert_eq!(format_size(1024 * 1024 * 1024 * 2), "  2.0G");
    }

    #[test]
    fn test_format_size_fixed_width() {
        // すべて6文字幅であることを確認
        assert_eq!(format_size(0).len(), 6);
        assert_eq!(format_size(892).len(), 6);
        assert_eq!(format_size(1024).len(), 6);
        assert_eq!(format_size(1024 * 1024).len(), 6);
        assert_eq!(format_size(1024 * 1024 * 1024).len(), 6);
    }
}
