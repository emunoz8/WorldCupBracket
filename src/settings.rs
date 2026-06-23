//! On-disk paths and the unified saved-state record (standings + UI + bracket).

use std::collections::HashMap;
use std::path::PathBuf;

use fifa_team3::{GroupState, Side, seed_group_states};

/// Where the live save lives: the per-user OS config directory, e.g.
/// `~/Library/Application Support/<APP>/save.json` (macOS),
/// `%APPDATA%\<APP>\save.json` (Windows), `~/.config/<APP>/save.json` (Linux).
/// Falls back to `data/save.json` (relative to the working dir) if no config dir.
pub(crate) fn save_path() -> PathBuf {
    match dirs::config_dir() {
        Some(dir) => dir.join(crate::APP_NAME).join("save.json"),
        None => PathBuf::from("data/save.json"),
    }
}

/// Where the live API debug log is mirrored to disk, so it can be read/copied
/// outside the app. Same dir as the save; falls back to `data/`. Retained for the
/// troubleshooting hook in `sync::log_line` (disabled by default).
#[allow(dead_code)]
pub(crate) fn log_path() -> PathBuf {
    match dirs::config_dir() {
        Some(dir) => dir.join(crate::APP_NAME).join("api_log.txt"),
        None => PathBuf::from("data/api_log.txt"),
    }
}

/// Directory holding the user's named save files (one `<name>.json` each).
pub(crate) fn saves_dir() -> PathBuf {
    match dirs::config_dir() {
        Some(dir) => dir.join(crate::APP_NAME).join("saves"),
        None => PathBuf::from("data/saves"),
    }
}

/// Path of a named save, sanitizing the name to a safe file stem.
pub(crate) fn named_save_path(name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    saves_dir().join(format!("{}.json", safe.trim()))
}

/// All saved names (file stems) in the saves directory, sorted.
pub(crate) fn list_saves() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(saves_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            (path.extension()?.to_str()? == "json")
                .then(|| path.file_stem()?.to_str().map(str::to_string))?
        })
        .collect();
    names.sort();
    names
}

/// One snapshot of everything the user can change: standings order/third-place,
/// theme, panel visibility, and bracket picks. Written by Save, read by Reload.
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct AppState {
    pub(crate) dark: bool,
    pub(crate) show_standings: bool,
    #[serde(default)]
    pub(crate) picks: HashMap<usize, Side>,
    #[serde(default)]
    pub(crate) groups: Vec<GroupState>,
    /// Whether the onboarding tutorial has been completed/skipped.
    #[serde(default)]
    pub(crate) tutorial_seen: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            dark: true,
            show_standings: true,
            picks: HashMap::new(),
            groups: seed_group_states(),
            tutorial_seen: false,
        }
    }
}
