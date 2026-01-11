//! シンタックスハイライト
//!
//! syntect + two-face を使用してファイルのシンタックスハイライトを行う。

use std::path::Path;

use ratatui::style::{
    Color,
    Style,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    self,
    ThemeSet,
};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// ハイライトされたスパン
#[derive(Debug, Clone)]
pub(crate) struct HighlightedSpan {
    /// テキスト
    pub(crate) text: String,
    /// スタイル
    pub(crate) style: Style,
}

/// ハイライトされた行
pub(crate) type HighlightedLine = Vec<HighlightedSpan>;

/// シンタックスハイライター
#[derive(Debug)]
pub(crate) struct Highlighter {
    /// 構文セット
    syntax_set: SyntaxSet,
    /// テーマセット
    theme_set: ThemeSet,
    /// 使用するテーマ名
    theme_name: String,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    /// 新しい `Highlighter` を作成する
    ///
    /// two-face の拡張構文セット（TOML, TypeScript 等を含む）を使用する。
    pub(crate) fn new() -> Self {
        Self {
            syntax_set: two_face::syntax::extra_newlines(),
            theme_set: ThemeSet::load_defaults(),
            theme_name: "base16-ocean.dark".to_string(),
        }
    }

    /// ファイルをハイライトする
    pub(crate) fn highlight_file(
        &self,
        path: &Path,
        content: &str,
    ) -> Vec<HighlightedLine> {
        // 拡張子から構文を検出
        let syntax = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| self.syntax_set.find_syntax_by_extension(ext))
            .or_else(|| {
                // ファイル名から検出を試みる
                path.file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| self.syntax_set.find_syntax_by_extension(name))
            })
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let Some(theme) = self.theme_set.themes.get(&self.theme_name) else {
            // テーマが見つからない場合はプレーンテキストとして返す
            return Self::plain_text_lines(content);
        };

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut result = Vec::new();

        for line in LinesWithEndings::from(content) {
            let Ok(ranges) = highlighter.highlight_line(line, &self.syntax_set) else {
                // ハイライト失敗時はプレーンテキストとして追加
                result.push(vec![HighlightedSpan {
                    text: line.trim_end_matches('\n').to_string(),
                    style: Style::default(),
                }]);
                continue;
            };

            let spans: Vec<HighlightedSpan> = ranges
                .into_iter()
                .map(|(style, text)| HighlightedSpan {
                    text: text.trim_end_matches('\n').to_string(),
                    style: Self::convert_style(style),
                })
                .collect();

            result.push(spans);
        }

        result
    }

    /// プレーンテキストとして行を返す
    fn plain_text_lines(content: &str) -> Vec<HighlightedLine> {
        content
            .lines()
            .map(|line| {
                vec![HighlightedSpan {
                    text: line.to_string(),
                    style: Style::default(),
                }]
            })
            .collect()
    }

    /// syntect のスタイルを ratatui のスタイルに変換する
    fn convert_style(syntect_style: highlighting::Style) -> Style {
        let fg = Self::convert_color(syntect_style.foreground);
        Style::default().fg(fg)
    }

    /// syntect の色を ratatui の色に変換する
    fn convert_color(color: highlighting::Color) -> Color {
        Color::Rgb(color.r, color.g, color.b)
    }

    /// 利用可能な構文の一覧を取得する（デバッグ用）
    #[cfg(test)]
    pub(crate) fn list_syntaxes(&self) -> Vec<(String, Vec<String>)> {
        self.syntax_set
            .syntaxes()
            .iter()
            .map(|s| (s.name.clone(), s.file_extensions.clone()))
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::Path;

    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[fixture]
    fn highlighter() -> Highlighter {
        Highlighter::new()
    }

    #[rstest]
    fn syntax_set_includes_toml(highlighter: Highlighter) {
        let syntaxes = highlighter.list_syntaxes();

        let has_toml = syntaxes.iter().any(|(name, exts)| {
            name.to_lowercase().contains("toml")
                || exts.iter().any(|e| e.to_lowercase() == "toml")
        });

        assert_that!(has_toml, eq(true));
    }

    #[rstest]
    fn syntax_set_includes_typescript(highlighter: Highlighter) {
        let syntaxes = highlighter.list_syntaxes();

        let has_typescript = syntaxes.iter().any(|(name, exts)| {
            name.to_lowercase().contains("typescript")
                || exts.iter().any(|e| e.to_lowercase() == "ts")
        });

        assert_that!(has_typescript, eq(true));
    }

    #[rstest]
    fn highlight_file_returns_lines(highlighter: Highlighter) {
        let content = "fn main() {\n    println!(\"Hello\");\n}";
        let path = Path::new("test.rs");

        let result = highlighter.highlight_file(path, content);

        assert_that!(result, len(eq(3)));
    }

    #[rstest]
    fn highlight_file_applies_styles(highlighter: Highlighter) {
        let content = "fn main() {}";
        let path = Path::new("test.rs");

        let result = highlighter.highlight_file(path, content);

        // Rust のシンタックスハイライトが適用され、複数のスパンに分割される
        assert_that!(result.len(), eq(1));
        assert_that!(result[0].len(), gt(1)); // 単一スパンではない
    }
}
