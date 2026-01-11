//! ファイルプレビュー
//!
//! 選択されたファイルの内容をプレビュー表示するための機能を提供する。

use std::fs;
use std::io::{
    BufRead,
    BufReader,
};
use std::path::{
    Path,
    PathBuf,
};

use crate::highlight::{
    HighlightedLine,
    Highlighter,
};

/// プレビューコンテンツ
#[derive(Debug)]
pub(crate) enum PreviewContent {
    /// テキストファイル
    Text {
        /// ハイライトされた行
        highlighted_lines: Vec<HighlightedLine>,
        /// ファイルの総行数
        total_lines: usize,
    },
    /// バイナリファイル
    Binary {
        /// ファイルサイズ
        size: u64,
    },
    /// ディレクトリ
    Directory {
        /// 子エントリ数
        children_count: usize,
    },
    /// エラー
    Error(String),
    /// 空（何も選択されていない）
    Empty,
}

/// プレビュー状態
#[derive(Debug)]
pub(crate) struct PreviewState {
    /// プレビューコンテンツ
    pub(crate) content: PreviewContent,
    /// スクロールオフセット
    pub(crate) scroll_offset: usize,
    /// 現在プレビュー中のパス
    current_path: Option<PathBuf>,
    /// シンタックスハイライター
    highlighter: Highlighter,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewState {
    /// 新しい `PreviewState` を作成する
    pub(crate) fn new() -> Self {
        Self {
            content: PreviewContent::Empty,
            scroll_offset: 0,
            current_path: None,
            highlighter: Highlighter::new(),
        }
    }

    /// 指定パスのプレビューを読み込む
    pub(crate) fn load(&mut self, path: &Path) {
        // 同じパスの場合はスキップ
        if self.current_path.as_ref() == Some(&path.to_path_buf()) {
            return;
        }

        self.current_path = Some(path.to_path_buf());
        self.scroll_offset = 0;

        self.content = self.load_content(path);
    }

    /// ファイルの内容を読み込む
    fn load_content(&self, path: &Path) -> PreviewContent {
        // ディレクトリの場合
        if path.is_dir() {
            return match fs::read_dir(path) {
                Ok(entries) => PreviewContent::Directory { children_count: entries.count() },
                Err(e) => PreviewContent::Error(e.to_string()),
            };
        }

        // ファイルサイズを取得
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) => return PreviewContent::Error(e.to_string()),
        };

        let size = metadata.len();

        // 大きすぎるファイルはバイナリとして扱う
        const MAX_SIZE: u64 = 1024 * 1024; // 1MB
        if size > MAX_SIZE {
            return PreviewContent::Binary { size };
        }

        // ファイルを開いて読み込む
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) => return PreviewContent::Error(e.to_string()),
        };

        let reader = BufReader::new(file);
        let mut lines = Vec::new();
        let mut total_lines = 0;
        let mut is_binary = false;

        const MAX_LINES: usize = 1000;

        for line_result in reader.lines() {
            match line_result {
                Ok(line) => {
                    // バイナリファイルの検出（NUL バイトを含む場合）
                    if line.contains('\0') {
                        is_binary = true;
                        break;
                    }

                    total_lines += 1;
                    if lines.len() < MAX_LINES {
                        lines.push(line);
                    }
                }
                Err(_) => {
                    // 読み込みエラーはバイナリとして扱う
                    is_binary = true;
                    break;
                }
            }
        }

        if is_binary {
            PreviewContent::Binary { size }
        } else {
            // シンタックスハイライトを適用
            let content = lines.join("\n");
            let highlighted_lines = self.highlighter.highlight_file(path, &content);
            PreviewContent::Text { highlighted_lines, total_lines }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::fs;

    use googletest::prelude::*;
    use rstest::*;
    use tempfile::TempDir;

    use super::*;

    #[rstest]
    fn load_text_file_parses_lines() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line1\nline2\nline3").unwrap();

        let mut state = PreviewState::new();
        state.load(&file_path);

        let PreviewContent::Text { highlighted_lines, total_lines } = &state.content else {
            panic!("Expected Text content");
        };

        assert_that!(*total_lines, eq(3));
        assert_that!(highlighted_lines, len(eq(3)));
    }

    #[rstest]
    fn load_directory_counts_children() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file1.txt"), "").unwrap();
        fs::write(dir.path().join("file2.txt"), "").unwrap();

        let mut state = PreviewState::new();
        state.load(dir.path());

        let PreviewContent::Directory { children_count } = &state.content else {
            panic!("Expected Directory content");
        };

        assert_that!(*children_count, eq(2));
    }
}
