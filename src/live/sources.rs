//! Live data sources (football-data.org + ESPN), fetched concurrently.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};
use std::thread;

use eframe::egui;

use super::*;

/// A source of live data. Sources run concurrently on worker threads, so they
/// must be `Sync`. Each contributes part of a `LiveData`; failures are logged,
/// never fatal.
pub(crate) trait LiveSource: Sync {
    fn fetch(&self) -> SourceData;
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

/// football-data.org provider (competition `WC`). Needs a free API token.
/// Supplies group standings, knockout results, and a fixtures fallback.
pub(crate) struct FootballData {
    pub(crate) token: String,
}

impl FootballData {
    fn logged_get(
        &self,
        name: &str,
        url: &str,
        log: &mut Vec<String>,
    ) -> Option<serde_json::Value> {
        let t = chrono::Local::now().format("%H:%M:%S");
        let result = ureq::get(url)
            .header("X-Auth-Token", &self.token)
            .call()
            .map_err(|e| format!("Request failed: {e}"))
            .and_then(|mut r| {
                r.body_mut()
                    .read_json()
                    .map_err(|e| format!("Bad response: {e}"))
            });
        match result {
            Ok(j) => {
                log.push(format!("{t}  GET {name}  →  OK"));
                Some(j)
            }
            Err(e) => {
                log.push(format!("{t}  GET {name}  →  ERR: {e}"));
                None
            }
        }
    }
}

impl LiveSource for FootballData {
    fn fetch(&self) -> SourceData {
        let mut log = Vec::new();
        // One request: the full match list. Standings are derived from it, which
        // both halves our request count (free tier: 10/min) and avoids the
        // separate /standings endpoint that can 400.
        let matches_json = self.logged_get(
            "competitions/WC/matches",
            "https://api.football-data.org/v4/competitions/WC/matches",
            &mut log,
        );

        let finished = matches_json
            .as_ref()
            .map(finished_from_matches)
            .unwrap_or_default();
        let discipline = matches_json.as_ref().map(tally_discipline).unwrap_or_default();
        let results = matches_json.as_ref().map(parse_matches).unwrap_or_default();
        // football-data fixtures are only a fallback; ESPN is preferred.
        let today = matches_json.as_ref().map(parse_today).unwrap_or_default();
        let remaining = matches_json
            .as_ref()
            .map(remaining_group_matches)
            .unwrap_or_default();
        SourceData {
            finished,
            discipline,
            results,
            today,
            remaining,
            log,
        }
    }
}

/// ESPN's public scoreboard — keyless, near-real-time today's fixtures.
pub(crate) struct Espn;

impl LiveSource for Espn {
    fn fetch(&self) -> SourceData {
        // Local "today" can span two UTC dates (a late-evening kickoff is the next
        // UTC day). Query a UTC range around now so those games aren't missed; the
        // parser then keeps only matches whose *local* date is today.
        let utc = chrono::Utc::now().date_naive();
        let start = (utc - chrono::Duration::days(1)).format("%Y%m%d");
        let end = (utc + chrono::Duration::days(1)).format("%Y%m%d");
        let url = format!(
            "https://site.api.espn.com/apis/site/v2/sports/soccer/fifa.world/scoreboard?dates={start}-{end}"
        );
        let mut log = Vec::new();
        let t = chrono::Local::now().format("%H:%M:%S");
        let today = match get_json_noauth(&url) {
            Ok(j) => {
                log.push(format!("{t}  GET ESPN scoreboard  →  OK"));
                let fx = parse_espn_today(&j);
                log.extend(debug_espn_raw(&fx));
                fx
            }
            Err(e) => {
                log.push(format!("{t}  GET ESPN scoreboard  →  ERR: {e}"));
                Vec::new()
            }
        };
        let finished = finished_from_fixtures(&today);
        SourceData {
            today,
            finished,
            log,
            ..Default::default()
        }
    }
}

/// Fetch football-data and ESPN **concurrently**, then merge. ESPN's live
/// fixtures take precedence; football-data's are the fallback.
pub(crate) fn fetch_live(token: String) -> LiveData {
    let fd = FootballData { token };
    let espn = Espn;
    let (fd_data, espn_data) = thread::scope(|s| {
        let h_fd = s.spawn(|| fd.fetch());
        let h_espn = s.spawn(|| espn.fetch());
        (
            h_fd.join().unwrap_or_default(),
            h_espn.join().unwrap_or_default(),
        )
    });

    let today = if espn_data.today.is_empty() {
        fd_data.today
    } else {
        espn_data.today
    };

    let key = |m: &FinishedMatch| {
        let mut p = [m.home.clone(), m.away.clone()];
        p.sort();
        p
    };
    let pair = |h: &str, a: &str| {
        let mut p = [h.to_string(), a.to_string()];
        p.sort();
        p
    };

    // A game ESPN currently reports as LIVE must never enter the finished table —
    // its in-progress score belongs only to the in-play projection. Excluding live
    // matchups also guards against a feed briefly flapping a live game to "finished"
    // and inflating a team's goals/points in the standings.
    let live_now: std::collections::HashSet<[String; 2]> = today
        .iter()
        .filter(|f| f.status.is_live())
        .map(|f| pair(&f.home_code, &f.away_code))
        .collect();

    // Union finished matches from both sources, deduped by the unordered matchup —
    // a single feed can also return the same match twice, so dedup the whole stream.
    // football-data has the full history; ESPN supplies just-finished games it
    // hasn't caught up on yet, so an FT result lands in the table immediately.
    let mut finished: Vec<FinishedMatch> = Vec::new();
    let mut seen: std::collections::HashSet<[String; 2]> = std::collections::HashSet::new();
    let mut from_espn = 0;
    for (m, is_espn) in fd_data
        .finished
        .into_iter()
        .map(|m| (m, false))
        .chain(espn_data.finished.into_iter().map(|m| (m, true)))
    {
        let k = key(&m);
        if live_now.contains(&k) || !seen.insert(k) {
            continue;
        }
        if is_espn {
            from_espn += 1;
        }
        finished.push(m);
    }
    let standings = build_standings(&finished, &fd_data.discipline);

    // Remaining = football-data's upcoming games ESPN hasn't reported done, plus
    // any game ESPN currently shows live (so the projection can apply its score —
    // a game one feed marked finished while ESPN still has it live was excluded
    // above, so it must be (re)added here or it would vanish entirely).
    let mut remaining: Vec<GroupFixture> = fd_data
        .remaining
        .into_iter()
        .filter(|f| !seen.contains(&pair(&f.home, &f.away)))
        .collect();
    for f in today.iter().filter(|f| f.status.is_live()) {
        let g = seed_group(&f.home_code);
        if g.is_some()
            && g == seed_group(&f.away_code)
            && !remaining
                .iter()
                .any(|r| pair(&r.home, &r.away) == pair(&f.home_code, &f.away_code))
        {
            remaining.push(GroupFixture {
                home: f.home_code.clone(),
                away: f.away_code.clone(),
            });
        }
    }

    let mut log = fd_data.log;
    log.extend(espn_data.log);
    log.push(format!(
        "          parsed: {} groups, {} finished (+{} from ESPN), {} results, {} today",
        standings.len(),
        finished.len(),
        from_espn,
        fd_data.results.len(),
        today.len()
    ));
    // Surface any finished-match code that isn't one of our seed codes — a feed
    // using an alternate code (e.g. URY vs URU) silently mis-credits goals and
    // breaks matchup dedup. Add such codes to `canonical_code`'s OVERRIDES.
    {
        use std::collections::BTreeSet;
        let unknown: BTreeSet<&str> = finished
            .iter()
            .flat_map(|m| [m.home.as_str(), m.away.as_str()])
            .filter(|c| seed_group(c).is_none())
            .collect();
        if !unknown.is_empty() {
            log.push(format!(
                "          !! unknown finished-match codes (feed mismatch): {unknown:?}"
            ));
        }
    }
    // Per-team tally so a double-count is visible at a glance: any team with
    // played > 3 (impossible in the group stage) is proof a match was counted twice.
    log.push("          standings (Pld·Pts GF:GA) — flags >3 played:".to_string());
    for s in &standings {
        let row: Vec<String> = s
            .teams
            .iter()
            .filter(|t| t.played > 0)
            .map(|t| {
                let warn = if t.played > 3 { "!!" } else { "" };
                format!("{}{} {}p{} {}:{}", t.code, warn, t.played, t.points, t.goals_for, t.goals_against)
            })
            .collect();
        if !row.is_empty() {
            log.push(format!("            {}: {}", s.group, row.join("  ")));
        }
    }
    LiveData {
        standings,
        results: fd_data.results,
        today,
        remaining,
        log,
    }
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
        let state = e["status"]["type"]["state"].as_str().unwrap_or("");
        // Keep today's games — but a still-live match that kicked off late the
        // previous (local) day stays shown until it finishes.
        if local.date_naive() != today && state != "in" {
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
        let status = match state {
            "in" => MatchStatus::Live,
            "post" => MatchStatus::Finished,
            _ => MatchStatus::Scheduled(local.format("%H:%M").to_string()),
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
            f.status.label(),
            f.home_code,
            score,
            f.away_code
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
                "IN_PLAY" | "PAUSED" => MatchStatus::Live,
                "FINISHED" => MatchStatus::Finished,
                _ => MatchStatus::Scheduled(kickoff),
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
fn finished_stats(matches: &[FinishedMatch]) -> HashMap<String, (u32, i64, i64, i64)> {
    let mut map: HashMap<String, (u32, i64, i64, i64)> = HashMap::new();
    for m in matches {
        for (code, scored, conceded) in [
            (&m.home, m.home_goals, m.away_goals),
            (&m.away, m.away_goals, m.home_goals),
        ] {
            if code.is_empty() {
                continue;
            }
            let e = map.entry(code.clone()).or_insert((0, 0, 0, 0));
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

/// Parse football-data's FINISHED group matches into `FinishedMatch`es.
fn finished_from_matches(json: &serde_json::Value) -> Vec<FinishedMatch> {
    json["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| {
            m["stage"].as_str() == Some("GROUP_STAGE")
                && m["status"].as_str() == Some("FINISHED")
        })
        .filter_map(|m| {
            let (h, a) = (
                m["score"]["fullTime"]["home"].as_i64()?,
                m["score"]["fullTime"]["away"].as_i64()?,
            );
            let home = canonical_code(m["homeTeam"]["tla"].as_str().unwrap_or(""));
            let away = canonical_code(m["awayTeam"]["tla"].as_str().unwrap_or(""));
            (!home.is_empty() && !away.is_empty()).then_some(FinishedMatch {
                home,
                away,
                home_goals: h,
                away_goals: a,
            })
        })
        .collect()
}

/// The seed group letter for a code (None if unknown).
fn seed_group(code: &str) -> Option<char> {
    fifa_team3::SEED_TEAMS
        .iter()
        .find(|(_, _, c, _)| *c == code)
        .map(|(g, _, _, _)| *g)
}

/// Pull FINISHED group matchups from today's fixtures (ESPN) — only true
/// group-stage pairings (both teams in the same seed group), so a knockout FT
/// is never counted into the group table.
fn finished_from_fixtures(today: &[LiveFixture]) -> Vec<FinishedMatch> {
    today
        .iter()
        .filter_map(|f| {
            if f.status != MatchStatus::Finished {
                return None;
            }
            let (h, a) = f.score?;
            let g = seed_group(&f.home_code)?;
            if seed_group(&f.away_code) != Some(g) {
                return None;
            }
            Some(FinishedMatch {
                home: f.home_code.clone(),
                away: f.away_code.clone(),
                home_goals: h,
                away_goals: a,
            })
        })
        .collect()
}

/// Sum group-stage fair-play disciplinary points per team from match bookings.
/// Approximate FIFA scale: yellow 1, second-yellow red 3, direct red 4.
fn tally_discipline(json: &serde_json::Value) -> HashMap<String, i64> {
    let mut map: HashMap<String, i64> = HashMap::new();
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

/// Build group standings from the unioned finished matches + discipline map.
/// Group membership comes from the authoritative seed (all four teams always
/// present); stats come from the finished results.
fn build_standings(
    finished: &[FinishedMatch],
    disc: &HashMap<String, i64>,
) -> Vec<LiveStanding> {
    use std::collections::BTreeMap;
    let stats = finished_stats(finished);
    let mut groups: BTreeMap<char, Vec<LiveTeam>> = BTreeMap::new();
    for &(g, name, code, _flag) in fifa_team3::SEED_TEAMS.iter() {
        let (played, points, gf, ga) = stats.get(code).copied().unwrap_or((0, 0, 0, 0));
        groups.entry(g).or_default().push(LiveTeam {
            name: name.to_string(),
            code: code.to_string(),
            position: 0,
            played,
            points,
            goals_for: gf,
            goals_against: ga,
            goal_diff: gf - ga,
            disciplinary: disc.get(code).copied().unwrap_or(0),
        });
    }
    let mut out: Vec<LiveStanding> = groups
        .into_iter()
        .map(|(group, teams)| LiveStanding { group, teams })
        .collect();
    sort_standings(&mut out);
    out
}

/// Collect group-stage matches that haven't finished yet (the scenario engine's
/// remaining fixtures). Anything not `FINISHED` counts as still to be played.
fn remaining_group_matches(json: &serde_json::Value) -> Vec<GroupFixture> {
    json["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| {
            m["stage"].as_str() == Some("GROUP_STAGE")
                && m["status"].as_str() != Some("FINISHED")
        })
        .filter_map(|m| {
            let home = canonical_code(m["homeTeam"]["tla"].as_str().unwrap_or(""));
            let away = canonical_code(m["awayTeam"]["tla"].as_str().unwrap_or(""));
            (!home.is_empty() && !away.is_empty()).then_some(GroupFixture { home, away })
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
        // football-data uses ISO-style codes for some teams; map to our FIFA seed.
        ("URY", "URU"), // Uruguay
        ("CUR", "CUW"), // Curaçao
    ];
    let up = raw.trim().to_ascii_uppercase();
    OVERRIDES
        .iter()
        .find(|(from, _)| *from == up)
        .map(|(_, to)| (*to).to_string())
        .unwrap_or(up)
}

/// Spawn a one-shot background sync; the UI polls the returned receiver.
/// `ctx` is used to wake the UI when the result lands.
pub(crate) fn spawn_fetch(token: String, ctx: egui::Context) -> Receiver<LiveData> {
    let (tx, rx) = channel();
    thread::spawn(move || {
        let _ = tx.send(fetch_live(token));
        ctx.request_repaint();
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_code_normalizes() {
        assert_eq!(canonical_code("mex"), "MEX");
        assert_eq!(canonical_code("  Bra "), "BRA");
        assert_eq!(canonical_code("KVX"), "KOS"); // override
    }

    #[test]
    fn finished_stats_counts_only_finished_group_games() {
        let j = json!({"matches": [
            {"stage":"GROUP_STAGE","status":"FINISHED",
             "homeTeam":{"tla":"BRA"},"awayTeam":{"tla":"SCO"},
             "score":{"fullTime":{"home":2,"away":0}}},
            {"stage":"GROUP_STAGE","status":"IN_PLAY",
             "homeTeam":{"tla":"BRA"},"awayTeam":{"tla":"MAR"},
             "score":{"fullTime":{"home":1,"away":1}}},
        ]});
        let finished = finished_from_matches(&j);
        let s = finished_stats(&finished);
        assert_eq!(s.get("BRA"), Some(&(1u32, 3i64, 2i64, 0i64))); // in-play game ignored
        assert_eq!(s.get("SCO"), Some(&(1u32, 0i64, 0i64, 2i64)));
        assert_eq!(s.get("MAR"), None);
    }

    #[test]
    fn tally_discipline_scores_cards() {
        let j = json!({"matches": [
            {"stage":"GROUP_STAGE","bookings":[
                {"team":{"tla":"BRA"},"card":"YELLOW_CARD"},
                {"team":{"tla":"BRA"},"card":"RED_CARD"},
                {"team":{"tla":"SCO"},"card":"YELLOW_RED_CARD"},
            ]},
            {"stage":"LAST_16","bookings":[{"team":{"tla":"BRA"},"card":"RED_CARD"}]},
        ]});
        let d = tally_discipline(&j);
        assert_eq!(d.get("BRA"), Some(&5)); // 1 + 4 (knockout card ignored)
        assert_eq!(d.get("SCO"), Some(&3));
    }

    #[test]
    fn parse_espn_today_reads_live_score() {
        // Build a kickoff at local noon today so the local-date filter keeps it.
        use chrono::{Local, TimeZone, Utc};
        let noon = Local::now().date_naive().and_hms_opt(12, 0, 0).unwrap();
        let utc = Local.from_local_datetime(&noon).unwrap().with_timezone(&Utc);
        let date = utc.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let j = json!({"events":[{
            "date": date,
            "status":{"type":{"state":"in"}},
            "competitions":[{"competitors":[
                {"homeAway":"home","team":{"abbreviation":"BRA","displayName":"Brazil"},"score":"2"},
                {"homeAway":"away","team":{"abbreviation":"SCO","displayName":"Scotland"},"score":"1"},
            ]}],
        }]});
        let fx = parse_espn_today(&j);
        assert_eq!(fx.len(), 1);
        assert!(fx[0].status.is_live());
        assert_eq!(fx[0].score, Some((2, 1)));
        assert_eq!(fx[0].home_code, "BRA");
        assert_eq!(fx[0].away_code, "SCO");
    }
}
