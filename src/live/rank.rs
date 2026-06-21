//! Ranking + projection math for live standings.

use std::collections::HashMap;

use super::*;

/// Each group team plays 3 group-stage matches.
const GROUP_GAMES: u32 = 3;

/// Map of team code → clinched final group position (1..4), for teams whose
/// position is mathematically fixed regardless of remaining results.
///
/// Points-only (conservative): A is guaranteed above B when A's worst-case
/// points (lose all remaining) still beat B's best-case (win all remaining). A
/// team's rank is locked once every rival is guaranteed above or below it. Two
/// teams that could finish level on points (goal difference would decide) are
/// *not* locked — so a team only goes solid when truly certain.
pub(crate) fn clinched_positions(standings: &[LiveStanding]) -> HashMap<String, u32> {
    let ceil = |t: &LiveTeam| t.points + 3 * i64::from(GROUP_GAMES.saturating_sub(t.played));
    let floor = |t: &LiveTeam| t.points;

    let mut out = HashMap::new();
    for s in standings {
        let others = s.teams.len().saturating_sub(1);
        let all_played = s.teams.iter().all(|t| t.played >= GROUP_GAMES);
        for t in &s.teams {
            if all_played {
                // Group finished → positions are final (sorted order is authoritative).
                out.insert(t.code.clone(), t.position);
                continue;
            }
            let mut above = 0usize;
            let mut below = 0usize;
            for o in &s.teams {
                if o.code == t.code {
                    continue;
                }
                if floor(o) > ceil(t) {
                    above += 1;
                } else if ceil(o) < floor(t) {
                    below += 1;
                }
            }
            if above + below == others {
                out.insert(t.code.clone(), (above + 1) as u32);
            }
        }
    }
    out
}

/// FIFA-ranking order of the 48 finalists (best first), used as the final
/// tiebreaker so ties resolve deterministically. The first 37 follow the
/// official FIFA Men's World Ranking top 50 (June 2026, finalists only); the
/// remaining 11 (outside the top 50) are approximate.
#[rustfmt::skip]
const FIFA_ORDER: [&str; 48] = [
    // Finalists within FIFA's published top 50, in exact order.
    "ARG", "ESP", "FRA", "ENG", "POR", "BRA", "MAR", "NED", "BEL", "GER",
    "CRO", "COL", "MEX", "SEN", "URU", "USA", "JPN", "SUI", "IRN", "TUR",
    "ECU", "AUT", "KOR", "AUS", "ALG", "EGY", "CAN", "NOR", "CIV", "PAN",
    "SWE", "CZE", "PAR", "SCO", "TUN", "COD", "UZB",
    // Finalists outside the top 50 (approximate).
    "RSA", "QAT", "KSA", "BIH", "CPV", "NZL", "IRQ", "JOR", "GHA", "HAI", "CUW",
];

/// A team's FIFA-ranking slot (lower is better); unknown codes sort last.
pub(crate) fn fifa_rank(code: &str) -> usize {
    FIFA_ORDER
        .iter()
        .position(|c| *c == code)
        .unwrap_or(usize::MAX)
}

/// Sort every group by the FIFA tiebreak chain: points → GD → GF → fair-play
/// (fewest disciplinary points) → FIFA rank → code. The last two keep tied teams
/// from flipping between polls. Renumbers positions.
pub(crate) fn sort_standings(standings: &mut [LiveStanding]) {
    for s in standings.iter_mut() {
        s.teams.sort_by(|x, y| {
            y.points
                .cmp(&x.points)
                .then(y.goal_diff.cmp(&x.goal_diff))
                .then(y.goals_for.cmp(&x.goals_for))
                // Fewer disciplinary points (yellow/red cards) ranks higher.
                .then(x.disciplinary.cmp(&y.disciplinary))
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
    for f in fixtures.iter().filter(|f| f.status.is_live()) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn team(code: &str, pos: u32, pts: i64, gd: i64, gf: i64) -> LiveTeam {
        team_p(code, pos, pts, gd, gf, 3)
    }

    fn team_p(code: &str, pos: u32, pts: i64, gd: i64, gf: i64, played: u32) -> LiveTeam {
        LiveTeam {
            name: code.to_string(),
            code: code.to_string(),
            position: pos,
            played,
            points: pts,
            goal_diff: gd,
            goals_for: gf,
            goals_against: gf - gd,
            disciplinary: 0,
        }
    }

    #[test]
    fn clinch_locks_leader_but_not_open_places() {
        // One game left each. A on 6 can't be caught; 2nd–4th still open.
        let s = vec![LiveStanding {
            group: 'A',
            teams: vec![
                team_p("AAA", 1, 6, 0, 0, 2),
                team_p("BBB", 2, 0, 0, 0, 2),
                team_p("CCC", 3, 0, 0, 0, 2),
                team_p("DDD", 4, 0, 0, 0, 2),
            ],
        }];
        let c = clinched_positions(&s);
        assert_eq!(c.get("AAA"), Some(&1));
        assert_eq!(c.get("BBB"), None); // 2nd/3rd/4th undecided
        assert_eq!(c.get("CCC"), None);
    }

    #[test]
    fn clinch_locks_all_when_group_finished() {
        let mut s = vec![LiveStanding {
            group: 'A',
            teams: vec![
                team("AAA", 1, 9, 5, 6),
                team("BBB", 2, 6, 2, 4),
                team("CCC", 3, 3, -1, 2),
                team("DDD", 4, 0, -6, 1),
            ],
        }];
        sort_standings(&mut s);
        let c = clinched_positions(&s);
        assert_eq!(c.len(), 4);
        assert_eq!(c.get("AAA"), Some(&1));
        assert_eq!(c.get("DDD"), Some(&4));
    }

    #[test]
    fn sort_breaks_ties_by_fifa_rank_and_renumbers() {
        // JPN (FIFA 17) and NED (FIFA 8) are level on points/GD/GF.
        let mut s = vec![LiveStanding {
            group: 'F',
            teams: vec![team("JPN", 0, 3, 0, 2), team("NED", 0, 3, 0, 2)],
        }];
        sort_standings(&mut s);
        assert_eq!(s[0].teams[0].code, "NED"); // better FIFA rank wins the tie
        assert_eq!(s[0].teams[0].position, 1);
        assert_eq!(s[0].teams[1].position, 2);
    }

    #[test]
    fn third_place_ranking_advances_top_eight() {
        // 12 groups; the 3rd-place team's points descend A=12 … L=1.
        let standings: Vec<LiveStanding> = ('A'..='L')
            .enumerate()
            .map(|(i, g)| LiveStanding {
                group: g,
                teams: vec![
                    team("Q1", 1, 9, 0, 0),
                    team("Q2", 2, 6, 0, 0),
                    team(&format!("{g}3"), 3, (12 - i) as i64, 0, 0),
                    team("X4", 4, 0, 0, 0),
                ],
            })
            .collect();

        let ranks = third_place_ranking(&standings);
        assert_eq!(ranks.len(), 12);
        assert_eq!(ranks.iter().filter(|r| r.advances).count(), 8);
        assert!(ranks[0].advances && ranks[0].points == 12);
        assert!(!ranks[11].advances && ranks[11].points == 1);
    }

    #[test]
    fn third_place_tiebreak_prefers_fewer_cards() {
        // Two 3rd-place teams identical except disciplinary points.
        let mk = |g: char, code: &str, disc: i64| {
            let mut t = team(code, 3, 3, 0, 2);
            t.disciplinary = disc;
            LiveStanding {
                group: g,
                teams: vec![team("a", 1, 9, 0, 0), team("b", 2, 6, 0, 0), t, team("c", 4, 0, 0, 0)],
            }
        };
        let ranks = third_place_ranking(&[mk('A', "ZZA", 4), mk('B', "ZZB", 1)]);
        assert_eq!(ranks[0].code, "ZZB"); // fewer cards ranks higher
    }
}
