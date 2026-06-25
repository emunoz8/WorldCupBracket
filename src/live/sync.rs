//! Live-mode orchestration: polling, applying a sync, and raising alerts.

use std::collections::HashMap;

use eframe::egui;
use fifa_team3::{KoMatch, Side, WinnerPrediction};

use crate::app::PredictorApp;

impl PredictorApp {
    /// Append a timestamped line to the API log (keeping the last ~120).
    fn log_line(&mut self, line: String) {
        // Troubleshooting hook: uncomment to mirror the API log to a `.txt` file
        // on disk (path from `settings::log_path()`), so it can be read/copied
        // outside the app when diagnosing live-feed issues (e.g. code mismatches).
        // use std::io::Write;
        // let path = crate::settings::log_path();
        // if let Some(dir) = path.parent() {
        //     let _ = std::fs::create_dir_all(dir);
        // }
        // if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        //     let _ = writeln!(f, "{line}");
        // }
        self.live.api_log.push(line);
        let len = self.live.api_log.len();
        if len > 120 {
            self.live.api_log.drain(0..len - 120);
        }
    }

    /// Fire goal toasts when a live fixture's score increased.
    fn detect_goals(&mut self) {
        let baseline = self.live.prev_scores.is_empty();
        let mut new_scores: HashMap<String, (i64, i64)> = HashMap::new();
        for f in &self.live.today_fixtures {
            let Some((h, a)) = f.score else { continue };
            let key = format!("{}-{}", f.home_code, f.away_code);
            if !baseline && let Some(&(oh, oa)) = self.live.prev_scores.get(&key) {
                if h > oh {
                    self.live.toasts.push(crate::live::Toast::new(
                        format!("GOAL · {} {h}-{a} {}", f.home, f.away),
                        crate::live::AlertKind::Up,
                    ));
                }
                if a > oa {
                    self.live.toasts.push(crate::live::Toast::new(
                        format!("GOAL · {} {h}-{a} {}", f.away, f.home),
                        crate::live::AlertKind::Up,
                    ));
                }
            }
            new_scores.insert(key, (h, a));
        }
        self.live.prev_scores = new_scores;
    }

    /// Kick off a background fetch of live standings from football-data.org.
    pub(crate) fn start_live_sync(&mut self, ctx: egui::Context, trigger: &str) {
        // No token is fine: football-data is skipped, but ESPN still gives live
        // scores/fixtures. A token additionally enables standings/results.
        let t = chrono::Local::now().format("%H:%M:%S");
        self.log_line(format!("{t}  sync started ({trigger})"));
        let token = self.live.api_key.trim().to_string();
        self.live.live_rx = Some(crate::live::spawn_fetch(token, ctx));
        self.live.live_status = Some("Syncing…".to_string());
    }

    /// Drain a finished background sync and apply it.
    pub(crate) fn poll_live(&mut self) {
        use std::sync::mpsc::TryRecvError;
        match self.live.live_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(data)) => {
                self.apply_live(data);
                self.live.live_rx = None;
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.live.live_status = Some("Sync failed: worker stopped".to_string());
                self.live.live_rx = None;
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
            remaining,
            log,
        } = data;
        for line in log {
            self.log_line(line);
        }
        self.live.today_fixtures = today;
        self.live.remaining = remaining;

        // Goal alerts: compare each live fixture's score to the previous poll.
        self.detect_goals();

        // Stickiness: football-data can briefly un-report a just-finished match
        // (a poll comes back with 38 games instead of 39), which would drop a
        // team's points and oscillate the order. Never let a team's played-count
        // go down vs the previous poll — keep the fuller stats. This also preserves
        // the last-good table through a full FD outage. Exempt a team currently in
        // a LIVE game: it *should* drop back to its pre-game baseline (its live game
        // is excluded from the standings on purpose, shown only in the projection).
        let mut standings = standings;
        let playing: std::collections::HashSet<&str> = self
            .live
            .today_fixtures
            .iter()
            .filter(|f| f.status.is_live())
            .flat_map(|f| [f.home_code.as_str(), f.away_code.as_str()])
            .collect();
        let prev: HashMap<&str, &crate::live::LiveTeam> = self
            .live
            .live_standings
            .iter()
            .flat_map(|s| s.teams.iter().map(|t| (t.code.as_str(), t)))
            .collect();
        for s in &mut standings {
            for t in &mut s.teams {
                if let Some(old) = prev.get(t.code.as_str())
                    && old.played > t.played
                    && !playing.contains(t.code.as_str())
                {
                    *t = (*old).clone();
                }
            }
        }

        // Sort by the full FIFA tiebreak chain (points → GD → GF → fair-play →
        // FIFA rank → code). The last two are *deterministic* — critically NOT the
        // user's group order, because we mirror this order back into the user's
        // groups below; tie-breaking on that order would create a feedback loop
        // and make tied teams oscillate between polls.
        crate::live::sort_standings(&mut standings);

        // In live mode, mirror the *projected* table order (finished results plus
        // any in-play scores) into the user's own groups, so the standings panel
        // and the bracket it feeds — including which team is each group's 3rd —
        // reflect the possible final standings, not just finished games. Only
        // groups that have kicked off are touched; un-started ones keep the user's
        // predicted order. (With live off, groups are never touched.)
        if self.live.live_mode {
            let projection = crate::live::project_standings(
                &standings,
                &self.live.today_fixtures,
                &self.live.remaining,
            );
            for s in &projection {
                if !s.teams.iter().any(|t| t.played > 0) {
                    continue;
                }
                if let Some(group) = self.groups.iter_mut().find(|g| g.group == s.group) {
                    let order: Vec<&str> = s.teams.iter().map(|t| t.code.as_str()).collect();
                    group.teams.sort_by_key(|t| {
                        order.iter().position(|c| *c == t.code).unwrap_or(usize::MAX)
                    });
                }
            }
        }

        let applied = standings.len();
        // Rank the 3rd-place race and its advance odds off the *projected* table
        // (finished results + any in-play scores) — the exact same basis the Live
        // Center's 3rd-place table is built on. If the odds were simulated from the
        // finished-only standings instead (with the in-play game still a random
        // "remaining" fixture), the Adv% would disagree with the row it sits next to
        // and flicker as the live score crosses a goal-difference boundary. The
        // in-play games are baked into the projection, so they're no longer
        // "remaining" for the simulation. Clinch flags stay on the *real* standings:
        // projecting bumps played-counts and would falsely lock an in-play game.
        let projected = crate::live::project_standings(
            &standings,
            &self.live.today_fixtures,
            &self.live.remaining,
        );
        let live_pairs: std::collections::HashSet<(&str, &str)> = self
            .live
            .today_fixtures
            .iter()
            .filter(|f| f.status.is_live())
            .map(|f| (f.home_code.as_str(), f.away_code.as_str()))
            .collect();
        let remaining_to_play: Vec<crate::live::GroupFixture> = self
            .live
            .remaining
            .iter()
            .filter(|r| !live_pairs.contains(&(r.home.as_str(), r.away.as_str())))
            .cloned()
            .collect();
        self.live.third_rank = crate::live::third_place_ranking(&projected);
        self.live.clinched = crate::live::clinched_positions(&standings);
        self.live.third_outlook =
            crate::live::third_place_outlook(&projected, &remaining_to_play);
        self.live.third_slot_pct =
            crate::live::third_slot_pct(&projected, &remaining_to_play, &self.annex);
        self.live.third_routing =
            crate::live::third_routing(&projected, &remaining_to_play, &self.annex);

        // Live mode: mirror the projected top-8 third-place race into each group's
        // bracket status, so the R32 third-place slots re-slot (via the annex
        // lookup in `fifa_team3`) whenever the set of advancing thirds changes.
        // Only groups that have kicked off are driven; un-started groups keep the
        // user's manual pick. (With live off, statuses are never touched.)
        if self.live.live_mode {
            let started: std::collections::HashSet<char> = projected
                .iter()
                .filter(|s| s.teams.iter().any(|t| t.played > 0))
                .map(|s| s.group)
                .collect();
            let updates: Vec<(char, bool)> = self
                .live
                .third_rank
                .iter()
                .filter(|r| started.contains(&r.group))
                .map(|r| (r.group, r.advances))
                .collect();
            for (group, advances) in updates {
                if let Some(g) = self.groups.iter_mut().find(|g| g.group == group) {
                    g.third_place_status = if advances {
                        fifa_team3::ThirdPlaceStatus::Advanced
                    } else {
                        fifa_team3::ThirdPlaceStatus::Eliminated
                    };
                }
            }
        }

        // Drop cached scenarios; each group is recomputed lazily when expanded.
        self.live.scenario_cache.clear();
        self.live.live_standings = standings;

        // Raise alerts for any movement vs the previous poll.
        self.diff_and_alert();

        // Apply finished knockout results to bracket picks.
        let wins = self.apply_live_results(&results);
        self.live.live_status = Some(format!(
            "Synced {applied} groups · {wins} knockout results from live data"
        ));
    }

    /// Alert only on real events: a team gaining points (a result), or a team
    /// crossing the top-8 cutoff in the 3rd-place race. With sticky stats these
    /// only ever move one way, so each alert fires once — no oscillation.
    fn diff_and_alert(&mut self) {
        let baseline = self.live.prev_group_points.is_empty();
        let mut events: Vec<(String, crate::live::AlertKind)> = Vec::new();

        // Teams in a LIVE game: their points/3rd-place position flicker as the feed
        // folds the in-play result in and out between polls. That isn't a real
        // event, so never alert on them — they alert once when the match goes Final.
        let live_now: std::collections::HashSet<&str> = self
            .live
            .today_fixtures
            .iter()
            .filter(|f| f.status.is_live())
            .flat_map(|f| [f.home_code.as_str(), f.away_code.as_str()])
            .collect();

        if !baseline {
            // A team's points went up → it just won or drew a match.
            for s in &self.live.live_standings {
                for t in &s.teams {
                    if live_now.contains(t.code.as_str()) {
                        continue;
                    }
                    if let Some(&old) = self.live.prev_group_points.get(&t.code)
                        && t.points > old
                    {
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
            for (i, r) in self.live.third_rank.iter().enumerate() {
                if live_now.contains(r.code.as_str()) {
                    continue;
                }
                if let Some(&old) = self.live.prev_third.get(&r.code) {
                    let was_in = old < 8;
                    let now_in = i < 8;
                    if was_in != now_in {
                        let up = now_in;
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

        for (text, kind) in events {
            self.live.toasts.push(crate::live::Toast::new(text, kind));
        }

        // Movement snapshot for the 3rd-place table arrows (vs the previous poll).
        self.live.third_delta = self
            .live
            .third_rank
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                self.live
                    .prev_third
                    .get(&r.code)
                    .map(|&old| (r.code.clone(), (i as i64 - old as i64).signum() as i8))
            })
            .collect();

        // Record this poll as the new baseline.
        self.live.prev_group_points = self
            .live
            .live_standings
            .iter()
            .flat_map(|s| s.teams.iter().map(|t| (t.code.clone(), t.points)))
            .collect();
        self.live.prev_third = self
            .live
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
}

/// Format a 1-based position as an ordinal (1st, 2nd, 3rd, 4th…).
pub(crate) fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}
