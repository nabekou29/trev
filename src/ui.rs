//! UI レイアウトと描画
//!
//! ratatui を使用した UI の構成と描画を提供する。

pub(crate) mod modal;
pub(crate) mod preview_view;
pub(crate) mod status_bar;
pub(crate) mod tree_view;

use ratatui::Frame;
use ratatui::layout::{
    Constraint,
    Layout,
};

use crate::app::App;
use crate::ui::modal::Modal;
use crate::ui::preview_view::PreviewView;
use crate::ui::status_bar::StatusBar;
use crate::ui::tree_view::TreeView;

/// メイン画面を描画する
///
/// レイアウト:
/// ```text
/// ┌─────────────────┬──────────────────┐
/// │   TreeView      │   PreviewView    │
/// │   (40%)         │   (60%)          │
/// │                 │                  │
/// ├─────────────────┴──────────────────┤
/// │           StatusBar                │
/// └────────────────────────────────────┘
/// ```
pub(crate) fn render(frame: &mut Frame<'_>, app: &App) {
    let [main_area, status_area] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());

    if app.show_preview {
        // プレビュー表示時: ツリー40% + プレビュー60%
        let [tree_area, preview_area] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .areas(main_area);

        // TreeView を描画
        let tree_view = TreeView::new(&app.tree, &app.marked_paths, app.clipboard.as_ref());
        frame.render_widget(&tree_view, tree_area);

        // PreviewView を描画
        let title = app.tree.selected_node().map(|n| n.name.clone());

        let mut preview_view = PreviewView::new(&app.preview);
        if let Some(t) = title {
            preview_view = preview_view.title(t);
        }
        frame.render_widget(&preview_view, preview_area);
    } else {
        // プレビュー非表示時: ツリー100%
        let tree_view = TreeView::new(&app.tree, &app.marked_paths, app.clipboard.as_ref());
        frame.render_widget(&tree_view, main_area);
    }

    // StatusBar を描画
    let status_bar = StatusBar::new(app);
    frame.render_widget(&status_bar, status_area);

    // モーダルダイアログを描画（必要な場合）
    let modal = Modal::new(app);
    if modal.should_show() {
        frame.render_widget(&modal, frame.area());
    }
}
