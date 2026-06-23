//! "What needs to happen" — full goal-margin qualification scenarios per group.
//!
//! Given a group's current (finished-match) standings and the matches still to
//! play, this enumerates every bounded scoreline of the remaining games, runs
//! each through the real FIFA tiebreak chain, and distils the result into
//! human-readable conditions per team — including the goal-difference margins a
//! team needs ("win by ≥2", "avoid losing by 3+").

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::*;

/// Goals modeled for one team in one remaining match: 0..=MAXG per side. 8 is a
/// comfortable bound on a realistic World Cup scoreline and keeps the search small.
const MAXG: i64 = 8;
/// Beyond this many remaining group matches the search explodes and the answers
/// aren't actionable yet — report "too early" instead. Two covers the final
/// round (both group games still to play) while keeping the search ≤ 81² per group.
const MAX_REMAINING: usize = 2;

/// The scenario outlook for one whole group.
pub(crate) enum GroupScenarios {
    /// Group fully played — positions are final, nothing to compute.
    Done,
    /// Too many matches left to enumerate meaningfully (carries the count).
    TooEarly(usize),
    /// One outlook per team, in current table order.
    Ready(Vec<TeamScenario>),
}

/// What a single team's last match can still produce.
pub(crate) struct TeamScenario {
    pub(crate) code: String,
    pub(crate) name: String,
    /// Distinct finishing positions still reachable (1..=4), ascending.
    pub(crate) possible: Vec<u32>,
    /// Per own-result outlook (only results that are actually still possible).
    pub(crate) branches: Vec<Branch>,
}

/// The team's own remaining-match result for this branch.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwnResult {
    Win,
    Draw,
    Loss,
    /// The team has no single remaining match (≠1 games left) — outlook is global.
    Any,
}

impl OwnResult {
    pub(crate) fn label(self) -> &'static str {
        match self {
            OwnResult::Win => "Win",
            OwnResult::Draw => "Draw",
            OwnResult::Loss => "Lose",
            OwnResult::Any => "Remaining games",
        }
    }
}

/// One branch of a team's outlook, keyed on the team's own result (Win/Draw/Lose).
/// `best`/`worst` bound the finish across the branch; `conditions` spell out each
/// concrete other-match outcome that drives it.
pub(crate) struct Branch {
    pub(crate) own: OwnResult,
    pub(crate) best: u32,
    pub(crate) worst: u32,
    pub(crate) conditions: Vec<Condition>,
}

/// One concrete sub-scenario inside a branch: a specific outcome of the group's
/// *other* match, the resulting finish, and any goal-margin requirement.
pub(crate) struct Condition {
    /// The other match's result, e.g. "CRO beat JPN" (empty if no other match).
    pub(crate) other: String,
    pub(crate) best: u32,
    pub(crate) worst: u32,
    /// Margin rule when goal difference still decides, e.g. "win by ≥2 for 1st".
    pub(crate) detail: String,
    /// Structured goal-difference swing for a paintable bar (only when a margin
    /// ≥2 in the *other* match actually flips the finish).
    pub(crate) gd: Option<GdViz>,
}

/// One cell of the goal-difference grid: does the analysed team hold its place,
/// lose it, or land on the goal-difference tie line (where goals-for decides)?
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GdCell {
    Hold,
    Lose,
    Tie,
}

/// A paintable 2-D goal-difference grid: this team's own winning/losing margin on
/// one axis, the threat team's winning margin on the other. The hold→lose boundary
/// is the diagonal "goal-difference of goal-differences" line.
pub(crate) struct GdViz {
    /// X-axis caption, e.g. "USA beats Canada by" / "USA loses to Canada by".
    pub(crate) own_axis: String,
    /// Y-axis caption, e.g. "Germany beats Ecuador by".
    pub(crate) threat_axis: String,
    /// X-axis values (this team's own margin magnitudes) and Y-axis (threat margins).
    pub(crate) own_margins: Vec<i64>,
    pub(crate) threat_margins: Vec<i64>,
    /// Row-major (rows = threat margins ascending, cols = own margins ascending).
    pub(crate) grid: Vec<GdCell>,
    pub(crate) hold: u32,
    pub(crate) lose: u32,
}

/// Three points for a result, from a team's perspective (its goals vs conceded).
fn pts3(scored: i64, conceded: i64) -> i64 {
    match scored.cmp(&conceded) {
        std::cmp::Ordering::Greater => 3,
        std::cmp::Ordering::Equal => 1,
        std::cmp::Ordering::Less => 0,
    }
}

/// The remaining fixtures whose both teams belong to this group.
pub(crate) fn group_remaining(teams: &[LiveTeam], all: &[GroupFixture]) -> Vec<GroupFixture> {
    let codes: BTreeSet<&str> = teams.iter().map(|t| t.code.as_str()).collect();
    all.iter()
        .filter(|f| codes.contains(f.home.as_str()) && codes.contains(f.away.as_str()))
        .cloned()
        .collect()
}

/// A team's record after a set of results: (points, goal_diff, goals_for,
/// disciplinary). Used both for ordering and for cross-group 3rd-place compares.
type Record = (i64, i64, i64, i64);

/// Order two records by the stat part of the FIFA tiebreak chain — points → goal
/// difference → goals for → fewest disciplinary points. Better record sorts first
/// (`Less`). Callers append FIFA-rank / code tiebreaks where a total order is needed.
fn cmp_record(a: Record, b: Record) -> std::cmp::Ordering {
    b.0.cmp(&a.0)
        .then(b.1.cmp(&a.1))
        .then(b.2.cmp(&a.2))
        .then(a.3.cmp(&b.3))
}

/// Final standings (top→bottom) after applying `scores` to `rem`, using the full
/// FIFA tiebreak chain (points → GD → GF → fair-play → FIFA rank). Each entry is
/// `(code, record)`.
fn final_table(base: &[LiveTeam], rem: &[GroupFixture], scores: &[(i64, i64)]) -> Vec<(String, Record)> {
    let mut acc: HashMap<&str, Record> = base
        .iter()
        .map(|t| {
            (
                t.code.as_str(),
                (t.points, t.goal_diff, t.goals_for, t.disciplinary),
            )
        })
        .collect();
    for (f, &(h, a)) in rem.iter().zip(scores) {
        if let Some(e) = acc.get_mut(f.home.as_str()) {
            e.0 += pts3(h, a);
            e.1 += h - a;
            e.2 += h;
        }
        if let Some(e) = acc.get_mut(f.away.as_str()) {
            e.0 += pts3(a, h);
            e.1 += a - h;
            e.2 += a;
        }
    }
    let mut codes: Vec<&str> = acc.keys().copied().collect();
    codes.sort_by(|x, y| {
        cmp_record(acc[x], acc[y])
            .then_with(|| fifa_rank(x).cmp(&fifa_rank(y)))
            .then_with(|| x.cmp(y))
    });
    codes.into_iter().map(|c| (c.to_string(), acc[c])).collect()
}

/// Final 1-based position of every team after applying `scores` to `rem`.
fn final_positions(
    base: &[LiveTeam],
    rem: &[GroupFixture],
    scores: &[(i64, i64)],
) -> HashMap<String, u32> {
    final_table(base, rem, scores)
        .into_iter()
        .enumerate()
        .map(|(i, (c, _))| (c, (i + 1) as u32))
        .collect()
}

/// Run `f` for every bounded scoreline assignment over `rem` (0..=MAXG each side).
fn for_each_assignment(rem: usize, mut f: impl FnMut(&[(i64, i64)])) {
    let mut scores = vec![(0i64, 0i64); rem];
    // Odometer over rem matches × (MAXG+1)² scorelines.
    let per = (MAXG + 1) * (MAXG + 1);
    let total = per.pow(rem as u32);
    for n in 0..total {
        let mut k = n;
        for s in scores.iter_mut() {
            let cell = k % per;
            s.0 = cell / (MAXG + 1);
            s.1 = cell % (MAXG + 1);
            k /= per;
        }
        f(&scores);
    }
}

/// Build the qualification outlook for one group.
pub(crate) fn group_scenarios(teams: &[LiveTeam], all_remaining: &[GroupFixture]) -> GroupScenarios {
    let rem = group_remaining(teams, all_remaining);
    if rem.is_empty() {
        return GroupScenarios::Done;
    }
    if rem.len() > MAX_REMAINING {
        return GroupScenarios::TooEarly(rem.len());
    }

    let scenarios = teams
        .iter()
        .map(|t| team_scenario(t, teams, &rem))
        .collect();
    GroupScenarios::Ready(scenarios)
}

/// Stat-only "a strictly outranks b": points → GD → GF → fewer cards. Deliberately
/// omits the FIFA-rank tiebreak, so it only returns true when the ordering is
/// decisive on real results — used to *guarantee* elimination, never to falsely
/// eliminate a team a tiebreak might still save.
fn strictly_better(a: Record, b: Record) -> bool {
    cmp_record(a, b) == std::cmp::Ordering::Less
}

/// One group's eventual 3rd-place slot, bounded by the strongest (`best`) and
/// weakest (`worst`) records it can still produce. For a finished group both
/// equal the fixed 3rd team's record; `best == None` means "unbounded" (too many
/// games left to bound), so that group could always outrank someone.
struct GroupThird {
    group: char,
    /// The current 3rd-place team's code (fixed once the group is finished).
    code: Option<String>,
    finished: bool,
    worst: Option<Record>,
    best: Option<Record>,
}

/// Bound every group's eventual 3rd-place record (strongest/weakest possible).
fn group_thirds(standings: &[LiveStanding], remaining: &[GroupFixture]) -> Vec<GroupThird> {
    standings
        .iter()
        .map(|s| {
            let rem = group_remaining(&s.teams, remaining);
            let current = s.teams.get(2).map(|t| t.code.clone());
            if rem.is_empty() {
                let third = final_table(&s.teams, &[], &[]).into_iter().nth(2);
                let rec = third.as_ref().map(|(_, r)| *r);
                GroupThird {
                    group: s.group,
                    code: third.map(|(c, _)| c).or(current),
                    finished: true,
                    worst: rec,
                    best: rec,
                }
            } else if rem.len() <= MAX_REMAINING {
                let (mut worst, mut best): (Option<Record>, Option<Record>) = (None, None);
                for_each_assignment(rem.len(), |scores| {
                    if let Some((_, r)) = final_table(&s.teams, &rem, scores).into_iter().nth(2) {
                        worst = Some(match worst {
                            Some(w) if strictly_better(w, r) => r,
                            Some(w) => w,
                            None => r,
                        });
                        best = Some(match best {
                            Some(b) if strictly_better(r, b) => r,
                            Some(b) => b,
                            None => r,
                        });
                    }
                });
                GroupThird {
                    group: s.group,
                    code: current,
                    finished: false,
                    worst,
                    best,
                }
            } else {
                // Too many games left to bound: weakest contributes nothing,
                // strongest is unbounded (could outrank anyone).
                GroupThird {
                    group: s.group,
                    code: current,
                    finished: false,
                    worst: None,
                    best: None,
                }
            }
        })
        .collect()
}

/// One 3rd-place team's outlook for the best-thirds race.
#[derive(Clone, Copy, Default)]
pub(crate) struct ThirdOutlook {
    /// Guaranteed top-8 (cannot be caught) — advances for sure.
    pub(crate) clinched: bool,
    /// Guaranteed bottom-4 — out for sure.
    pub(crate) eliminated: bool,
    /// Simulated probability of reaching the R32 (any route), 0.0..=1.0.
    pub(crate) pct: f32,
}

/// Full outlook per group's current 3rd-place team: exact clinch/elimination
/// flags plus a Monte-Carlo probability of advancing. Cross-group exact counting
/// would explode, so the probability is simulated; the flags stay exact.
pub(crate) fn third_place_outlook(
    standings: &[LiveStanding],
    remaining: &[GroupFixture],
) -> HashMap<String, ThirdOutlook> {
    let groups = group_thirds(standings, remaining);
    let mut out: HashMap<String, ThirdOutlook> = HashMap::new();

    for t in &groups {
        let Some(code) = &t.code else { continue };
        let mut o = ThirdOutlook::default();
        if t.finished && let Some(trec) = t.worst {
            // Eliminated: ≥8 others guaranteed above.
            let above = groups
                .iter()
                .filter(|x| x.group != t.group)
                .filter(|x| x.worst.is_some_and(|w| strictly_better(w, trec)))
                .count();
            o.eliminated = above >= 8;
            // Clinched: ≥4 others guaranteed below ⇒ at most 7 can be above.
            let below = groups
                .iter()
                .filter(|x| x.group != t.group)
                .filter(|x| x.best.is_some_and(|b| strictly_better(trec, b)))
                .count();
            o.clinched = below >= 4;
        }
        out.insert(code.clone(), o);
    }

    for (code, p) in third_place_pct(standings, remaining) {
        out.entry(code).or_default().pct = p;
    }
    out
}

/// Monte-Carlo probability that each team reaches the R32 (top 2, or a top-8
/// third) over randomly simulated remaining results. Every team is tracked — not
/// just the current 3rd — so whichever team the table shows as 3rd (after the
/// in-play projection reorders a group) always has a real probability.
fn third_place_pct(
    standings: &[LiveStanding],
    remaining: &[GroupFixture],
) -> HashMap<String, f32> {
    const SIMS: u32 = 4000;
    let watched: Vec<String> = standings
        .iter()
        .flat_map(|s| s.teams.iter().map(|t| t.code.clone()))
        .collect();
    let mut hits: HashMap<String, u32> = watched.iter().map(|c| (c.clone(), 0)).collect();

    // Per-group remaining fixtures, pre-sliced so each sim just rolls scores.
    let rems: Vec<(&LiveStanding, Vec<GroupFixture>)> = standings
        .iter()
        .map(|s| (s, group_remaining(&s.teams, remaining)))
        .collect();

    let mut rng = 0x9E37_79B9_7F4A_7C15u64;
    for _ in 0..SIMS {
        let mut advanced: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut thirds: Vec<Record> = Vec::with_capacity(standings.len());
        let mut third_codes: Vec<String> = Vec::with_capacity(standings.len());
        for (s, rem) in &rems {
            let scores: Vec<(i64, i64)> = rem
                .iter()
                .map(|_| (sample_goals(&mut rng), sample_goals(&mut rng)))
                .collect();
            let table = final_table(&s.teams, rem, &scores);
            if let Some((c, _)) = table.first() {
                advanced.insert(c.clone());
            }
            if let Some((c, _)) = table.get(1) {
                advanced.insert(c.clone());
            }
            if let Some((c, r)) = table.get(2) {
                thirds.push(*r);
                third_codes.push(c.clone());
            }
        }
        // Rank thirds, top 8 advance.
        let mut idx: Vec<usize> = (0..thirds.len()).collect();
        idx.sort_by(|&a, &b| cmp_record(thirds[a], thirds[b]));
        for &i in idx.iter().take(8) {
            advanced.insert(third_codes[i].clone());
        }
        for c in &watched {
            if advanced.contains(c) {
                *hits.get_mut(c).unwrap() += 1;
            }
        }
    }

    hits.into_iter()
        .map(|(c, h)| (c, h as f32 / SIMS as f32))
        .collect()
}

/// Monte-Carlo probability that a specific team is a given group winner's R32
/// third-place opponent — i.e. P(team T fills winner-slot S's "3rd" slot). Each
/// sim rolls the remaining games, ranks the thirds, takes the top 8, looks the
/// resulting qualifying combination up in the Annex (which maps every winner slot
/// to a group's third for that combination), and tallies which team filled each
/// slot. Returns winner_slot ("1A") → team_code → probability (0.0..=1.0).
///
/// This is the joint probability the bracket needs (the team must finish 3rd, be
/// top-8, *and* the annex must route its group to this slot) — distinct from the
/// plain advance odds in [`third_place_outlook`].
pub(crate) fn third_slot_pct(
    standings: &[LiveStanding],
    remaining: &[GroupFixture],
    annex: &fifa_team3::Annex,
) -> HashMap<String, HashMap<String, f32>> {
    const SIMS: u32 = 4000;
    let rems: Vec<(&LiveStanding, Vec<GroupFixture>)> = standings
        .iter()
        .map(|s| (s, group_remaining(&s.teams, remaining)))
        .collect();

    let mut counts: HashMap<String, HashMap<String, u32>> = HashMap::new();
    let mut rng = 0x9E37_79B9_7F4A_7C15u64;
    for _ in 0..SIMS {
        // Each group's simulated 3rd-place team, plus the thirds to rank.
        let mut group_third: HashMap<char, String> = HashMap::new();
        let mut thirds: Vec<(char, Record)> = Vec::with_capacity(standings.len());
        for (s, rem) in &rems {
            let scores: Vec<(i64, i64)> = rem
                .iter()
                .map(|_| (sample_goals(&mut rng), sample_goals(&mut rng)))
                .collect();
            let table = final_table(&s.teams, rem, &scores);
            if let Some((c, r)) = table.get(2) {
                group_third.insert(s.group, c.clone());
                thirds.push((s.group, *r));
            }
        }
        // Top 8 thirds advance; the sorted set of their groups keys the Annex.
        thirds.sort_by(|a, b| cmp_record(a.1, b.1));
        let mut advancing: Vec<char> = thirds.iter().take(8).map(|(g, _)| *g).collect();
        advancing.sort_unstable();
        let key: String = advancing.into_iter().collect();
        if let Some(alloc) = annex.get(&key) {
            for (winner_slot, opp_key) in alloc {
                if let Some(g) = opp_key.chars().nth(1)
                    && let Some(team) = group_third.get(&g)
                {
                    *counts
                        .entry(winner_slot.clone())
                        .or_default()
                        .entry(team.clone())
                        .or_insert(0) += 1;
                }
            }
        }
    }

    counts
        .into_iter()
        .map(|(slot, teams)| {
            let probs = teams
                .into_iter()
                .map(|(c, n)| (c, n as f32 / SIMS as f32))
                .collect();
            (slot, probs)
        })
        .collect()
}

/// xorshift64 step — a tiny deterministic PRNG (no external crate needed).
fn next_rand(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Sample a single team's goals from a rough World-Cup scoring distribution.
fn sample_goals(state: &mut u64) -> i64 {
    let r = (next_rand(state) >> 11) as f64 / (1u64 << 53) as f64; // [0,1)
    match r {
        x if x < 0.30 => 0,
        x if x < 0.64 => 1,
        x if x < 0.85 => 2,
        x if x < 0.95 => 3,
        x if x < 0.99 => 4,
        _ => 5,
    }
}

/// Position finishing stats for one (own-result, other-results) coarse cell.
struct KeyStat {
    best: u32,
    worst: u32,
    /// (this team's net margin, the other match's net margin) → (best, worst)
    /// finish. The other margin is the lever that turns goal-difference ties.
    by_pair: BTreeMap<(i64, i64), (u32, u32)>,
}

fn ord(n: u32) -> String {
    super::sync::ordinal(n)
}

fn team_scenario(team: &LiveTeam, teams: &[LiveTeam], rem: &[GroupFixture]) -> TeamScenario {
    let code = team.code.as_str();
    let own_idx: Vec<usize> = rem
        .iter()
        .enumerate()
        .filter(|(_, f)| f.home == code || f.away == code)
        .map(|(i, _)| i)
        .collect();
    let single = (own_idx.len() == 1).then(|| own_idx[0]);
    // Indices of the group's *other* matches (the ones this team isn't in).
    let others: Vec<usize> = match single {
        Some(j) => (0..rem.len()).filter(|i| *i != j).collect(),
        None => Vec::new(),
    };

    let mut possible: BTreeSet<u32> = BTreeSet::new();
    let mut g_best = teams.len() as u32;
    let mut g_worst = 1u32;
    // (own result sign, other-match signs) → finishing stats.
    let mut keys: HashMap<(i8, Vec<i8>), KeyStat> = HashMap::new();

    for_each_assignment(rem.len(), |scores| {
        let pos = final_positions(teams, rem, scores)
            .get(code)
            .copied()
            .unwrap_or(teams.len() as u32);
        possible.insert(pos);
        g_best = g_best.min(pos);
        g_worst = g_worst.max(pos);
        if let Some(j) = single {
            let (s, c) = scores[j];
            let margin = if rem[j].home == code { s - c } else { c - s };
            let own_sign = margin.signum() as i8;
            let others_sig: Vec<i8> = others
                .iter()
                .map(|&i| (scores[i].0 - scores[i].1).signum() as i8)
                .collect();
            let other_margin = others
                .first()
                .map(|&i| scores[i].0 - scores[i].1)
                .unwrap_or(0);
            let ks = keys.entry((own_sign, others_sig)).or_insert(KeyStat {
                best: pos,
                worst: pos,
                by_pair: BTreeMap::new(),
            });
            ks.best = ks.best.min(pos);
            ks.worst = ks.worst.max(pos);
            let e = ks.by_pair.entry((margin, other_margin)).or_insert((pos, pos));
            e.0 = e.0.min(pos);
            e.1 = e.1.max(pos);
        }
    });

    let name_of = |c: &str| {
        teams
            .iter()
            .find(|t| t.code == c)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| c.to_string())
    };

    let branches = if single.is_some() {
        build_branches(&keys, &others, rem, &name_of, code)
    } else {
        vec![Branch {
            own: OwnResult::Any,
            best: g_best,
            worst: g_worst,
            conditions: Vec::new(),
        }]
    };

    TeamScenario {
        code: team.code.clone(),
        name: team.name.clone(),
        possible: possible.into_iter().collect(),
        branches,
    }
}

/// Group the (own, others) cells into Win / Draw / Lose branches, each carrying a
/// concrete condition per distinct other-match outcome.
fn build_branches(
    keys: &HashMap<(i8, Vec<i8>), KeyStat>,
    others: &[usize],
    rem: &[GroupFixture],
    name_of: &dyn Fn(&str) -> String,
    code: &str,
) -> Vec<Branch> {
    let mut out = Vec::new();
    for (own, sign) in [
        (OwnResult::Win, 1i8),
        (OwnResult::Draw, 0),
        (OwnResult::Loss, -1),
    ] {
        let mut cells: Vec<(&Vec<i8>, &KeyStat)> = keys
            .iter()
            .filter(|((s, _), _)| *s == sign)
            .map(|((_, sig), ks)| (sig, ks))
            .collect();
        if cells.is_empty() {
            continue;
        }
        cells.sort_by(|a, b| b.0.cmp(a.0)); // group "other wins" first, consistent
        let best = cells.iter().map(|(_, k)| k.best).min().unwrap();
        let worst = cells.iter().map(|(_, k)| k.worst).max().unwrap();
        let conditions = cells
            .iter()
            .map(|(sig, ks)| {
                let (detail, gd) = margin_detail(sign, sig, others, rem, name_of, ks, code);
                Condition {
                    other: describe_others(sig, others, rem, name_of),
                    best: ks.best,
                    worst: ks.worst,
                    detail,
                    gd,
                }
            })
            .collect();
        out.push(Branch {
            own,
            best,
            worst,
            conditions,
        });
    }
    out
}

/// Human description of the other match(es) for one outcome signature.
fn describe_others(
    sig: &[i8],
    others: &[usize],
    rem: &[GroupFixture],
    name_of: &dyn Fn(&str) -> String,
) -> String {
    if others.is_empty() {
        return String::new();
    }
    others
        .iter()
        .zip(sig)
        .map(|(&i, &s)| {
            let h = name_of(&rem[i].home);
            let a = name_of(&rem[i].away);
            match s {
                1 => format!("{h} beat {a}"),
                -1 => format!("{a} beat {h}"),
                _ => format!("{h} & {a} draw"),
            }
        })
        .collect::<Vec<_>>()
        .join(" and ")
}

/// When goal difference still decides inside a cell, spell out the swing. If the
/// other match has a winner, the lever is *that team's* margin at this team's
/// closest margin (e.g. "lose by 1 → 1st unless South Korea win by ≥3"). When the
/// other match is a draw (or there is none), only this team's own margin matters
/// ("win by ≥2 for 1st"). Empty when the finish is already fixed.
fn margin_detail(
    sign: i8,
    sig: &[i8],
    others: &[usize],
    rem: &[GroupFixture],
    name_of: &dyn Fn(&str) -> String,
    ks: &KeyStat,
    code: &str,
) -> (String, Option<GdViz>) {
    if ks.best == ks.worst {
        return (String::new(), None);
    }
    // This team's own opponent (the team it isn't sharing with `others`).
    let own_opp = (0..rem.len())
        .find(|i| !others.contains(i))
        .map(|i| {
            if rem[i].home == code {
                name_of(&rem[i].away)
            } else {
                name_of(&rem[i].home)
            }
        })
        .unwrap_or_default();

    // With no contested other match (none, or it's a draw), only own margin moves it.
    if others.is_empty() || sig.first().copied() == Some(0) {
        return (own_margin_rule(sign, ks, &own_opp), None);
    }

    // The other match has a winner — express the swing as their margin while this
    // team is at its closest decisive margin (win by 1 / lose by 1).
    let oi = others[0];
    let os = sig[0];
    let threat = if os == 1 {
        name_of(&rem[oi].home)
    } else {
        name_of(&rem[oi].away)
    };
    let own_b = sign as i64; // win → +1, lose → −1
    let mut rows: Vec<(i64, u32)> = ks
        .by_pair
        .iter()
        .filter(|((mt, _), _)| *mt == own_b)
        .map(|((_, mo), (_, w))| {
            let threat_margin = if os == 1 { *mo } else { -*mo };
            (threat_margin, *w)
        })
        .collect();
    rows.sort_by_key(|(tm, _)| *tm);
    let z = rows.iter().find(|(_, w)| *w > ks.best).map(|(tm, _)| *tm);

    let own_phrase = match sign {
        1 => format!("beat {own_opp} by 1"),
        -1 => format!("lose to {own_opp} by 1"),
        _ => format!("draw {own_opp}"),
    };
    let own_name = name_of(code);
    let threat_opp = if os == 1 {
        name_of(&rem[oi].away)
    } else {
        name_of(&rem[oi].home)
    };
    let gd = build_gd_grid(sign, os, ks, &threat, &threat_opp, &own_name, &own_opp);
    // A clean threat-margin threshold only reads well when it's ≥2 (a win is
    // already ≥1, and here the cell already states the threat team won — so
    // "unless they win" would just repeat the condition). Otherwise the margins
    // interact and the grid tells the story.
    let text = if let Some(z) = z.filter(|&z| z >= 2) {
        format!(
            "{own_phrase} → {} unless {threat} win by ≥{z} (then {})",
            ord(ks.best),
            ord(ks.worst)
        )
    } else if gd.is_some() {
        format!(
            "{own_phrase} → {} or {} — goal margins decide (see grid)",
            ord(ks.best),
            ord(ks.worst)
        )
    } else {
        own_margin_rule(sign, ks, &own_opp)
    };
    (text, gd)
}

/// Build the 2-D goal-difference grid: this team's own margin (cols) vs the
/// threat team's winning margin (rows). Each cell is Hold / Lose / Tie. `None`
/// when the grid would be trivial (one axis only).
#[allow(clippy::too_many_arguments)]
fn build_gd_grid(
    sign: i8,
    os: i8,
    ks: &KeyStat,
    threat: &str,
    threat_opp: &str,
    own_name: &str,
    own_opp: &str,
) -> Option<GdViz> {
    const WIN: i64 = 5; // window width per axis
    // Full cell map over every enumerated margin (0..=MAXG each side).
    let mut cells: HashMap<(i64, i64), GdCell> = HashMap::new();
    for (&(om, otm), &(b, w)) in &ks.by_pair {
        match sign {
            1 if om <= 0 => continue,
            -1 if om >= 0 => continue,
            0 if om != 0 => continue,
            _ => {}
        }
        let tm = if os == 1 { otm } else { -otm }; // threat team's winning margin
        if tm < 1 {
            continue;
        }
        let omag = om.abs();
        if omag < 1 {
            continue;
        }
        let cell = if b != w {
            GdCell::Tie
        } else if b == ks.best {
            GdCell::Hold
        } else {
            GdCell::Lose
        };
        cells.insert((omag, tm), cell);
    }
    // Need a real boundary somewhere, else the grid teaches nothing.
    if !cells.values().any(|c| *c == GdCell::Hold)
        || !cells.values().any(|c| *c == GdCell::Lose)
    {
        return None;
    }

    // Centre the window on the hold↔lose boundary so the diagonal is in view. A
    // column/row is "on the boundary" when it contains *both* a hold and a lose —
    // that's where goals actually swing the place. Centre on the median of those.
    let owns: BTreeSet<i64> = cells.keys().map(|(o, _)| *o).collect();
    let threats: BTreeSet<i64> = cells.keys().map(|(_, t)| *t).collect();
    let mixed = |fixed_is_own: bool, k: i64| -> bool {
        let (mut h, mut l) = (false, false);
        let line = if fixed_is_own { &threats } else { &owns };
        for &j in line {
            let cell = if fixed_is_own {
                cells.get(&(k, j))
            } else {
                cells.get(&(j, k))
            };
            match cell {
                Some(GdCell::Hold) => h = true,
                Some(GdCell::Lose) => l = true,
                _ => {}
            }
        }
        h && l
    };
    let trans_o: Vec<i64> = owns.iter().copied().filter(|&o| mixed(true, o)).collect();
    let trans_t: Vec<i64> = threats.iter().copied().filter(|&t| mixed(false, t)).collect();
    let median = |mut v: Vec<i64>, fallback: i64| -> i64 {
        if v.is_empty() {
            return fallback;
        }
        v.sort_unstable();
        v[v.len() / 2]
    };
    let hi = *threats.iter().max().unwrap_or(&WIN);
    let oc = median(trans_o, *owns.iter().next().unwrap_or(&1));
    let tc = median(trans_t, WIN);
    // A WIN-wide window centred on the boundary, clamped to ≥1 and the data range.
    let window = |centre: i64, top: i64| -> Vec<i64> {
        let lo = (centre - WIN / 2).clamp(1, (top - WIN + 1).max(1));
        (lo..lo + WIN).filter(|m| *m <= top.max(lo)).collect()
    };
    let own_margins = window(oc, hi.max(WIN));
    let threat_margins = window(tc, hi);
    if own_margins.is_empty() || threat_margins.is_empty() {
        return None;
    }

    // Row-major: rows = threat margins ascending, cols = own margins ascending.
    // Cells past the enumerated range follow the monotone trend (more threat goals
    // can only hurt, more of your own goals can only help).
    let cellref = &cells;
    let cols = &own_margins;
    let grid: Vec<GdCell> = threat_margins
        .iter()
        .flat_map(|&tm| {
            cols.iter().map(move |&om| {
                cellref
                    .get(&(om, tm))
                    .copied()
                    .unwrap_or(if tm > om { GdCell::Lose } else { GdCell::Hold })
            })
        })
        .collect();

    let own_axis = match sign {
        1 => format!("{own_name} beats {own_opp} by"),
        -1 => format!("{own_name} loses to {own_opp} by"),
        _ => format!("{own_name} vs {own_opp}"),
    };
    Some(GdViz {
        own_axis,
        threat_axis: format!("{threat} beats {threat_opp} by"),
        own_margins,
        threat_margins,
        grid,
        hold: ks.best,
        lose: ks.worst,
    })
}

/// Margin rule driven solely by this team's own goal margin (vs `own_opp`).
fn own_margin_rule(sign: i8, ks: &KeyStat, own_opp: &str) -> String {
    // Worst finish per own margin (over every other-match result).
    let mut worst_by_margin: BTreeMap<i64, u32> = BTreeMap::new();
    for ((mt, _), (_, w)) in &ks.by_pair {
        let e = worst_by_margin.entry(*mt).or_insert(0);
        *e = (*e).max(*w);
    }
    match sign {
        1 => {
            let mut thr = None;
            let margins: Vec<i64> = worst_by_margin.keys().copied().filter(|m| *m > 0).collect();
            for &m in margins.iter().rev() {
                if worst_by_margin.range(m..).all(|(_, &w)| w == ks.best) {
                    thr = Some(m);
                } else {
                    break;
                }
            }
            match thr {
                Some(m) => format!(
                    "beat {own_opp} by ≥{m} for {}, else {}",
                    ord(ks.best),
                    ord(ks.worst)
                ),
                None => "goal difference decides".to_string(),
            }
        }
        -1 => {
            let mut limit = None;
            for (&m, &w) in worst_by_margin.iter().rev() {
                if m < 0 {
                    if w == ks.best {
                        limit = Some(-m);
                    } else {
                        break;
                    }
                }
            }
            match limit {
                Some(k) => format!(
                    "lose to {own_opp} by ≤{k} for {}, else {}",
                    ord(ks.best),
                    ord(ks.worst)
                ),
                None => "goal difference decides".to_string(),
            }
        }
        _ => "goal difference decides".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team_p(code: &str, pos: u32, pts: i64, gd: i64, gf: i64, played: u32) -> LiveTeam {
        LiveTeam {
            name: code.to_string(),
            code: code.to_string(),
            position: pos,
            played,
            points: pts,
            goals_for: gf,
            goals_against: gf - gd,
            goal_diff: gd,
            disciplinary: 0,
        }
    }

    fn fx(h: &str, a: &str) -> GroupFixture {
        GroupFixture {
            home: h.to_string(),
            away: a.to_string(),
        }
    }

    #[test]
    fn leader_with_unbeatable_lead_qualifies_no_help() {
        // Last round, one game each. AAA on 6, others ≤1 — already untouchable.
        let teams = vec![
            team_p("AAA", 1, 6, 4, 5, 2),
            team_p("BBB", 2, 1, 0, 2, 2),
            team_p("CCC", 3, 1, -2, 1, 2),
            team_p("DDD", 4, 1, -2, 1, 2),
        ];
        let rem = vec![fx("AAA", "BBB"), fx("CCC", "DDD")];
        let GroupScenarios::Ready(s) = group_scenarios(&teams, &rem) else {
            panic!("expected Ready");
        };
        let aaa = s.iter().find(|t| t.code == "AAA").unwrap();
        assert_eq!(aaa.possible, vec![1]);
        // Every branch keeps AAA top-2 with no help.
        assert!(aaa.branches.iter().all(|b| b.worst <= 2));
    }

    #[test]
    fn done_when_no_matches_remain() {
        let teams = vec![team_p("AAA", 1, 9, 5, 6, 3), team_p("BBB", 2, 6, 2, 4, 3)];
        assert!(matches!(
            group_scenarios(&teams, &[]),
            GroupScenarios::Done
        ));
    }

    #[test]
    fn win_guarantees_top_two_for_midtable_team() {
        // Everyone level on 3 going into the last round; a win always reaches top 2.
        let teams = vec![
            team_p("AAA", 1, 3, 0, 2, 2),
            team_p("BBB", 2, 3, 0, 2, 2),
            team_p("CCC", 3, 3, 0, 2, 2),
            team_p("DDD", 4, 3, 0, 2, 2),
        ];
        let rem = vec![fx("AAA", "CCC"), fx("BBB", "DDD")];
        let GroupScenarios::Ready(s) = group_scenarios(&teams, &rem) else {
            panic!("expected Ready");
        };
        let aaa = s.iter().find(|t| t.code == "AAA").unwrap();
        let win = aaa.branches.iter().find(|b| b.own == OwnResult::Win).unwrap();
        assert!(win.worst <= 2, "a win should guarantee top two here");
        // Conditions must name the *other* match (BBB vs DDD), not be vague.
        assert!(!win.conditions.is_empty());
        assert!(
            win.conditions
                .iter()
                .any(|c| c.other.contains("BBB") && c.other.contains("DDD")),
            "expected the other match spelled out, got {:?}",
            win.conditions.iter().map(|c| &c.other).collect::<Vec<_>>()
        );
    }

    #[test]
    fn condition_reports_winning_margin_threshold() {
        // AAA & CCC tied; AAA's finish vs CCC turns on goal difference when CCC
        // wins its game — so a winning-margin rule should surface.
        let teams = vec![
            team_p("AAA", 1, 3, 1, 3, 2),
            team_p("CCC", 2, 3, 1, 3, 2),
            team_p("BBB", 3, 3, 0, 2, 2),
            team_p("DDD", 4, 0, -2, 0, 2),
        ];
        let rem = vec![fx("AAA", "BBB"), fx("CCC", "DDD")];
        let GroupScenarios::Ready(s) = group_scenarios(&teams, &rem) else {
            panic!("expected Ready");
        };
        let aaa = s.iter().find(|t| t.code == "AAA").unwrap();
        let has_rule = aaa.branches.iter().flat_map(|b| &b.conditions).any(|c| {
            c.gd.is_some()
                || c.detail.contains("by ≥")
                || c.detail.contains("by ≤")
                || c.detail.contains("unless")
                || c.detail.contains("margins")
        });
        assert!(has_rule, "expected a goal-margin / threat rule in some condition");
    }

    #[test]
    fn third_place_bottom_four_eliminated_when_all_finished() {
        // 12 finished groups; the 3rd-place team's points descend A=12 … L=1.
        // Best 8 advance, so I/J/K/L (points 4/3/2/1) are eliminated.
        let standings: Vec<LiveStanding> = ('A'..='L')
            .enumerate()
            .map(|(i, g)| LiveStanding {
                group: g,
                teams: vec![
                    team_p("T1", 1, 30, 9, 9, 3),
                    team_p("T2", 2, 20, 5, 6, 3),
                    team_p(&format!("{g}3"), 3, (12 - i) as i64, 0, 3, 3),
                    team_p("T4", 4, 0, -9, 0, 3),
                ],
            })
            .collect();

        let out = third_place_outlook(&standings, &[]);
        let elim: Vec<&String> = out.iter().filter(|(_, o)| o.eliminated).map(|(c, _)| c).collect();
        assert_eq!(elim.len(), 4);
        assert!(out["L3"].eliminated && out["I3"].eliminated);
        assert!(!out["H3"].eliminated); // 8th best still advances
        // The runaway top third is guaranteed in; the doomed one never advances.
        assert!(out["A3"].clinched && out["A3"].pct > 0.99);
        assert!(out["L3"].pct < 0.01);
    }

    #[test]
    fn too_early_with_many_matches_left() {
        let teams = vec![
            team_p("AAA", 1, 0, 0, 0, 0),
            team_p("BBB", 2, 0, 0, 0, 0),
            team_p("CCC", 3, 0, 0, 0, 0),
            team_p("DDD", 4, 0, 0, 0, 0),
        ];
        let rem = vec![
            fx("AAA", "BBB"),
            fx("CCC", "DDD"),
            fx("AAA", "CCC"),
            fx("BBB", "DDD"),
        ];
        assert!(matches!(
            group_scenarios(&teams, &rem),
            GroupScenarios::TooEarly(4)
        ));
    }
}
