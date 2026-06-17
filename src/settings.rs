//! On-disk paths and the unified saved-state record (standings + UI + bracket).

use std::collections::HashMap;
use std::path::PathBuf;

use fifa_team3::{GroupState, Side, default_group_states};

/// Committed seed of groups + teams; used until the first Save creates the save file.
pub(crate) const TEAMS_PATH: &str = "data/teams.json";

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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            dark: true,
            show_standings: true,
            picks: HashMap::new(),
            groups: default_group_states(),
        }
    }
}
