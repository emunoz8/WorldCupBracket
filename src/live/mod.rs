//! Live-data domain types and the central `LiveState`.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::time::Instant;

mod rank;
mod scenarios;
mod sources;
mod sync;
mod ui;

pub(crate) use rank::*;
pub(crate) use scenarios::*;
pub(crate) use sources::*;
pub(crate) use ui::*;

/// All live-mode state, grouped out of `PredictorApp`. Populated by syncs; the
/// user's own bracket/standings live separately and are never touched by it.
pub(crate) struct LiveState {
    pub(crate) show_live: bool,
    pub(crate) api_key: String,
    pub(crate) live_rx: Option<Receiver<LiveData>>,
    pub(crate) live_status: Option<String>,
    pub(crate) live_standings: Vec<LiveStanding>,
    pub(crate) third_rank: Vec<ThirdPlaceRank>,
    pub(crate) live_mode: bool,
    pub(crate) last_poll: Option<Instant>,
    pub(crate) prev_group_points: HashMap<String, i64>,
    pub(crate) prev_third: HashMap<String, usize>,
    pub(crate) third_delta: HashMap<String, i8>,
    /// Team code → mathematically clinched group position (1..4), this sync.
    pub(crate) clinched: HashMap<String, u32>,
    /// Per 3rd-place team: clinch/elimination flags + simulated advance odds.
    pub(crate) third_outlook: HashMap<String, ThirdOutlook>,
    /// Winner-slot ("1A") → team code → P(that team is this slot's R32 third-place
    /// opponent), simulated through the Annex. Drives the bracket's 3rd-slot %.
    pub(crate) third_slot_pct: HashMap<String, HashMap<String, f32>>,
    /// Per group: where its third-place team lands in the R32, by scenario —
    /// "if results hold" plus the full destination distribution. Powers the
    /// expandable routing detail under each 3rd-place row.
    pub(crate) third_routing: HashMap<char, GroupRouting>,
    /// 3rd-place rows the user has expanded to see routing detail (by team code).
    pub(crate) expanded_thirds: std::collections::HashSet<String>,
    /// Qualification scenarios, computed lazily when a group is expanded and
    /// cached until the next sync — so a sync never brute-forces all 12 groups.
    pub(crate) scenario_cache: HashMap<char, GroupScenarios>,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) today_fixtures: Vec<LiveFixture>,
    /// Group-stage matches not yet finished, for the scenario engine.
    pub(crate) remaining: Vec<GroupFixture>,
    pub(crate) prev_scores: HashMap<String, (i64, i64)>,
    pub(crate) show_live_center: bool,
    pub(crate) api_log: Vec<String>,
}

impl Default for LiveState {
    fn default() -> Self {
        Self {
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
            clinched: HashMap::new(),
            third_outlook: HashMap::new(),
            third_slot_pct: HashMap::new(),
            third_routing: HashMap::new(),
            expanded_thirds: std::collections::HashSet::new(),
            scenario_cache: HashMap::new(),
            toasts: Vec::new(),
            today_fixtures: Vec::new(),
            remaining: Vec::new(),
            prev_scores: HashMap::new(),
            show_live_center: true,
            api_log: Vec::new(),
        }
    }
}

/// A transient notification shown bottom-right while in live mode.
pub(crate) enum AlertKind {
    Up,
    Down,
    Info,
}

pub(crate) struct Toast {
    pub(crate) text: String,
    pub(crate) kind: AlertKind,
    pub(crate) created: Instant,
}

impl Toast {
    pub(crate) fn new(text: String, kind: AlertKind) -> Self {
        Self {
            text,
            kind,
            created: Instant::now(),
        }
    }
}

/// One team's live league-table row.
#[derive(Clone)]
pub(crate) struct LiveTeam {
    pub(crate) name: String,
    pub(crate) code: String,
    pub(crate) position: u32,
    pub(crate) played: u32,
    pub(crate) points: i64,
    pub(crate) goal_diff: i64,
    pub(crate) goals_for: i64,
    pub(crate) goals_against: i64,
    /// Fair-play disciplinary points (lower is better). 0 until card data is sourced.
    pub(crate) disciplinary: i64,
}

/// One group's live standings, ordered by table position.
#[derive(Clone)]
pub(crate) struct LiveStanding {
    pub(crate) group: char,
    pub(crate) teams: Vec<LiveTeam>,
}

/// A 3rd-place team ranked across all groups; the top 8 advance to the R32.
#[derive(Clone)]
pub(crate) struct ThirdPlaceRank {
    pub(crate) group: char,
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) played: u32,
    pub(crate) points: i64,
    pub(crate) goal_diff: i64,
    pub(crate) goals_for: i64,
    pub(crate) disciplinary: i64,
    pub(crate) advances: bool,
}

/// A group-stage match not yet finished — used by the scenario engine to
/// enumerate remaining outcomes. Both codes are already canonical.
#[derive(Clone)]
pub(crate) struct GroupFixture {
    pub(crate) home: String,
    pub(crate) away: String,
}

/// A finished group-stage match with its final score. Both codes are canonical.
/// Sources contribute these; they're unioned (deduped by matchup) into standings.
#[derive(Clone)]
pub(crate) struct FinishedMatch {
    pub(crate) home: String,
    pub(crate) away: String,
    pub(crate) home_goals: i64,
    pub(crate) away_goals: i64,
}

/// A finished knockout match: the two teams and who won (by code).
#[derive(Clone)]
pub(crate) struct LiveResult {
    pub(crate) home: String,
    pub(crate) away: String,
    pub(crate) winner: Option<String>,
}

/// A match's state. `Scheduled` carries the local kickoff time (HH:MM).
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum MatchStatus {
    Scheduled(String),
    Live,
    Finished,
}

impl MatchStatus {
    pub(crate) fn is_live(&self) -> bool {
        matches!(self, MatchStatus::Live)
    }

    /// Short label for the status pill.
    pub(crate) fn label(&self) -> &str {
        match self {
            MatchStatus::Scheduled(time) => time,
            MatchStatus::Live => "LIVE",
            MatchStatus::Finished => "FT",
        }
    }
}

/// A fixture scheduled for today (kickoff time, teams, status, live/final score).
#[derive(Clone)]
pub(crate) struct LiveFixture {
    pub(crate) home: String,
    pub(crate) away: String,
    pub(crate) home_code: String,
    pub(crate) away_code: String,
    pub(crate) status: MatchStatus,
    pub(crate) score: Option<(i64, i64)>,
}

/// Everything one sync returns: standings, finished results, today's fixtures.
pub(crate) struct LiveData {
    pub(crate) standings: Vec<LiveStanding>,
    pub(crate) results: Vec<LiveResult>,
    pub(crate) today: Vec<LiveFixture>,
    pub(crate) remaining: Vec<GroupFixture>,
    /// Per-request log lines for this sync (endpoint, result).
    pub(crate) log: Vec<String>,
}

/// One source's contribution to a sync (any field may be empty).
#[derive(Default)]
pub(crate) struct SourceData {
    /// Finished group matches this source has seen (unioned across sources).
    pub(crate) finished: Vec<FinishedMatch>,
    /// Per-team disciplinary points (fair-play tiebreak); football-data only.
    pub(crate) discipline: HashMap<String, i64>,
    pub(crate) results: Vec<LiveResult>,
    pub(crate) today: Vec<LiveFixture>,
    pub(crate) remaining: Vec<GroupFixture>,
    pub(crate) log: Vec<String>,
}
