//! テキスト入力バッファ
//!
//! カーソル位置を管理し、Emacs 風のキーバインドをサポートする。

/// テキスト入力バッファ
///
/// カーソル位置を管理し、単語単位の操作をサポートする。
#[derive(Debug, Default, Clone)]
pub(crate) struct InputBuffer {
    /// テキスト内容
    text: String,
    /// カーソル位置（文字単位）
    cursor: usize,
}

impl InputBuffer {
    /// 新しい空のバッファを作成
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// テキストを取得
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// カーソル位置を取得（文字単位）- テスト用
    #[cfg(test)]
    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }

    /// バッファが空かどうか
    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// バッファをクリア
    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// テキストを設定（カーソルは末尾へ）
    pub(crate) fn set(&mut self, text: String) {
        self.cursor = text.chars().count();
        self.text = text;
    }

    /// テキストを取り出してクリア
    pub(crate) fn take(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }

    /// カーソル位置に文字を挿入
    pub(crate) fn insert(&mut self, c: char) {
        let byte_pos = self.cursor_byte_pos();
        self.text.insert(byte_pos, c);
        self.cursor += 1;
    }

    /// カーソル前の1文字を削除 (Backspace, C-h)
    pub(crate) fn backspace(&mut self) {
        if self.cursor > 0 {
            let byte_pos = self.cursor_byte_pos();
            let prev_byte_pos = self.char_to_byte_pos(self.cursor - 1);
            self.text.drain(prev_byte_pos..byte_pos);
            self.cursor -= 1;
        }
    }

    /// カーソル前の単語を削除 (C-w)
    pub(crate) fn delete_word(&mut self) {
        if self.cursor > 0 {
            let chars: Vec<char> = self.text.chars().collect();
            let mut new_cursor = self.cursor;

            // 区切り文字（空白と/）をスキップ
            while new_cursor > 0
                && chars
                    .get(new_cursor - 1)
                    .is_some_and(|c| c.is_whitespace() || *c == '/')
            {
                new_cursor -= 1;
            }

            // 単語をスキップ（区切り文字まで）
            while new_cursor > 0
                && chars
                    .get(new_cursor - 1)
                    .is_some_and(|c| !c.is_whitespace() && *c != '/')
            {
                new_cursor -= 1;
            }

            // 削除
            let start_byte = self.char_to_byte_pos(new_cursor);
            let end_byte = self.cursor_byte_pos();
            self.text.drain(start_byte..end_byte);
            self.cursor = new_cursor;
        }
    }

    /// カーソルを左に移動
    pub(crate) fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// カーソルを右に移動
    pub(crate) fn move_right(&mut self) {
        let char_count = self.text.chars().count();
        if self.cursor < char_count {
            self.cursor += 1;
        }
    }

    /// 前の単語へ移動 (M-b, M-Left)
    pub(crate) fn move_word_left(&mut self) {
        if self.cursor > 0 {
            let chars: Vec<char> = self.text.chars().collect();

            // 区切り文字（空白と/）をスキップ
            while self.cursor > 0
                && chars
                    .get(self.cursor - 1)
                    .is_some_and(|c| c.is_whitespace() || *c == '/')
            {
                self.cursor -= 1;
            }

            // 単語をスキップ（区切り文字まで）
            while self.cursor > 0
                && chars
                    .get(self.cursor - 1)
                    .is_some_and(|c| !c.is_whitespace() && *c != '/')
            {
                self.cursor -= 1;
            }
        }
    }

    /// 次の単語へ移動 (M-f, M-Right)
    pub(crate) fn move_word_right(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        let char_count = chars.len();

        // 単語をスキップ（区切り文字まで）
        while self.cursor < char_count
            && chars
                .get(self.cursor)
                .is_some_and(|c| !c.is_whitespace() && *c != '/')
        {
            self.cursor += 1;
        }

        // 区切り文字（空白と/）をスキップ
        while self.cursor < char_count
            && chars
                .get(self.cursor)
                .is_some_and(|c| c.is_whitespace() || *c == '/')
        {
            self.cursor += 1;
        }
    }

    /// 行頭へ移動 (C-a, Home)
    pub(crate) fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// 行末へ移動 (C-e, End)
    pub(crate) fn move_end(&mut self) {
        self.cursor = self.text.chars().count();
    }

    /// 行頭まで削除 (C-u)
    pub(crate) fn delete_to_start(&mut self) {
        if self.cursor > 0 {
            let byte_pos = self.cursor_byte_pos();
            self.text.drain(..byte_pos);
            self.cursor = 0;
        }
    }

    /// カーソル位置のバイトオフセットを計算
    fn cursor_byte_pos(&self) -> usize {
        self.char_to_byte_pos(self.cursor)
    }

    /// 文字位置からバイトオフセットを計算
    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    /// カーソル前後のテキストを分割して取得（表示用）
    pub(crate) fn split_at_cursor(&self) -> (&str, &str) {
        let byte_pos = self.cursor_byte_pos();
        (&self.text[..byte_pos], &self.text[byte_pos..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_backspace() {
        let mut buf = InputBuffer::new();
        buf.insert('a');
        buf.insert('b');
        buf.insert('c');
        assert_eq!(buf.text(), "abc");
        assert_eq!(buf.cursor(), 3);

        buf.backspace();
        assert_eq!(buf.text(), "ab");
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn test_move_and_insert() {
        let mut buf = InputBuffer::new();
        buf.set("hello".to_string());
        buf.move_home();
        buf.insert('X');
        assert_eq!(buf.text(), "Xhello");

        buf.move_end();
        buf.insert('Y');
        assert_eq!(buf.text(), "XhelloY");
    }

    #[test]
    fn test_delete_word() {
        let mut buf = InputBuffer::new();
        buf.set("foo/bar/baz".to_string());
        buf.delete_word();
        assert_eq!(buf.text(), "foo/bar/");

        buf.delete_word();
        assert_eq!(buf.text(), "foo/");

        buf.delete_word();
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn test_move_word() {
        let mut buf = InputBuffer::new();
        buf.set("foo/bar baz".to_string());
        buf.move_home();

        buf.move_word_right();
        assert_eq!(buf.cursor(), 4); // after "foo/"

        buf.move_word_right();
        assert_eq!(buf.cursor(), 8); // after "bar "

        buf.move_word_left();
        assert_eq!(buf.cursor(), 4); // back to start of "bar"

        buf.move_word_left();
        assert_eq!(buf.cursor(), 0); // back to start of "foo"
    }

    #[test]
    fn test_split_at_cursor() {
        let mut buf = InputBuffer::new();
        buf.set("hello".to_string());
        buf.cursor = 2;

        let (before, after) = buf.split_at_cursor();
        assert_eq!(before, "he");
        assert_eq!(after, "llo");
    }
}
