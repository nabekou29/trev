//! Git ステータス管理
//!
//! `git status --porcelain` を使用してファイルの Git 状態を取得する。

use std::collections::HashMap;
use std::path::{
    Path,
    PathBuf,
};
use std::process::Command;

/// Git ファイルステータス
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitStatus {
    /// 変更あり (M)
    Modified,
    /// 追加 (A)
    Added,
    /// 削除 (D)
    Deleted,
    /// 未追跡 (?)
    Untracked,
    /// コンフリクト (U)
    Conflicted,
    /// 無視 (!)
    Ignored,
    /// リネーム (R)
    Renamed,
}

impl GitStatus {
    /// ステータスを1文字で表現する
    pub(crate) fn as_char(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Untracked => '?',
            Self::Conflicted => 'U',
            Self::Ignored => '!',
            Self::Renamed => 'R',
        }
    }
}

/// ディレクトリのGitステータス集計
///
/// ディレクトリ内のファイルのGitステータスを集計する。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GitStatusSummary {
    /// 変更ファイル数
    pub(crate) modified: usize,
    /// 追加ファイル数
    pub(crate) added: usize,
    /// 削除ファイル数
    pub(crate) deleted: usize,
    /// 未追跡ファイル数
    pub(crate) untracked: usize,
    /// コンフリクトファイル数
    pub(crate) conflicted: usize,
    /// リネームファイル数
    pub(crate) renamed: usize,
}

impl GitStatusSummary {
    /// サマリが空かどうかを返す
    pub(crate) fn is_empty(&self) -> bool {
        self.modified == 0
            && self.added == 0
            && self.deleted == 0
            && self.untracked == 0
            && self.conflicted == 0
            && self.renamed == 0
    }

    /// ステータスを加算する
    fn add_status(&mut self, status: GitStatus) {
        match status {
            GitStatus::Modified => self.modified += 1,
            GitStatus::Added => self.added += 1,
            GitStatus::Deleted => self.deleted += 1,
            GitStatus::Untracked => self.untracked += 1,
            GitStatus::Conflicted => self.conflicted += 1,
            GitStatus::Ignored => {} // 無視ファイルはカウントしない
            GitStatus::Renamed => self.renamed += 1,
        }
    }
}

/// Git ステータスキャッシュ
///
/// リポジトリ内のファイルの Git 状態をキャッシュする。
#[derive(Debug, Default)]
pub(crate) struct GitCache {
    /// パス -> ステータスのマップ
    statuses: HashMap<PathBuf, GitStatus>,
    /// リポジトリルート
    repo_root: Option<PathBuf>,
}

impl GitCache {
    /// 指定パスから Git リポジトリを検出してキャッシュを構築する
    pub(crate) fn from_path(path: &Path) -> Self {
        let mut cache = Self::default();

        // リポジトリルートを検出
        if let Some(root) = Self::find_repo_root(path) {
            cache.repo_root = Some(root.clone());
            cache.load_status(&root);
        }

        cache
    }

    /// リポジトリルートを検出する
    fn find_repo_root(path: &Path) -> Option<PathBuf> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(path)
            .output()
            .ok()?;

        if output.status.success() {
            let root = String::from_utf8_lossy(&output.stdout);
            // パスを正規化して一致させる
            PathBuf::from(root.trim()).canonicalize().ok()
        } else {
            None
        }
    }

    /// Git ステータスを読み込む
    fn load_status(&mut self, repo_root: &Path) {
        let output = Command::new("git")
            .args(["status", "--porcelain=v1", "-z", "--ignored=matching"])
            .current_dir(repo_root)
            .output();

        let Ok(output) = output else {
            return;
        };

        if !output.status.success() {
            return;
        }

        self.parse_porcelain_output(&output.stdout, repo_root);
    }

    /// porcelain 出力をパースする
    fn parse_porcelain_output(&mut self, output: &[u8], repo_root: &Path) {
        let output_str = String::from_utf8_lossy(output);

        // ヌル文字区切りで分割
        for entry in output_str.split('\0') {
            if entry.len() < 3 {
                continue;
            }

            let status_chars: Vec<char> = entry.chars().take(2).collect();
            let Some(index_status) = status_chars.first() else {
                continue;
            };
            let Some(worktree_status) = status_chars.get(1) else {
                continue;
            };

            // パス部分（3文字目以降）
            let path_str = entry.get(3..).unwrap_or_default();
            if path_str.is_empty() {
                continue;
            }

            let full_path = repo_root.join(path_str);
            let status = Self::parse_status(*index_status, *worktree_status);

            if let Some(s) = status {
                self.statuses.insert(full_path, s);
            }
        }
    }

    /// ステータス文字をパースする
    fn parse_status(index: char, worktree: char) -> Option<GitStatus> {
        // 未追跡
        if index == '?' && worktree == '?' {
            return Some(GitStatus::Untracked);
        }

        // 無視
        if index == '!' && worktree == '!' {
            return Some(GitStatus::Ignored);
        }

        // コンフリクト
        if index == 'U' || worktree == 'U' {
            return Some(GitStatus::Conflicted);
        }

        // リネーム
        if index == 'R' || worktree == 'R' {
            return Some(GitStatus::Renamed);
        }

        // 追加
        if index == 'A' {
            return Some(GitStatus::Added);
        }

        // 削除
        if index == 'D' || worktree == 'D' {
            return Some(GitStatus::Deleted);
        }

        // 変更
        if index == 'M' || worktree == 'M' {
            return Some(GitStatus::Modified);
        }

        None
    }

    /// 指定パスの Git ステータスを取得する
    ///
    /// ファイル自体にステータスがない場合、親ディレクトリのステータスを継承する。
    /// （未追跡ディレクトリ内のファイルは個別にリストされないため）
    pub(crate) fn get_status(&self, path: &Path) -> Option<GitStatus> {
        // まず直接のステータスを確認
        if let Some(status) = self.statuses.get(path) {
            return Some(*status);
        }

        // 親ディレクトリを遡ってステータスを探す
        let mut current = path.parent();
        while let Some(parent) = current {
            if let Some(status) = self.statuses.get(parent) {
                // 未追跡または無視のディレクトリは子に継承
                if matches!(status, GitStatus::Untracked | GitStatus::Ignored) {
                    return Some(*status);
                }
            }
            current = parent.parent();
        }

        None
    }

    /// 指定ディレクトリ配下のGitステータスを集計する
    ///
    /// ディレクトリ内のすべてのファイルのステータスをカウントして返す。
    pub(crate) fn get_directory_summary(&self, dir_path: &Path) -> GitStatusSummary {
        let mut summary = GitStatusSummary::default();

        for (path, status) in &self.statuses {
            // ディレクトリ配下のパスのみを対象とする
            if path.starts_with(dir_path) && path != dir_path {
                summary.add_status(*status);
            }
        }

        summary
    }
}
