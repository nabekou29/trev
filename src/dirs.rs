//! Application directory resolution — XDG Base Directory compliant on all platforms.
//!
//! Uses the [`etcetera`] crate with XDG strategy so that config, state, cache,
//! and runtime directories follow community conventions for CLI/TUI tools
//! (matching helix, yazi, bat, starship).
//!
//! Environment variable overrides: `TREV_CONFIG_DIR`, `TREV_STATE_DIR`.

use std::path::{
    Path,
    PathBuf,
};

use anyhow::{
    Context as _,
    Result,
};

/// Application name used as the subdirectory under each XDG base path.
const APP_NAME: &str = "trev";

/// Resolved application directories.
///
/// All paths are absolute and include the `trev` subdirectory.
/// Created once at startup via [`AppDirs::new`].
#[derive(Debug, Clone)]
pub struct AppDirs {
    /// Configuration directory (`~/.config/trev/`).
    config: PathBuf,
    /// State directory (`~/.local/state/trev/`).
    state: PathBuf,
    /// Cache directory (`~/.cache/trev/`).
    cache: PathBuf,
    /// Runtime directory (`$XDG_RUNTIME_DIR/trev/` or `$TMPDIR/trev/`).
    runtime: PathBuf,
}

impl AppDirs {
    /// Resolve application directories from the current environment.
    ///
    /// Respects `TREV_CONFIG_DIR` and `TREV_STATE_DIR` overrides.
    /// Falls back to XDG base directories via [`etcetera`].
    pub fn new() -> Result<Self> {
        use etcetera::BaseStrategy as _;

        let strategy =
            etcetera::choose_base_strategy().context("could not determine home directory")?;

        Ok(Self::resolve(
            std::env::var("TREV_CONFIG_DIR").ok().as_deref(),
            std::env::var("TREV_STATE_DIR").ok().as_deref(),
            &strategy.config_dir(),
            strategy.state_dir().as_deref(),
            &strategy.cache_dir(),
            strategy.runtime_dir().as_deref(),
        ))
    }

    /// Pure path resolution logic, separated from environment for testability.
    fn resolve(
        env_config: Option<&str>,
        env_state: Option<&str>,
        xdg_config: &Path,
        xdg_state: Option<&Path>,
        xdg_cache: &Path,
        xdg_runtime: Option<&Path>,
    ) -> Self {
        let config = env_config.map_or_else(|| xdg_config.join(APP_NAME), PathBuf::from);

        let state = env_state.map_or_else(
            || xdg_state.map_or_else(|| xdg_cache.join(APP_NAME), |s| s.join(APP_NAME)),
            PathBuf::from,
        );

        let cache = xdg_cache.join(APP_NAME);

        let runtime =
            xdg_runtime.map_or_else(|| std::env::temp_dir().join(APP_NAME), |r| r.join(APP_NAME));

        Self { config, state, cache, runtime }
    }

    /// Configuration directory (e.g. `~/.config/trev/`).
    pub fn config_dir(&self) -> &Path {
        &self.config
    }

    /// State directory (e.g. `~/.local/state/trev/`).
    pub fn state_dir(&self) -> &Path {
        &self.state
    }

    /// Cache directory (e.g. `~/.cache/trev/`).
    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }

    /// Runtime directory for IPC sockets (e.g. `$XDG_RUNTIME_DIR/trev/`).
    pub fn runtime_dir(&self) -> &Path {
        &self.runtime
    }

    /// Session storage directory (`{state}/sessions/`).
    pub fn sessions_dir(&self) -> PathBuf {
        self.state.join("sessions")
    }

    /// Log file directory (`{state}/`).
    pub fn log_dir(&self) -> &Path {
        &self.state
    }

    /// Trash directory (`{state}/trash/`).
    pub fn trash_dir(&self) -> PathBuf {
        self.state.join("trash")
    }

    /// Configuration file path (`{config}/config.yml`).
    pub fn config_path(&self) -> PathBuf {
        self.config.join("config.yml")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use rstest::*;

    use super::*;

    // --- resolve defaults ---

    #[rstest]
    fn resolve_default_paths() {
        let dirs = AppDirs::resolve(
            None,
            None,
            Path::new("/home/user/.config"),
            Some(Path::new("/home/user/.local/state")),
            Path::new("/home/user/.cache"),
            Some(Path::new("/run/user/1000")),
        );

        assert_eq!(dirs.config_dir(), Path::new("/home/user/.config/trev"));
        assert_eq!(dirs.state_dir(), Path::new("/home/user/.local/state/trev"));
        assert_eq!(dirs.cache_dir(), Path::new("/home/user/.cache/trev"));
        assert_eq!(dirs.runtime_dir(), Path::new("/run/user/1000/trev"));
    }

    // --- env var overrides ---

    #[rstest]
    fn resolve_config_dir_override() {
        let dirs = AppDirs::resolve(
            Some("/custom/config"),
            None,
            Path::new("/home/user/.config"),
            Some(Path::new("/home/user/.local/state")),
            Path::new("/home/user/.cache"),
            None,
        );

        assert_eq!(dirs.config_dir(), Path::new("/custom/config"));
        assert_eq!(dirs.config_path(), PathBuf::from("/custom/config/config.yml"));
    }

    #[rstest]
    fn resolve_state_dir_override() {
        let dirs = AppDirs::resolve(
            None,
            Some("/custom/state"),
            Path::new("/home/user/.config"),
            Some(Path::new("/home/user/.local/state")),
            Path::new("/home/user/.cache"),
            None,
        );

        assert_eq!(dirs.state_dir(), Path::new("/custom/state"));
        assert_eq!(dirs.sessions_dir(), PathBuf::from("/custom/state/sessions"));
        assert_eq!(dirs.trash_dir(), PathBuf::from("/custom/state/trash"));
    }

    // --- fallbacks ---

    #[rstest]
    fn resolve_state_falls_back_to_cache_when_none() {
        let dirs = AppDirs::resolve(
            None,
            None,
            Path::new("/home/user/.config"),
            None, // state_dir is None (Windows)
            Path::new("/home/user/.cache"),
            None,
        );

        assert_eq!(dirs.state_dir(), Path::new("/home/user/.cache/trev"));
    }

    #[rstest]
    fn resolve_runtime_falls_back_to_temp_dir() {
        let dirs = AppDirs::resolve(
            None,
            None,
            Path::new("/home/user/.config"),
            Some(Path::new("/home/user/.local/state")),
            Path::new("/home/user/.cache"),
            None, // runtime_dir is None
        );

        let expected = std::env::temp_dir().join("trev");
        assert_eq!(dirs.runtime_dir(), expected.as_path());
    }

    // --- subpath accessors ---

    #[rstest]
    fn sessions_dir_is_under_state() {
        let dirs = AppDirs::resolve(
            None,
            None,
            Path::new("/cfg"),
            Some(Path::new("/state")),
            Path::new("/cache"),
            None,
        );

        assert_eq!(dirs.sessions_dir(), PathBuf::from("/state/trev/sessions"));
    }

    #[rstest]
    fn trash_dir_is_under_state() {
        let dirs = AppDirs::resolve(
            None,
            None,
            Path::new("/cfg"),
            Some(Path::new("/state")),
            Path::new("/cache"),
            None,
        );

        assert_eq!(dirs.trash_dir(), PathBuf::from("/state/trev/trash"));
    }

    #[rstest]
    fn log_dir_equals_state_dir() {
        let dirs = AppDirs::resolve(
            None,
            None,
            Path::new("/cfg"),
            Some(Path::new("/state")),
            Path::new("/cache"),
            None,
        );

        assert_eq!(dirs.log_dir(), dirs.state_dir());
    }
}
