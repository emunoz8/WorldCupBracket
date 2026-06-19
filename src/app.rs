//! Application state and the top-level egui frame.

use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

use eframe::egui::{self, Rect, RichText, Stroke, Vec2};
use fifa_team3::{
    Annex, GroupState, KoMatch, PredictionReport, Side, Slot, WinnerPrediction,
    annex_filters_from_groups, prediction_report, seed_group_states,
};

use crate::APP_NAME;
use crate::bracket::bracket_view;
use crate::print::{build_print_html, open_path};
use crate::settings::{AppState, save_path};
use crate::standings::{group_table, info_chip, third_place_chip};
use crate::theme::{COLOR_ACCENT, COLOR_GREEN, COLOR_RED, Palette};

pub(crate) struct PredictorApp {
    annex: Annex,
    pub(crate) groups: Vec<GroupState>,
    pub(crate) dragged: Option<(usize, usize)>,
    /// Row a drag is currently hovering over (group, position) — where a gap opens.
    pub(crate) drop_target: Option<(usize, usize)>,
    /// User-selected winner for each knockout match number.
    pub(crate) picks: HashMap<usize, Side>,
    /// Whether the left standings panel is visible.
    show_standings: bool,
    /// Dark theme when true, light theme when false.
    dark: bool,
    /// Transient status message for save/reload feedback.
    status: Option<String>,
    load_error: Option<String>,
    /// Active onboarding step, or None when the tutorial is not running.
    pub(crate) tutorial: Option<crate::tutorial::TutorialStep>,
    /// Group letters selected as the 4 worst 3rd-place teams during the tutorial.
    pub(crate) worst: std::collections::HashSet<char>,
    /// Whether the tutorial has been completed/skipped (persisted).
    pub(crate) tutorial_seen: bool,
    /// Whether the named-saves manager window is open.
    pub(crate) show_saves: bool,
    /// Text-field buffer for naming a new save.
    pub(crate) save_name: String,
    /// Whether the live-data window is open.
    pub(crate) show_live: bool,
    /// football-data.org API token.
    pub(crate) api_key: String,
    /// Receiver for an in-flight background fetch, if any.
    pub(crate) live_rx: Option<std::sync::mpsc::Receiver<crate::live::LiveMsg>>,
    /// Status line for the live-data window.
    pub(crate) live_status: Option<String>,
    /// Standings from FINISHED matches only (stable mid-match).
    pub(crate) live_standings: Vec<crate::live::LiveStanding>,
    /// Cross-group 3rd-place ranking from the last live sync (top 8 advance).
    pub(crate) third_rank: Vec<crate::live::ThirdPlaceRank>,
    /// Live mode: poll scores every 20s and raise alerts.
    pub(crate) live_mode: bool,
    /// When the last automatic poll fired.
    pub(crate) last_poll: Option<Instant>,
    /// Previous poll's group points (code → points) for result-based alerts.
    prev_group_points: HashMap<String, i64>,
    /// Previous poll's 3rd-place ranks (code → index) for diffing.
    prev_third: HashMap<String, usize>,
    /// Movement since last poll for the 3rd-place table arrows (-1 up, +1 down).
    pub(crate) third_delta: HashMap<String, i8>,
    /// Bottom-right live notifications.
    pub(crate) toasts: Vec<crate::live::Toast>,
    /// Today's fixtures from the last sync.
    pub(crate) today_fixtures: Vec<crate::live::LiveFixture>,
    /// Previous poll's live scores per fixture, for goal detection.
    prev_scores: HashMap<String, (i64, i64)>,
    /// Whether the combined Live Center window is shown.
    pub(crate) show_live_center: bool,
    /// Rolling log of API calls (most recent last).
    pub(crate) api_log: Vec<String>,
}

impl PredictorApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Enable image (incl. SVG) loaders so flag textures can be drawn.
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Annex C is embedded at compile time so the binary is self-contained.
        const ANNEX_JSON: &str = include_str!("../data/annex_c.json");
        let annex_result = serde_json::from_str(ANNEX_JSON).map_err(|e| e.to_string());

        // Start from a clean slate every launch (seed teams, no picks, no results).
        // Only UI preferences (theme, panel, tutorial-seen) are remembered; the
        // bracket/standings are loaded only when the user opens a saved profile.
        let saved: Option<AppState> = fs::read_to_string(save_path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok());

        let (annex, load_error) = match annex_result {
            Ok(annex) => (annex, None),
            Err(e) => (Annex::new(), Some(e)),
        };

        let dark = saved.as_ref().is_none_or(|s| s.dark);
        let show_standings = saved.as_ref().is_none_or(|s| s.show_standings);
        let tutorial_seen = saved.as_ref().is_some_and(|s| s.tutorial_seen);

        // Auto-run the tutorial until it has been completed/skipped once.
        let tutorial = (!tutorial_seen).then_some(crate::tutorial::TutorialStep::Groups);

        Self {
            annex,
            groups: seed_group_states(),
            dragged: None,
            drop_target: None,
            picks: HashMap::new(),
            show_standings,
            dark,
            status: None,
            load_error,
            tutorial,
            worst: std::collections::HashSet::new(),
            tutorial_seen,
            show_saves: false,
            save_name: String::new(),
            show_live: false,
            api_key: std::env::var("FOOTBALL_DATA_TOKEN").unwrap_or_default(),
            live_rx: None,
            live_status: None,
            live_standings: Vec::new(),
            third_rank: Vec::new(),
            live_mode: false,
            last_poll: None,
            prev_group_points: HashMap::new(),
            prev_third: HashMap::new(),
            third_delta: HashMap::new(),
            toasts: Vec::new(),
            today_fixtures: Vec::new(),
            prev_scores: HashMap::new(),
            show_live_center: true,
            api_log: Vec::new(),
        }
    }

    /// Append a timestamped line to the API log (keeping the last ~120).
    fn log_line(&mut self, line: String) {
        self.api_log.push(line);
        let len = self.api_log.len();
        if len > 120 {
            self.api_log.drain(0..len - 120);
        }
    }

    /// Fire goal alerts (toast + sound) when a live fixture's score increased.
    fn detect_goals(&mut self) {
        let baseline = self.prev_scores.is_empty();
        let mut new_scores: HashMap<String, (i64, i64)> = HashMap::new();
        let mut goal = false;
        for f in &self.today_fixtures {
            let Some((h, a)) = f.score else { continue };
            let key = format!("{}-{}", f.home_code, f.away_code);
            if !baseline && let Some(&(oh, oa)) = self.prev_scores.get(&key) {
                if h > oh {
                    self.toasts.push(crate::live::Toast::new(
                        format!("GOAL · {} {h}-{a} {}", f.home, f.away),
                        crate::live::AlertKind::Up,
                    ));
                    goal = true;
                }
                if a > oa {
                    self.toasts.push(crate::live::Toast::new(
                        format!("GOAL · {} {h}-{a} {}", f.away, f.home),
                        crate::live::AlertKind::Up,
                    ));
                    goal = true;
                }
            }
            new_scores.insert(key, (h, a));
        }
        if goal {
            crate::live::play_alert(true);
        }
        self.prev_scores = new_scores;
    }

    /// Kick off a background fetch of live standings from football-data.org.
    pub(crate) fn start_live_sync(&mut self, ctx: egui::Context, trigger: &str) {
        // No token is fine: football-data is skipped, but ESPN still gives live
        // scores/fixtures. A token additionally enables standings/results.
        let t = chrono::Local::now().format("%H:%M:%S");
        self.log_line(format!("{t}  sync started ({trigger})"));
        let provider = Box::new(crate::live::FootballData {
            token: self.api_key.trim().to_string(),
        });
        self.live_rx = Some(crate::live::spawn_fetch(provider, ctx));
        self.live_status = Some("Syncing…".to_string());
    }

    /// Drain a finished background fetch and apply it.
    fn poll_live(&mut self) {
        use std::sync::mpsc::TryRecvError;
        let result = self.live_rx.as_ref().map(|rx| rx.try_recv());
        match result {
            Some(Ok(crate::live::LiveMsg::Data(data))) => {
                self.apply_live(data);
                self.live_rx = None;
            }
            Some(Ok(crate::live::LiveMsg::Error(e))) => {
                self.live_status = Some(format!("Sync failed: {e}"));
                self.live_rx = None;
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.live_status = Some("Sync failed: worker stopped".to_string());
                self.live_rx = None;
            }
            _ => {}
        }
    }

    /// Reorder groups, rank 3rd-place, and apply finished knockout results.
    fn apply_live(&mut self, data: crate::live::LiveData) {
        let crate::live::LiveData {
            standings,
            results,
            today,
            log,
        } = data;
        for line in log {
            self.log_line(line);
        }
        self.today_fixtures = today;

        // Goal alerts: compare each live fixture's score to the previous poll.
        self.detect_goals();

        // Stickiness: football-data can briefly un-report a just-finished match,
        // which would drop a team's points and oscillate the order. Never let a
        // team's played-count go down vs the previous poll — keep the fuller stats.
        let mut standings = standings;
        let prev: HashMap<&str, &crate::live::LiveTeam> = self
            .live_standings
            .iter()
            .flat_map(|s| s.teams.iter().map(|t| (t.code.as_str(), t)))
            .collect();
        for s in &mut standings {
            for t in &mut s.teams {
                if let Some(old) = prev.get(t.code.as_str())
                    && old.played > t.played
                {
                    *t = (*old).clone();
                }
            }
        }
        crate::live::sort_standings(&mut standings);

        // Break remaining ties using the user's own group order (the left panel),
        // so the live standings / projection match what's shown on the left.
        for s in &mut standings {
            if let Some(group) = self.groups.iter().find(|g| g.group == s.group) {
                let order: Vec<&str> = group.teams.iter().map(|t| t.code.as_str()).collect();
                let rank = |code: &str| order.iter().position(|c| *c == code).unwrap_or(usize::MAX);
                s.teams.sort_by(|x, y| {
                    y.points
                        .cmp(&x.points)
                        .then(y.goal_diff.cmp(&x.goal_diff))
                        .then(y.goals_for.cmp(&x.goals_for))
                        .then_with(|| rank(&x.code).cmp(&rank(&y.code)))
                });
                for (i, t) in s.teams.iter_mut().enumerate() {
                    t.position = (i + 1) as u32;
                }
            }
        }

        // Live data is view-only: it populates the Live Center (standings, 3rd-place
        // table, stat badges) but never mutates the user's own groups — so saved
        // brackets/standings stay exactly as saved.
        let applied = standings.len();
        self.third_rank = crate::live::third_place_ranking(&standings);
        self.live_standings = standings;

        // Raise alerts for any movement vs the previous poll.
        self.diff_and_alert();

        // Apply finished knockout results to bracket picks.
        let wins = self.apply_live_results(&results);
        self.live_status = Some(format!(
            "Synced {applied} groups · {wins} knockout results from live data"
        ));
    }

    /// Alert only on real events: a team gaining points (a result), or a team
    /// crossing the top-8 cutoff in the 3rd-place race. With sticky stats these
    /// only ever move one way, so each alert fires once — no oscillation.
    fn diff_and_alert(&mut self) {
        let baseline = self.prev_group_points.is_empty();
        let mut events: Vec<(String, crate::live::AlertKind)> = Vec::new();
        let mut any_up = false;

        if !baseline {
            // A team's points went up → it just won or drew a match.
            for s in &self.live_standings {
                for t in &s.teams {
                    if let Some(&old) = self.prev_group_points.get(&t.code)
                        && t.points > old
                    {
                        any_up = true;
                        events.push((
                            format!(
                                "+ {} · {} in Group {}",
                                t.name,
                                ordinal(t.position),
                                s.group
                            ),
                            crate::live::AlertKind::Up,
                        ));
                    }
                }
            }
            // A 3rd-place team crossed the top-8 line (advancing ↔ eliminated).
            for (i, r) in self.third_rank.iter().enumerate() {
                if let Some(&old) = self.prev_third.get(&r.code) {
                    let was_in = old < 8;
                    let now_in = i < 8;
                    if was_in != now_in {
                        let up = now_in;
                        any_up |= up;
                        let (mark, status) = if now_in {
                            ("+", "now advancing")
                        } else {
                            ("-", "now ELIMINATED")
                        };
                        events.push((
                            format!("{mark} {} · 3rd-place race ({status})", r.name),
                            if up {
                                crate::live::AlertKind::Up
                            } else {
                                crate::live::AlertKind::Down
                            },
                        ));
                    }
                }
            }
        }

        let any_down = events
            .iter()
            .any(|(_, k)| matches!(k, crate::live::AlertKind::Down));
        for (text, kind) in events {
            self.toasts.push(crate::live::Toast::new(text, kind));
        }
        if any_up {
            crate::live::play_alert(true);
        }
        if any_down {
            crate::live::play_alert(false);
        }

        // Movement snapshot for the 3rd-place table arrows (vs the previous poll).
        self.third_delta = self
            .third_rank
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                self.prev_third
                    .get(&r.code)
                    .map(|&old| (r.code.clone(), (i as i64 - old as i64).signum() as i8))
            })
            .collect();

        // Record this poll as the new baseline.
        self.prev_group_points = self
            .live_standings
            .iter()
            .flat_map(|s| s.teams.iter().map(|t| (t.code.clone(), t.points)))
            .collect();
        self.prev_third = self
            .third_rank
            .iter()
            .enumerate()
            .map(|(i, r)| (r.code.clone(), i))
            .collect();
    }

    /// Set bracket picks from finished live knockout matches. Matches are paired
    /// by the two teams (no FIFA match-number dependency); rounds are processed in
    /// order so winners propagate forward. Returns how many picks were set.
    fn apply_live_results(&mut self, results: &[crate::live::LiveResult]) -> usize {
        use std::collections::HashMap;
        let report = self.report();
        let predictions: HashMap<&str, &WinnerPrediction> = report
            .predictions
            .iter()
            .map(|p| (p.winner_slot.as_str(), p))
            .collect();
        let mut matches = fifa_team3::knockout_matches();
        matches.sort_by_key(|m| m.round);
        let index: HashMap<usize, KoMatch> = matches.iter().map(|m| (m.match_number, *m)).collect();

        let mut count = 0;
        for km in &matches {
            let left = self.resolve_side(km, Side::Left, &index, &predictions);
            let right = self.resolve_side(km, Side::Right, &index, &predictions);
            let (Some(lc), Some(rc)) = (
                crate::bracket::competitor_code(self, &left),
                crate::bracket::competitor_code(self, &right),
            ) else {
                continue;
            };
            // Find a finished result whose two teams match this pairing.
            if let Some(result) = results.iter().find(|r| {
                r.winner.is_some()
                    && ((r.home == lc && r.away == rc) || (r.home == rc && r.away == lc))
            }) {
                let winner = result.winner.as_deref().unwrap_or("");
                let side = if winner == lc {
                    Some(Side::Left)
                } else if winner == rc {
                    Some(Side::Right)
                } else {
                    None
                };
                if let Some(side) = side {
                    self.picks.insert(km.match_number, side);
                    count += 1;
                }
            }
        }
        count
    }

    /// Reset everything to zero: seed teams, no standings results, no bracket picks.
    pub(crate) fn reset_all(&mut self) {
        self.groups = seed_group_states();
        self.picks.clear();
        self.status = Some("Reset to a clean slate".to_string());
    }

    /// Build a printable HTML report and open it in the default browser.
    fn print_bracket(&mut self, report: &PredictionReport) {
        let html = build_print_html(self, report);
        let mut path = std::env::temp_dir();
        path.push("fifa_bracket_print.html");
        self.status = Some(match fs::write(&path, html) {
            Ok(()) => match open_path(&path) {
                Ok(()) => "Opened print view in your browser (use Cmd/Ctrl+P)".to_string(),
                Err(e) => format!("Saved {} but could not open it: {e}", path.display()),
            },
            Err(e) => format!("Print failed: {e}"),
        });
    }

    fn report(&self) -> PredictionReport {
        let (passing, eliminated) = annex_filters_from_groups(&self.groups);
        prediction_report(&self.annex, &passing, &eliminated)
    }

    /// Live stats for a team (by group + code), if a sync has been done.
    pub(crate) fn live_stats(&self, group: char, code: &str) -> Option<&crate::live::LiveTeam> {
        let cc = crate::live::canonical_code(code);
        self.live_standings
            .iter()
            .find(|s| s.group == group)?
            .teams
            .iter()
            .find(|t| t.code == cc)
    }

    pub(crate) fn pal(&self) -> Palette {
        if self.dark {
            Palette::DARK
        } else {
            Palette::LIGHT
        }
    }

    /// Capture the current UI/standings/bracket into a serializable snapshot.
    fn snapshot(&self) -> AppState {
        AppState {
            dark: self.dark,
            show_standings: self.show_standings,
            picks: self.picks.clone(),
            groups: self.groups.clone(),
            tutorial_seen: self.tutorial_seen,
        }
    }

    /// Apply a loaded snapshot onto the live app state. Loading turns off the live
    /// feed so a freshly loaded bracket/standings can't be overwritten by a sync.
    fn apply_state(&mut self, state: AppState) {
        self.groups = state.groups;
        self.picks = state.picks;
        self.show_standings = state.show_standings;
        self.dark = state.dark;
        self.tutorial_seen = state.tutorial_seen;
        self.live_mode = false;
        self.live_rx = None; // drop any in-flight sync
    }

    fn write_snapshot(&self, path: &std::path::Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.snapshot()).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, json).map_err(|e| e.to_string())
    }

    /// Snapshot everything to the current-session file (auto-restored on launch).
    pub(crate) fn save_state(&mut self) {
        self.status = Some(match self.write_snapshot(&save_path()) {
            Ok(()) => "Saved standings, theme, and bracket".to_string(),
            Err(e) => format!("Save failed: {e}"),
        });
    }

    /// Restore the current-session file, discarding unsaved changes.
    fn reload_state(&mut self) {
        self.status = Some(
            match fs::read_to_string(save_path())
                .map_err(|e| e.to_string())
                .and_then(|json| serde_json::from_str::<AppState>(&json).map_err(|e| e.to_string()))
            {
                Ok(state) => {
                    self.apply_state(state);
                    "Reloaded current session".to_string()
                }
                Err(e) => format!("Reload failed: {e}"),
            },
        );
    }

    /// Save the current state under a chosen name (a separate, shareable file).
    pub(crate) fn save_as(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.status = Some("Enter a name to save".to_string());
            return;
        }
        let path = crate::settings::named_save_path(name);
        self.status = Some(match self.write_snapshot(&path) {
            Ok(()) => {
                let _ = self.write_snapshot(&save_path()); // keep current session in sync
                format!("Saved as \"{name}\"")
            }
            Err(e) => format!("Save failed: {e}"),
        });
    }

    /// Load a named save (e.g. someone else's bracket) into the live state.
    pub(crate) fn load_named(&mut self, name: &str) {
        self.status = Some(
            match fs::read_to_string(crate::settings::named_save_path(name))
                .map_err(|e| e.to_string())
                .and_then(|json| serde_json::from_str::<AppState>(&json).map_err(|e| e.to_string()))
            {
                Ok(state) => {
                    self.apply_state(state);
                    let _ = self.write_snapshot(&save_path());
                    format!("Loaded \"{name}\"")
                }
                Err(e) => format!("Load failed: {e}"),
            },
        );
    }

    /// Export the current bracket to any location via a native save dialog.
    pub(crate) fn export_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Bracket", &["json"])
            .set_file_name("bracket.json")
            .save_file()
        {
            self.status = Some(match self.write_snapshot(&path) {
                Ok(()) => format!("Exported to {}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            });
        }
    }

    /// Import a bracket from any location via a native open dialog.
    pub(crate) fn import_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Bracket", &["json"])
            .pick_file()
        {
            self.status = Some(
                match fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|json| {
                        serde_json::from_str::<AppState>(&json).map_err(|e| e.to_string())
                    }) {
                    Ok(state) => {
                        self.apply_state(state);
                        let _ = self.write_snapshot(&save_path());
                        self.show_saves = false;
                        format!("Imported {}", path.display())
                    }
                    Err(e) => format!("Import failed: {e}"),
                },
            );
        }
    }

    /// Delete a named save file.
    pub(crate) fn delete_named(&mut self, name: &str) {
        self.status = Some(
            match fs::remove_file(crate::settings::named_save_path(name)) {
                Ok(()) => format!("Deleted \"{name}\""),
                Err(e) => format!("Delete failed: {e}"),
            },
        );
    }

    fn team_for_slot(&self, slot: &str) -> String {
        let mut chars = slot.chars();
        let position = chars.next().and_then(|v| v.to_digit(10)).unwrap_or(0) as usize;
        let group = chars.next().unwrap_or(' ');
        self.groups
            .iter()
            .find(|s| s.group == group)
            .and_then(|s| s.teams.get(position.saturating_sub(1)))
            .map(|t| t.name.clone())
            .unwrap_or_else(|| slot.to_string())
    }

    pub(crate) fn third_place_team(&self, opponent: &str) -> String {
        opponent
            .chars()
            .nth(1)
            .and_then(|group| {
                self.groups
                    .iter()
                    .find(|s| s.group == group)
                    .and_then(|s| s.teams.get(2))
                    .map(|t| t.name.clone())
            })
            .unwrap_or_else(|| opponent.to_string())
    }

    /// Display name for one competitor of a match, following picks into earlier rounds.
    pub(crate) fn resolve_side(
        &self,
        km: &KoMatch,
        side: Side,
        index: &HashMap<usize, KoMatch>,
        predictions: &HashMap<&str, &WinnerPrediction>,
    ) -> String {
        let slot = match side {
            Side::Left => km.left,
            Side::Right => km.right,
        };
        match slot {
            Slot::Group(s) => self.team_for_slot(s),
            // A third-place opponent: name it only once a single opponent remains.
            Slot::ThirdPlace => self.certain_third_place(km, predictions),
            Slot::Winner(m) => match (self.picks.get(&m), index.get(&m)) {
                (Some(picked), Some(child)) => {
                    self.resolve_side(child, *picked, index, predictions)
                }
                _ => format!("Winner M{m}"),
            },
            Slot::Loser(m) => match (self.picks.get(&m), index.get(&m)) {
                (Some(Side::Left), Some(child)) => {
                    self.resolve_side(child, Side::Right, index, predictions)
                }
                (Some(Side::Right), Some(child)) => {
                    self.resolve_side(child, Side::Left, index, predictions)
                }
                _ => format!("Loser M{m}"),
            },
        }
    }

    /// The third-place opponent's real team name if exactly one remains possible, else "3rd place".
    fn certain_third_place(
        &self,
        km: &KoMatch,
        predictions: &HashMap<&str, &WinnerPrediction>,
    ) -> String {
        if let Slot::Group(winner_slot) = km.left
            && let Some(prediction) = predictions.get(winner_slot)
            && prediction.opponents.len() == 1
        {
            return self.third_place_team(&prediction.opponents[0].opponent);
        }
        "3rd place".to_string()
    }
}

impl eframe::App for PredictorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_live();

        // Live mode: poll scores about once a minute.
        if self.live_mode {
            ctx.request_repaint_after(Duration::from_secs(1));
            let due = self.last_poll.is_none_or(|t| t.elapsed().as_secs() >= 20);
            if due && self.live_rx.is_none() {
                self.last_poll = Some(Instant::now());
                self.start_live_sync(ctx.clone(), "auto");
            }
        }

        let report = self.report();
        let pal = self.pal();

        ctx.set_visuals(if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        let mut style = (*ctx.style()).clone();
        style.visuals.panel_fill = pal.panel;
        style.visuals.window_fill = pal.card;
        ctx.set_style(style);

        egui::TopBottomPanel::top("top_bar")
            .exact_height(52.0)
            .frame(
                egui::Frame::new()
                    .fill(pal.top_bar)
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let toggle_label = if self.show_standings {
                        "◀ Hide standings"
                    } else {
                        "Show standings ▶"
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(toggle_label).size(12.0).color(pal.text),
                            )
                            .fill(pal.card)
                            .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.show_standings = !self.show_standings;
                    }
                    let theme_label = if self.dark { "☀ Light" } else { "🌙 Dark" };
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(theme_label).size(12.0).color(pal.text),
                            )
                            .fill(pal.card)
                            .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.dark = !self.dark;
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Save").size(12.0).color(COLOR_GREEN))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.save_state();
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Reload").size(12.0).color(pal.dim))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.reload_state();
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("📁 Saves").size(12.0).color(pal.text))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .on_hover_text("Save under a name / load others' brackets")
                        .clicked()
                    {
                        self.show_saves = !self.show_saves;
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("🛰 Live").size(12.0).color(pal.text))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .on_hover_text("Pull real standings from football-data.org")
                        .clicked()
                    {
                        self.show_live = !self.show_live;
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("🖨 Print").size(12.0).color(pal.text))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.print_bracket(&report);
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("? Tutorial").size(12.0).color(pal.text),
                            )
                            .fill(pal.card)
                            .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .on_hover_text("Replay the guided setup")
                        .clicked()
                    {
                        self.worst.clear();
                        self.tutorial = Some(crate::tutorial::TutorialStep::Groups);
                    }
                    ui.add_space(12.0);
                    ui.label(RichText::new(APP_NAME).size(20.0).strong().color(pal.text));
                    ui.add_space(16.0);
                    if let Some(status) = &self.status {
                        ui.label(RichText::new(status).size(12.0).color(pal.dim));
                    }
                    info_chip(
                        ui,
                        &format!("{} scenarios", report.possible_scenarios),
                        COLOR_ACCENT,
                        pal,
                    );
                    if !report.known_passing.is_empty() {
                        info_chip(
                            ui,
                            &format!("Advanced: {}", report.known_passing),
                            COLOR_GREEN,
                            pal,
                        );
                    }
                    if !report.known_eliminated.is_empty() {
                        info_chip(
                            ui,
                            &format!("Out: {}", report.known_eliminated),
                            COLOR_RED,
                            pal,
                        );
                    }
                    if let Some(err) = &self.load_error {
                        ui.label(RichText::new(err).color(COLOR_RED).size(12.0));
                    }
                });
            });

        if self.show_standings {
            egui::SidePanel::left("groups_panel")
                .resizable(true)
                .default_width(340.0)
                .width_range(280.0..=480.0)
                .frame(
                    egui::Frame::new()
                        .fill(pal.panel)
                        .inner_margin(egui::Margin::same(0)),
                )
                .show(ctx, |ui| {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Standings")
                                .size(18.0)
                                .strong()
                                .color(pal.text),
                        );
                        ui.label(RichText::new("drag to reorder").size(11.0).color(pal.dim));
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let all_done = self.groups.iter().all(|g| g.completed);
                        let (label, color) = if all_done {
                            ("✓ All groups complete — mark pending", COLOR_GREEN)
                        } else {
                            ("Complete all groups", pal.text)
                        };
                        if ui
                            .add(
                                egui::Button::new(RichText::new(label).size(12.0).color(color))
                                    .fill(pal.card)
                                    .stroke(Stroke::new(1.0, pal.border)),
                            )
                            .on_hover_text("Lock every group's standings as final")
                            .clicked()
                        {
                            let value = !all_done;
                            for group in &mut self.groups {
                                group.completed = value;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(2.0);
                            for group_index in 0..self.groups.len() {
                                group_table(ui, self, group_index);
                                ui.add_space(10.0);
                            }
                            // Bottom padding so the last group (L) can scroll fully
                            // clear of the panel edge while reordering.
                            ui.add_space(160.0);
                        });
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(pal.panel)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                if !report.errors.is_empty() {
                    for error in &report.errors {
                        ui.label(RichText::new(error).color(COLOR_RED));
                    }
                }

                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("3rd place:").size(12.0).color(pal.dim));
                    for group in &mut self.groups {
                        third_place_chip(ui, group, pal);
                    }
                });
                ui.add_space(8.0);

                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        bracket_view(ui, self, &report);
                    });
            });

        // Floating ghost card following cursor while dragging
        if let Some((gi, pi)) = self.dragged {
            ctx.request_repaint();
            if let Some(cursor) = ctx.input(|i| i.pointer.interact_pos()) {
                let layer = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("drag_ghost"),
                ));
                let ghost_rect =
                    Rect::from_min_size(cursor + Vec2::new(14.0, -15.0), Vec2::new(180.0, 30.0));
                layer.rect(
                    ghost_rect,
                    4.0,
                    pal.card,
                    Stroke::new(1.5, COLOR_ACCENT),
                    egui::StrokeKind::Middle,
                );
                if let Some(team) = self.groups[gi].teams.get(pi) {
                    layer.text(
                        ghost_rect.min + Vec2::new(10.0, 8.0),
                        egui::Align2::LEFT_TOP,
                        &team.name,
                        egui::FontId::proportional(12.0),
                        pal.text,
                    );
                }
            }
        }

        // On release, drop the dragged team into the hovered slot (insertion move).
        if ctx.input(|i| i.pointer.any_released()) {
            if let (Some((from_group, from_pos)), Some((to_group, to_pos))) =
                (self.dragged, self.drop_target)
                && from_group == to_group
                // to_pos is an insertion index (0..=len); dropping at from_pos or
                // just after it is a no-op.
                && to_pos != from_pos
                && to_pos != from_pos + 1
            {
                let team = self.groups[from_group].teams.remove(from_pos);
                // Removing an earlier element shifts later insertion points down by one.
                let dest = if from_pos < to_pos {
                    to_pos - 1
                } else {
                    to_pos
                };
                self.groups[from_group].teams.insert(dest, team);
            }
            self.dragged = None;
            self.drop_target = None;
        }

        // Named-saves manager + onboarding overlay, drawn on top of everything.
        crate::saves::saves_window(self, ctx);
        crate::live::live_window(self, ctx);
        crate::live::live_center_window(self, ctx);
        crate::live::toasts_overlay(self, ctx);
        crate::tutorial::run(self, ctx);
    }
}

/// Format a 1-based position as an ordinal (1st, 2nd, 3rd, 4th…).
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}
