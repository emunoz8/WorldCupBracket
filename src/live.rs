//! Live-stats plumbing: a background fetch that pulls real group standings from
//! a provider (football-data.org) and hands them back to the UI over a channel.
//!
//! egui is single-threaded and synchronous, so network I/O runs on a worker
//! thread; results arrive as `LiveMsg` values the UI drains each frame.

use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText, Sense, Stroke};

use crate::app::PredictorApp;
use crate::standings::flag_image;
use crate::theme::{COLOR_ACCENT, COLOR_GREEN, COLOR_RED, Palette};

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

/// Play a short beep on a worker thread: higher pitch for up, lower for down.
pub(crate) fn play_alert(up: bool) {
    thread::spawn(move || {
        use rodio::Source;
        let Ok(handle) = rodio::DeviceSinkBuilder::open_default_sink() else {
            return;
        };
        let freq = if up { 880.0 } else { 392.0 };
        let beep = rodio::source::SineWave::new(freq)
            .take_duration(Duration::from_millis(180))
            .amplify(0.15);
        handle.mixer().add(beep);
        // Keep the device handle alive until the beep finishes.
        thread::sleep(Duration::from_millis(230));
    });
}

/// Render the stack of live-mode notifications in the bottom-right corner.
pub(crate) fn toasts_overlay(app: &mut PredictorApp, ctx: &egui::Context) {
    app.toasts
        .retain(|t| t.created.elapsed().as_secs_f32() < 8.0);
    if app.toasts.is_empty() {
        return;
    }
    ctx.request_repaint_after(Duration::from_millis(500));
    let pal = app.pal();

    egui::Area::new(egui::Id::new("live_toasts"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
        .show(ctx, |ui| {
            ui.set_max_width(300.0);
            for toast in app.toasts.iter().rev().take(6) {
                let accent = match toast.kind {
                    AlertKind::Up => COLOR_GREEN,
                    AlertKind::Down => COLOR_RED,
                    AlertKind::Info => COLOR_ACCENT,
                };
                egui::Frame::new()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.5, accent))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.set_min_width(220.0);
                        ui.label(RichText::new(&toast.text).size(12.0).color(pal.text));
                    });
                ui.add_space(6.0);
            }
        });
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

/// FIFA-ranking order of the 48 finalists (best first), used as the final
/// tiebreaker so ties resolve deterministically. Top finalists follow the
/// official FIFA Men's World Ranking (11 June 2026); the lower tail is
/// approximate — refine nearer the tournament.
#[rustfmt::skip]
const FIFA_ORDER: [&str; 48] = [
    // Exact order from FIFA's top 20 (finalists only).
    "ARG", "ESP", "FRA", "ENG", "POR", "BRA", "MAR", "NED", "BEL", "GER",
    "CRO", "COL", "MEX", "SEN", "URU", "USA", "JPN", "SUI", "IRN",
    // Approximate beyond the published top 20.
    "TUR", "KOR", "ECU", "AUT", "AUS", "CAN", "NOR", "SWE", "EGY", "RSA",
    "ALG", "PAR", "CIV", "TUN", "QAT", "BIH", "SCO", "CZE", "UZB", "COD",
    "JOR", "NZL", "KSA", "IRQ", "GHA", "PAN", "CPV", "HAI", "CUW",
];

/// A team's FIFA-ranking slot (lower is better); unknown codes sort last.
fn fifa_rank(code: &str) -> usize {
    FIFA_ORDER
        .iter()
        .position(|c| *c == code)
        .unwrap_or(usize::MAX)
}

/// Sort every group by points → GD → GF, with a deterministic final tiebreaker
/// (FIFA rank, then code) so tied teams never flip between polls, and renumber.
pub(crate) fn sort_standings(standings: &mut [LiveStanding]) {
    for s in standings.iter_mut() {
        s.teams.sort_by(|x, y| {
            y.points
                .cmp(&x.points)
                .then(y.goal_diff.cmp(&x.goal_diff))
                .then(y.goals_for.cmp(&x.goals_for))
                .then_with(|| fifa_rank(&x.code).cmp(&fifa_rank(&y.code)))
                .then_with(|| x.code.cmp(&y.code))
        });
        for (i, t) in s.teams.iter_mut().enumerate() {
            t.position = (i + 1) as u32;
        }
    }
}

/// Apply in-progress (LIVE) scores onto a copy of the standings and re-sort —
/// the "possible new positions". Used only for the projection window; the main
/// panel stays feed-driven to avoid any double-count.
pub(crate) fn project_standings(
    base: &[LiveStanding],
    fixtures: &[LiveFixture],
) -> Vec<LiveStanding> {
    let mut proj = base.to_vec();
    for f in fixtures.iter().filter(|f| f.status == "LIVE") {
        let Some((h, a)) = f.score else { continue };
        for s in &mut proj {
            for t in &mut s.teams {
                let (scored, conceded, win) = if t.code == f.home_code {
                    (h, a, h.cmp(&a))
                } else if t.code == f.away_code {
                    (a, h, a.cmp(&h))
                } else {
                    continue;
                };
                t.played += 1;
                t.goals_for += scored;
                t.goals_against += conceded;
                t.goal_diff = t.goals_for - t.goals_against;
                t.points += match win {
                    std::cmp::Ordering::Greater => 3,
                    std::cmp::Ordering::Equal => 1,
                    std::cmp::Ordering::Less => 0,
                };
            }
        }
    }
    // Stable sort on the official stat keys only: tied teams keep the order they
    // already have in the (already fully-sorted) base standings, so the projection
    // uses the exact same ranking as the standings table — only real goals move a team.
    for s in &mut proj {
        s.teams.sort_by(|x, y| {
            y.points
                .cmp(&x.points)
                .then(y.goal_diff.cmp(&x.goal_diff))
                .then(y.goals_for.cmp(&x.goals_for))
        });
        for (i, t) in s.teams.iter_mut().enumerate() {
            t.position = (i + 1) as u32;
        }
    }
    proj
}

/// Rank every group's 3rd-place team by the FIFA tiebreakers, in order: points,
/// goal difference, goals scored, fewest disciplinary points, then FIFA ranking.
/// The best 8 of 12 advance.
pub(crate) fn third_place_ranking(standings: &[LiveStanding]) -> Vec<ThirdPlaceRank> {
    let mut ranks: Vec<ThirdPlaceRank> = standings
        .iter()
        .filter_map(|s| {
            let t = s
                .teams
                .iter()
                .find(|t| t.position == 3)
                .or_else(|| s.teams.get(2))?;
            Some(ThirdPlaceRank {
                group: s.group,
                code: t.code.clone(),
                name: t.name.clone(),
                played: t.played,
                points: t.points,
                goal_diff: t.goal_diff,
                goals_for: t.goals_for,
                disciplinary: t.disciplinary,
                advances: false,
            })
        })
        .collect();

    ranks.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then(b.goal_diff.cmp(&a.goal_diff))
            .then(b.goals_for.cmp(&a.goals_for))
            // Fewer disciplinary points is better.
            .then(a.disciplinary.cmp(&b.disciplinary))
            // Better (lower) FIFA ranking breaks any remaining tie.
            .then(fifa_rank(&a.code).cmp(&fifa_rank(&b.code)))
    });
    for (i, r) in ranks.iter_mut().enumerate() {
        r.advances = i < 8;
    }
    ranks
}

/// A finished knockout match: the two teams and who won (by code).
#[derive(Clone)]
pub(crate) struct LiveResult {
    pub(crate) home: String,
    pub(crate) away: String,
    pub(crate) winner: Option<String>,
}

/// A fixture scheduled for today (kickoff time, teams, status, live/final score).
#[derive(Clone)]
pub(crate) struct LiveFixture {
    pub(crate) home: String,
    pub(crate) away: String,
    pub(crate) home_code: String,
    pub(crate) away_code: String,
    pub(crate) status: String,
    pub(crate) score: Option<(i64, i64)>,
}

/// Everything one sync returns: standings, finished results, today's fixtures.
pub(crate) struct LiveData {
    pub(crate) standings: Vec<LiveStanding>,
    pub(crate) results: Vec<LiveResult>,
    pub(crate) today: Vec<LiveFixture>,
    /// Per-request log lines for this sync (endpoint, result).
    pub(crate) log: Vec<String>,
}

/// Message sent from the worker thread back to the UI.
pub(crate) enum LiveMsg {
    Data(LiveData),
    Error(String),
}

/// A source of live data. Implementors run on a worker thread.
pub(crate) trait StatsProvider: Send {
    fn fetch(&self) -> Result<LiveData, String>;
}

/// football-data.org provider (competition `WC`). Needs a free API token.
pub(crate) struct FootballData {
    pub(crate) token: String,
}

impl FootballData {
    fn get_json(&self, url: &str) -> Result<serde_json::Value, String> {
        let mut res = ureq::get(url)
            .header("X-Auth-Token", &self.token)
            .call()
            .map_err(|e| format!("Request failed: {e}"))?;
        res.body_mut()
            .read_json()
            .map_err(|e| format!("Bad response: {e}"))
    }

    /// Like `get_json` but appends a timestamped log line for the request.
    fn logged_get(
        &self,
        name: &str,
        url: &str,
        log: &mut Vec<String>,
    ) -> Result<serde_json::Value, String> {
        let t = chrono::Local::now().format("%H:%M:%S");
        match self.get_json(url) {
            Ok(j) => {
                log.push(format!("{t}  GET {name}  →  OK"));
                Ok(j)
            }
            Err(e) => {
                log.push(format!("{t}  GET {name}  →  ERR: {e}"));
                Err(e)
            }
        }
    }
}

impl StatsProvider for FootballData {
    fn fetch(&self) -> Result<LiveData, String> {
        let mut log = Vec::new();

        // football-data is best-effort: if it fails (no/invalid token, rate limit),
        // we still fetch ESPN below for today's fixtures and live scores.
        let mut standings = self
            .logged_get(
                "competitions/WC/standings",
                "https://api.football-data.org/v4/competitions/WC/standings",
                &mut log,
            )
            .ok()
            .and_then(|j| parse_standings(&j).ok())
            .unwrap_or_default();

        let matches_json = self
            .logged_get(
                "competitions/WC/matches",
                "https://api.football-data.org/v4/competitions/WC/matches",
                &mut log,
            )
            .ok();

        // Rebuild stats from FINISHED matches only, so the table reflects a fixed
        // pre-kickoff baseline (the feed's standings update live, which would make
        // projection double-count). Rosters/groups still come from the feed.
        if let Some(json) = &matches_json {
            let discipline = tally_discipline(json);
            let finished = finished_stats(json);
            for s in &mut standings {
                for t in &mut s.teams {
                    let (played, points, gf, ga) =
                        finished.get(&t.code).copied().unwrap_or((0, 0, 0, 0));
                    t.played = played;
                    t.points = points;
                    t.goals_for = gf;
                    t.goals_against = ga;
                    t.goal_diff = gf - ga;
                    t.disciplinary = discipline.get(&t.code).copied().unwrap_or(0);
                }
            }
            sort_standings(&mut standings);
        }
        let results = matches_json.as_ref().map(parse_matches).unwrap_or_default();

        // Today's fixtures: prefer ESPN (near-real-time in-play scores); fall back
        // to football-data's matches if ESPN is unavailable.
        let t = chrono::Local::now().format("%H:%M:%S");
        let today = match get_json_noauth(
            "https://site.api.espn.com/apis/site/v2/sports/soccer/fifa.world/scoreboard",
        ) {
            Ok(j) => {
                log.push(format!("{t}  GET ESPN scoreboard  →  OK"));
                let fx = parse_espn_today(&j);
                for line in debug_espn_raw(&fx) {
                    log.push(line);
                }
                fx
            }
            Err(e) => {
                log.push(format!("{t}  GET ESPN scoreboard  →  ERR: {e}"));
                let fx = matches_json.as_ref().map(parse_today).unwrap_or_default();
                if let Some(json) = &matches_json {
                    for line in debug_today_raw(json) {
                        log.push(line);
                    }
                }
                fx
            }
        };
        log.push(format!(
            "          parsed: {} groups, {} results, {} today",
            standings.len(),
            results.len(),
            today.len()
        ));
        Ok(LiveData {
            standings,
            results,
            today,
            log,
        })
    }
}

/// A keyless GET (for ESPN's public scoreboard endpoint).
fn get_json_noauth(url: &str) -> Result<serde_json::Value, String> {
    let mut res = ureq::get(url)
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;
    res.body_mut()
        .read_json()
        .map_err(|e| format!("Bad response: {e}"))
}

/// Parse ESPN's UTC timestamp (which omits seconds, e.g. `2026-06-18T16:00Z`)
/// into local time.
fn parse_espn_time(raw: &str) -> Option<chrono::DateTime<chrono::Local>> {
    use chrono::{Local, NaiveDateTime, TimeZone, Utc};
    let s = raw.trim_end_matches('Z');
    let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M"))
        .ok()?;
    Some(Utc.from_utc_datetime(&naive).with_timezone(&Local))
}

/// ESPN's near-real-time scoreboard for today's fixtures (state in/post/pre).
fn parse_espn_today(json: &serde_json::Value) -> Vec<LiveFixture> {
    let today = chrono::Local::now().date_naive();
    let score_of = |c: &serde_json::Value| -> Option<i64> {
        c["score"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .or_else(|| c["score"].as_i64())
    };
    let mut out = Vec::new();
    for e in json["events"].as_array().into_iter().flatten() {
        let Some(local) = parse_espn_time(e["date"].as_str().unwrap_or("")) else {
            continue;
        };
        if local.date_naive() != today {
            continue;
        }
        let comp = &e["competitions"][0];
        let mut home = None;
        let mut away = None;
        for c in comp["competitors"].as_array().into_iter().flatten() {
            match c["homeAway"].as_str() {
                Some("home") => home = Some(c),
                Some("away") => away = Some(c),
                _ => {}
            }
        }
        let (Some(h), Some(a)) = (home, away) else {
            continue;
        };
        let state = e["status"]["type"]["state"].as_str().unwrap_or("");
        let status = match state {
            "in" => "LIVE".to_string(),
            "post" => "FT".to_string(),
            _ => local.format("%H:%M").to_string(),
        };
        let score = if state == "pre" {
            None
        } else {
            match (score_of(h), score_of(a)) {
                (Some(hh), Some(aa)) => Some((hh, aa)),
                _ => None,
            }
        };
        out.push(LiveFixture {
            home: h["team"]["displayName"]
                .as_str()
                .unwrap_or("TBD")
                .to_string(),
            away: a["team"]["displayName"]
                .as_str()
                .unwrap_or("TBD")
                .to_string(),
            home_code: canonical_code(h["team"]["abbreviation"].as_str().unwrap_or("")),
            away_code: canonical_code(a["team"]["abbreviation"].as_str().unwrap_or("")),
            status,
            score,
        });
    }
    out
}

/// Raw debug lines for ESPN's today fixtures (status / score).
fn debug_espn_raw(fixtures: &[LiveFixture]) -> Vec<String> {
    let mut out = vec!["          ESPN today (status / score):".to_string()];
    for f in fixtures {
        let score = match f.score {
            Some((h, a)) => format!("{h}-{a}"),
            None => "-".to_string(),
        };
        out.push(format!(
            "            {:11} {} {} {}",
            f.status, f.home_code, score, f.away_code
        ));
    }
    if out.len() == 1 {
        out.push("            (no matches today)".to_string());
    }
    out
}

/// Raw debug lines for today's matches: local time, the provider's exact status,
/// the teams, and the score — so you can verify when the API goes live.
fn debug_today_raw(json: &serde_json::Value) -> Vec<String> {
    use chrono::{DateTime, Local};
    let today = Local::now().date_naive();
    let mut out = vec!["          today's matches (raw status / score):".to_string()];
    for m in json["matches"].as_array().into_iter().flatten() {
        let Ok(dt) = DateTime::parse_from_rfc3339(m["utcDate"].as_str().unwrap_or("")) else {
            continue;
        };
        let local = dt.with_timezone(&Local);
        if local.date_naive() != today {
            continue;
        }
        let status = m["status"].as_str().unwrap_or("?");
        let home = m["homeTeam"]["tla"]
            .as_str()
            .or_else(|| m["homeTeam"]["name"].as_str())
            .unwrap_or("TBD");
        let away = m["awayTeam"]["tla"]
            .as_str()
            .or_else(|| m["awayTeam"]["name"].as_str())
            .unwrap_or("TBD");
        let score = match (
            m["score"]["fullTime"]["home"].as_i64(),
            m["score"]["fullTime"]["away"].as_i64(),
        ) {
            (Some(h), Some(a)) => format!("{h}-{a}"),
            _ => "-".to_string(),
        };
        out.push(format!(
            "            {}  {:11} {} {} {}",
            local.format("%H:%M"),
            status,
            home,
            score,
            away
        ));
    }
    if out.len() == 1 {
        out.push("            (no matches today)".to_string());
    }
    out
}

/// Parse matches kicking off today (local time) into fixtures with local kickoff.
fn parse_today(json: &serde_json::Value) -> Vec<LiveFixture> {
    use chrono::{DateTime, Local};
    let today = Local::now().date_naive();
    json["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            // Convert the match's UTC kickoff to the machine's local timezone.
            let local = DateTime::parse_from_rfc3339(m["utcDate"].as_str().unwrap_or(""))
                .ok()?
                .with_timezone(&Local);
            if local.date_naive() != today {
                return None;
            }
            let kickoff = local.format("%H:%M").to_string();
            let status = match m["status"].as_str().unwrap_or("") {
                "IN_PLAY" | "PAUSED" => "LIVE".to_string(),
                "FINISHED" => "FT".to_string(),
                _ => kickoff,
            };
            let score = match (
                m["score"]["fullTime"]["home"].as_i64(),
                m["score"]["fullTime"]["away"].as_i64(),
            ) {
                (Some(h), Some(a)) => Some((h, a)),
                _ => None,
            };
            Some(LiveFixture {
                home: m["homeTeam"]["name"].as_str().unwrap_or("TBD").to_string(),
                away: m["awayTeam"]["name"].as_str().unwrap_or("TBD").to_string(),
                home_code: canonical_code(m["homeTeam"]["tla"].as_str().unwrap_or("")),
                away_code: canonical_code(m["awayTeam"]["tla"].as_str().unwrap_or("")),
                status,
                score,
            })
        })
        .collect()
}

/// Tally (played, points, goals-for, goals-against) per team from FINISHED
/// group-stage matches — the pre-kickoff baseline, excluding any in-play game.
fn finished_stats(
    json: &serde_json::Value,
) -> std::collections::HashMap<String, (u32, i64, i64, i64)> {
    let mut map: std::collections::HashMap<String, (u32, i64, i64, i64)> =
        std::collections::HashMap::new();
    for m in json["matches"].as_array().into_iter().flatten() {
        if m["stage"].as_str() != Some("GROUP_STAGE") || m["status"].as_str() != Some("FINISHED") {
            continue;
        }
        let (Some(h), Some(a)) = (
            m["score"]["fullTime"]["home"].as_i64(),
            m["score"]["fullTime"]["away"].as_i64(),
        ) else {
            continue;
        };
        let home = canonical_code(m["homeTeam"]["tla"].as_str().unwrap_or(""));
        let away = canonical_code(m["awayTeam"]["tla"].as_str().unwrap_or(""));
        for (code, scored, conceded) in [(home, h, a), (away, a, h)] {
            if code.is_empty() {
                continue;
            }
            let e = map.entry(code).or_insert((0, 0, 0, 0));
            e.0 += 1;
            e.1 += match scored.cmp(&conceded) {
                std::cmp::Ordering::Greater => 3,
                std::cmp::Ordering::Equal => 1,
                std::cmp::Ordering::Less => 0,
            };
            e.2 += scored;
            e.3 += conceded;
        }
    }
    map
}

/// Sum group-stage fair-play disciplinary points per team from match bookings.
/// Approximate FIFA scale: yellow 1, second-yellow red 3, direct red 4.
fn tally_discipline(json: &serde_json::Value) -> std::collections::HashMap<String, i64> {
    let mut map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for m in json["matches"].as_array().into_iter().flatten() {
        if m["stage"].as_str() != Some("GROUP_STAGE") {
            continue;
        }
        for b in m["bookings"].as_array().into_iter().flatten() {
            let code = canonical_code(b["team"]["tla"].as_str().unwrap_or(""));
            if code.is_empty() {
                continue;
            }
            let points = match b["card"].as_str() {
                Some("YELLOW_CARD") => 1,
                Some("YELLOW_RED_CARD") => 3,
                Some("RED_CARD") => 4,
                _ => 0,
            };
            *map.entry(code).or_insert(0) += points;
        }
    }
    map
}

/// Parse football-data's `/matches` payload into finished knockout results.
fn parse_matches(json: &serde_json::Value) -> Vec<LiveResult> {
    json["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let stage = m["stage"].as_str().unwrap_or("");
            if stage.is_empty() || stage == "GROUP_STAGE" {
                return None;
            }
            let home = canonical_code(m["homeTeam"]["tla"].as_str().unwrap_or(""));
            let away = canonical_code(m["awayTeam"]["tla"].as_str().unwrap_or(""));
            if home.is_empty() || away.is_empty() {
                return None;
            }
            let winner = match m["score"]["winner"].as_str() {
                Some("HOME_TEAM") => Some(home.clone()),
                Some("AWAY_TEAM") => Some(away.clone()),
                _ => None,
            };
            Some(LiveResult { home, away, winner })
        })
        .collect()
}

/// Normalize a provider's team code to our 3-letter seed code.
/// Uppercases/trims and applies overrides for any provider that differs from
/// the FIFA TLA we seed with. Extend `OVERRIDES` if a team lands mismatched.
pub(crate) fn canonical_code(raw: &str) -> String {
    const OVERRIDES: &[(&str, &str)] = &[
        // (provider TLA, our seed code) — add entries here when codes disagree.
        ("KVX", "KOS"), // example: Kosovo (not in 2026, illustrative)
    ];
    let up = raw.trim().to_ascii_uppercase();
    OVERRIDES
        .iter()
        .find(|(from, _)| *from == up)
        .map(|(_, to)| (*to).to_string())
        .unwrap_or(up)
}

/// Parse football-data's `/standings` payload into our `LiveStanding`s.
fn parse_standings(json: &serde_json::Value) -> Result<Vec<LiveStanding>, String> {
    let groups = json["standings"]
        .as_array()
        .ok_or("No 'standings' array in response")?;

    let mut out = Vec::new();
    for s in groups {
        let group_name = s["group"].as_str().unwrap_or("");
        // e.g. "GROUP_A" -> 'A'
        let Some(group) = group_name
            .chars()
            .rev()
            .find(|c| c.is_ascii_alphabetic())
            .map(|c| c.to_ascii_uppercase())
        else {
            continue;
        };

        let teams = s["table"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|r| LiveTeam {
                        name: r["team"]["name"].as_str().unwrap_or("").to_string(),
                        code: canonical_code(r["team"]["tla"].as_str().unwrap_or("")),
                        position: r["position"].as_u64().unwrap_or(0) as u32,
                        played: r["playedGames"].as_u64().unwrap_or(0) as u32,
                        points: r["points"].as_i64().unwrap_or(0),
                        goal_diff: r["goalDifference"].as_i64().unwrap_or(0),
                        goals_for: r["goalsFor"].as_i64().unwrap_or(0),
                        goals_against: r["goalsAgainst"].as_i64().unwrap_or(0),
                        // Card data isn't in the standings feed; sourced separately later.
                        disciplinary: 0,
                    })
                    .collect()
            })
            .unwrap_or_default();

        out.push(LiveStanding { group, teams });
    }

    if out.is_empty() {
        return Err("No group standings available yet".to_string());
    }
    out.sort_by_key(|s| s.group);
    Ok(out)
}

/// Spawn a one-shot background fetch; the UI polls the returned receiver.
/// `ctx` is used to wake the UI when the result lands.
pub(crate) fn spawn_fetch(
    provider: Box<dyn StatsProvider>,
    ctx: egui::Context,
) -> Receiver<LiveMsg> {
    let (tx, rx) = channel();
    thread::spawn(move || {
        let msg = match provider.fetch() {
            Ok(data) => LiveMsg::Data(data),
            Err(e) => LiveMsg::Error(e),
        };
        let _ = tx.send(msg);
        ctx.request_repaint();
    });
    rx
}

/// Bottom-center window listing today's fixtures with live/final scores.
/// One movable, collapsible Live Center: today's games on top, a 4×3 grid of
/// projected group tables on the left, and the 3rd-place ranking on the right.
pub(crate) fn live_center_window(app: &mut PredictorApp, ctx: &egui::Context) {
    if !app.show_live_center {
        return;
    }
    let pal = app.pal();
    let fixtures = app.today_fixtures.clone();
    // Standings are finished-only; this grid is the in-play "what-if" projection,
    // with arrows comparing the projection back to the finished standings.
    let proj = project_standings(&app.live_standings, &app.today_fixtures);
    let feed: std::collections::HashMap<String, u32> = app
        .live_standings
        .iter()
        .flat_map(|s| s.teams.iter().map(|t| (t.code.clone(), t.position)))
        .collect();
    // Codes of teams currently in a live fixture (for the breathing highlight).
    let playing: std::collections::HashSet<String> = fixtures
        .iter()
        .filter(|f| f.status == "LIVE")
        .flat_map(|f| [f.home_code.clone(), f.away_code.clone()])
        .collect();
    let mut open = true;
    let screen = ctx.screen_rect();

    egui::Window::new(RichText::new("Live center").size(14.0))
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_pos(egui::pos2((screen.center().x - 540.0).max(20.0), 60.0))
        .default_size(egui::vec2(1060.0, (screen.height() - 120.0).max(420.0)))
        .frame(
            egui::Frame::new()
                .fill(pal.panel)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(12)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(1000.0);
            egui::ScrollArea::vertical()
                .max_height((screen.height() - 150.0).max(320.0))
                .show(ui, |ui| {
                    // ── Today's games (with live scores) ──
                    if !fixtures.is_empty() {
                        ui.label(
                            RichText::new("Today's games & live scores (local time)")
                                .size(12.0)
                                .strong()
                                .color(pal.dim),
                        );
                        ui.add_space(2.0);
                        for f in &fixtures {
                            fixture_row(ui, f, pal);
                        }
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);
                    }

                    if proj.is_empty() {
                        ui.label(
                            RichText::new("Sync live data to see possible standings.")
                                .size(12.0)
                                .color(pal.dim),
                        );
                        return;
                    }

                    // ── Projected group tables (left) + 3rd-place race (right) ──
                    ui.label(
                        RichText::new("Possible final standings (if live scores hold)")
                            .size(12.0)
                            .strong()
                            .color(pal.dim),
                    );
                    ui.add_space(4.0);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            egui::Grid::new("live_center_groups")
                                .spacing(egui::vec2(8.0, 8.0))
                                .show(ui, |ui| {
                                    for (i, s) in proj.iter().enumerate() {
                                        projected_group_card(ui, s, &feed, &playing, pal);
                                        if (i + 1) % 4 == 0 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        });
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("3rd-place ranking — top 8 advance")
                                    .size(12.0)
                                    .strong()
                                    .color(pal.text),
                            );
                            ui.add_space(4.0);
                            third_place_table(ui, app, pal);
                        });
                    });
                });
        });

    if !open {
        app.show_live_center = false;
    }
}

/// One today's-fixture row; LIVE matches get a breathing green background.
fn fixture_row(ui: &mut egui::Ui, f: &LiveFixture, pal: Palette) {
    let live = f.status == "LIVE";
    let fill = if live {
        let t = ui.input(|i| i.time);
        let pulse = ((t * 2.4).sin() * 0.5 + 0.5) as f32;
        let alpha = (40.0 + 50.0 * pulse) as u8;
        ui.ctx().request_repaint();
        Color32::from_rgba_unmultiplied(22, 163, 74, alpha)
    } else {
        Color32::TRANSPARENT
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(5.0)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let chip = if live { COLOR_RED } else { pal.dim };
                ui.add_sized(
                    [44.0, 16.0],
                    egui::Label::new(RichText::new(&f.status).size(11.0).strong().color(chip)),
                );
                flag_image(ui, &f.home_code, egui::vec2(18.0, 12.0));
                ui.add_sized(
                    [120.0, 16.0],
                    egui::Label::new(RichText::new(&f.home).size(12.0).color(pal.text)),
                );
                let mid = match f.score {
                    Some((h, a)) => format!("{h} – {a}"),
                    None => "v".to_string(),
                };
                ui.add_sized(
                    [44.0, 16.0],
                    egui::Label::new(
                        RichText::new(mid)
                            .size(12.0)
                            .strong()
                            .monospace()
                            .color(pal.text),
                    ),
                );
                ui.add_sized(
                    [120.0, 16.0],
                    egui::Label::new(RichText::new(&f.away).size(12.0).color(pal.text)),
                );
                flag_image(ui, &f.away_code, egui::vec2(18.0, 12.0));
            });
        });
}

/// One projected group card with ▲/▼ arrows vs the current (feed) position.
fn projected_group_card(
    ui: &mut egui::Ui,
    s: &LiveStanding,
    feed: &std::collections::HashMap<String, u32>,
    playing: &std::collections::HashSet<String>,
    pal: Palette,
) {
    egui::Frame::new()
        .fill(pal.card)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(7))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(176.0);
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(
                    RichText::new(format!("Group {}", s.group))
                        .size(11.0)
                        .strong()
                        .color(pal.dim),
                );
                for t in &s.teams {
                    let (arrow, ac) = match feed.get(&t.code).copied() {
                        Some(old) if t.position < old => ("+", COLOR_GREEN),
                        Some(old) if t.position > old => ("-", COLOR_RED),
                        _ => (" ", pal.dim),
                    };
                    // Teams in a live game get a breathing green row.
                    let fill = if playing.contains(&t.code) {
                        let time = ui.input(|i| i.time);
                        let pulse = ((time * 2.4).sin() * 0.5 + 0.5) as f32;
                        let alpha = (38.0 + 46.0 * pulse) as u8;
                        ui.ctx().request_repaint();
                        Color32::from_rgba_unmultiplied(22, 163, 74, alpha)
                    } else {
                        Color32::TRANSPARENT
                    };
                    egui::Frame::new()
                        .fill(fill)
                        .corner_radius(3.0)
                        .inner_margin(egui::Margin::symmetric(2, 1))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                ui.label(RichText::new(arrow).size(11.0).strong().color(ac));
                                flag_image(ui, &t.code, egui::vec2(18.0, 12.0));
                                ui.add_sized(
                                    [34.0, 14.0],
                                    egui::Label::new(
                                        RichText::new(&t.code)
                                            .monospace()
                                            .size(12.0)
                                            .color(pal.text),
                                    ),
                                );
                                ui.label(
                                    RichText::new(format!("{}p {:+}", t.points, t.goal_diff))
                                        .size(10.0)
                                        .color(pal.dim),
                                );
                            });
                        });
                }
            });
        });
}

/// The Live-data window: API token entry + "Sync now" + status.
pub(crate) fn live_window(app: &mut PredictorApp, ctx: &egui::Context) {
    if !app.show_live {
        return;
    }
    let pal = app.pal();
    let mut open = true;

    egui::Window::new("Live data (football-data.org)")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::new()
                .fill(pal.panel)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(14)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.label(
                RichText::new("Pull real group standings and apply them to the bracket.")
                    .size(12.0)
                    .color(pal.dim),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("API token")
                    .size(12.0)
                    .strong()
                    .color(pal.text),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.api_key)
                    .password(true)
                    .hint_text("football-data.org token")
                    .desired_width(340.0),
            );
            ui.add_space(8.0);

            let syncing = app.live_rx.is_some();
            ui.horizontal(|ui| {
                let label = if syncing { "Syncing…" } else { "Sync now" };
                if ui
                    .add_enabled(
                        !syncing,
                        egui::Button::new(
                            RichText::new(label)
                                .size(13.0)
                                .color(crate::theme::COLOR_GREEN),
                        )
                        .fill(pal.card)
                        .stroke(Stroke::new(1.0, pal.border)),
                    )
                    .clicked()
                {
                    app.start_live_sync(ctx.clone(), "manual");
                }
            });

            ui.add_space(4.0);
            let mut live_mode = app.live_mode;
            if ui
                .checkbox(&mut live_mode, "Live mode — auto-poll every 20s + alerts")
                .changed()
            {
                app.live_mode = live_mode;
                app.last_poll = None; // poll right away when turned on
                if live_mode {
                    app.toasts.push(Toast::new(
                        "Live mode on — polling every 20s".to_string(),
                        AlertKind::Info,
                    ));
                }
            }
            ui.checkbox(
                &mut app.show_live_center,
                "Show Live Center (games, standings, 3rd place)",
            );

            if let Some(status) = &app.live_status {
                ui.add_space(8.0);
                ui.label(RichText::new(status).size(12.0).color(pal.dim));
            }

            // API call log.
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("API log").size(12.0).strong().color(pal.text));
                if ui.button(RichText::new("clear").size(10.0)).clicked() {
                    app.api_log.clear();
                }
            });
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &app.api_log {
                        ui.label(RichText::new(line).size(10.5).monospace().color(pal.dim));
                    }
                });
        });

    if !open {
        app.show_live = false;
    }
}

/// The cross-group 3rd-place ranking, with a cutoff line after the top 8.
fn third_place_table(ui: &mut egui::Ui, app: &PredictorApp, pal: Palette) {
    for (i, r) in app.third_rank.iter().enumerate() {
        if i == 8 {
            ui.add_space(3.0);
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width().min(460.0), 2.0),
                Sense::hover(),
            );
            ui.painter()
                .hline(rect.x_range(), rect.center().y, Stroke::new(1.5, COLOR_RED));
            ui.label(
                RichText::new("cutoff — below this line is eliminated")
                    .size(9.0)
                    .color(COLOR_RED),
            );
            ui.add_space(3.0);
        }
        let color = if r.advances { COLOR_GREEN } else { COLOR_RED };
        // Movement arrow vs the previous poll.
        let (arrow, arrow_color) = match app.third_delta.get(&r.code).copied() {
            Some(d) if d < 0 => ("+", COLOR_GREEN),
            Some(d) if d > 0 => ("-", COLOR_RED),
            Some(_) => ("=", pal.dim),
            None => (" ", pal.dim),
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(RichText::new(arrow).size(12.0).strong().color(arrow_color));
            ui.label(
                RichText::new(format!("{:>2}", i + 1))
                    .monospace()
                    .size(12.0)
                    .color(pal.dim),
            );
            ui.label(
                RichText::new(r.group.to_string())
                    .monospace()
                    .size(12.0)
                    .strong()
                    .color(pal.dim),
            );
            flag_image(ui, &r.code, egui::vec2(20.0, 13.0));
            ui.add_sized(
                [120.0, 16.0],
                egui::Label::new(RichText::new(&r.name).size(12.0).color(pal.text)),
            );
            ui.label(
                RichText::new(format!("Pld {}", r.played))
                    .size(11.0)
                    .color(pal.dim),
            );
            ui.label(
                RichText::new(format!("Pts {}", r.points))
                    .size(11.0)
                    .strong()
                    .color(pal.text),
            );
            ui.label(
                RichText::new(format!("GD {:+}", r.goal_diff))
                    .size(11.0)
                    .color(pal.dim),
            );
            ui.label(
                RichText::new(format!("GF {}", r.goals_for))
                    .size(11.0)
                    .color(pal.dim),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(if r.advances { "ADV" } else { "OUT" })
                        .size(11.0)
                        .strong()
                        .color(color),
                );
            });
        });
    }
}
