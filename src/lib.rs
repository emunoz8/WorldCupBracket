use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type Allocation = HashMap<String, String>;
pub type Annex = HashMap<String, Allocation>;
pub type MatchCounts = HashMap<String, HashMap<String, usize>>;

const WINNER_SLOTS: [&str; 8] = ["1A", "1B", "1D", "1E", "1G", "1I", "1K", "1L"];

#[derive(Debug, PartialEq)]
pub struct AnnexTableRow {
    pub option: usize,
    pub allocation: Allocation,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct OpponentPrediction {
    pub opponent: String,
    pub count: usize,
    pub percentage: f64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct WinnerPrediction {
    pub winner_slot: String,
    pub total: usize,
    pub opponents: Vec<OpponentPrediction>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct PredictionReport {
    pub known_passing: String,
    pub known_eliminated: String,
    pub possible_scenarios: usize,
    pub predictions: Vec<WinnerPrediction>,
    pub errors: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum ThirdPlaceStatus {
    Unknown,
    Advanced,
    Eliminated,
}

impl ThirdPlaceStatus {
    pub fn next(self) -> Self {
        match self {
            Self::Unknown => Self::Advanced,
            Self::Advanced => Self::Eliminated,
            Self::Eliminated => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Advanced => "Advanced",
            Self::Eliminated => "Eliminated",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Team {
    pub name: String,
    pub code: String,
    /// Flag emoji (regional-indicator pair). Renders in the printed HTML; egui's
    /// default font cannot draw flag glyphs, so it is unused on screen.
    #[serde(default)]
    pub flag: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GroupState {
    pub group: char,
    pub teams: Vec<Team>,
    pub third_place_status: ThirdPlaceStatus,
    /// True once the final group standings are known for certain. Drives whether
    /// this group's teams appear solid (vs shaded) in the bracket.
    #[serde(default)]
    pub completed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BracketMatch {
    pub match_number: usize,
    pub left_slot: &'static str,
    pub right_slot: &'static str,
}

pub const ROUND_OF_32_MATCHES: [BracketMatch; 16] = [
    BracketMatch {
        match_number: 73,
        left_slot: "2A",
        right_slot: "2B",
    },
    BracketMatch {
        match_number: 74,
        left_slot: "1E",
        right_slot: "3rd",
    },
    BracketMatch {
        match_number: 75,
        left_slot: "1F",
        right_slot: "2C",
    },
    BracketMatch {
        match_number: 76,
        left_slot: "1C",
        right_slot: "2F",
    },
    BracketMatch {
        match_number: 77,
        left_slot: "1I",
        right_slot: "3rd",
    },
    BracketMatch {
        match_number: 78,
        left_slot: "2E",
        right_slot: "2I",
    },
    BracketMatch {
        match_number: 79,
        left_slot: "1A",
        right_slot: "3rd",
    },
    BracketMatch {
        match_number: 80,
        left_slot: "1L",
        right_slot: "3rd",
    },
    BracketMatch {
        match_number: 81,
        left_slot: "1D",
        right_slot: "3rd",
    },
    BracketMatch {
        match_number: 82,
        left_slot: "1G",
        right_slot: "3rd",
    },
    BracketMatch {
        match_number: 83,
        left_slot: "2K",
        right_slot: "2L",
    },
    BracketMatch {
        match_number: 84,
        left_slot: "1H",
        right_slot: "2J",
    },
    BracketMatch {
        match_number: 85,
        left_slot: "1B",
        right_slot: "3rd",
    },
    BracketMatch {
        match_number: 86,
        left_slot: "1J",
        right_slot: "2H",
    },
    BracketMatch {
        match_number: 87,
        left_slot: "1K",
        right_slot: "3rd",
    },
    BracketMatch {
        match_number: 88,
        left_slot: "2D",
        right_slot: "2G",
    },
];

/// Which competitor of a match advanced.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Side {
    Left,
    Right,
}

/// Where a knockout competitor comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    /// A group-stage placement slot, e.g. "1A" or "2B".
    Group(&'static str),
    /// The annex-allocated third-place opponent ("3rd").
    ThirdPlace,
    /// The winner of an earlier knockout match number.
    Winner(usize),
    /// The loser of an earlier knockout match number (third-place playoff).
    Loser(usize),
}

/// One knockout match, generalised across every round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KoMatch {
    pub match_number: usize,
    /// 0 = Round of 32, 1 = Round of 16, 2 = QF, 3 = SF, 4 = Final.
    pub round: u8,
    pub left: Slot,
    pub right: Slot,
}

/// Venue for a knockout match: (stadium, state/region, country). Source: the
/// official 2026 FIFA World Cup knockout schedule (matches 73–104).
#[rustfmt::skip]
pub fn match_venue(match_number: usize) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match match_number {
        73 => ("SoFi Stadium", "California", "USA"),
        74 => ("Gillette Stadium", "Massachusetts", "USA"),
        75 => ("Estadio BBVA", "Nuevo León", "Mexico"),
        76 => ("NRG Stadium", "Texas", "USA"),
        77 => ("MetLife Stadium", "New Jersey", "USA"),
        78 => ("AT&T Stadium", "Texas", "USA"),
        79 => ("Estadio Azteca", "Mexico City", "Mexico"),
        80 => ("Mercedes-Benz Stadium", "Georgia", "USA"),
        81 => ("Levi's Stadium", "California", "USA"),
        82 => ("Lumen Field", "Washington", "USA"),
        83 => ("BMO Field", "Ontario", "Canada"),
        84 => ("SoFi Stadium", "California", "USA"),
        85 => ("BC Place", "British Columbia", "Canada"),
        86 => ("Hard Rock Stadium", "Florida", "USA"),
        87 => ("Arrowhead Stadium", "Missouri", "USA"),
        88 => ("AT&T Stadium", "Texas", "USA"),
        89 => ("Lincoln Financial Field", "Pennsylvania", "USA"),
        90 => ("NRG Stadium", "Texas", "USA"),
        91 => ("MetLife Stadium", "New Jersey", "USA"),
        92 => ("Estadio Azteca", "Mexico City", "Mexico"),
        93 => ("AT&T Stadium", "Texas", "USA"),
        94 => ("Lumen Field", "Washington", "USA"),
        95 => ("Mercedes-Benz Stadium", "Georgia", "USA"),
        96 => ("BC Place", "British Columbia", "Canada"),
        97 => ("Gillette Stadium", "Massachusetts", "USA"),
        98 => ("SoFi Stadium", "California", "USA"),
        99 => ("Hard Rock Stadium", "Florida", "USA"),
        100 => ("Arrowhead Stadium", "Missouri", "USA"),
        101 => ("AT&T Stadium", "Texas", "USA"),
        102 => ("Mercedes-Benz Stadium", "Georgia", "USA"),
        103 => ("Hard Rock Stadium", "Florida", "USA"),
        104 => ("MetLife Stadium", "New Jersey", "USA"),
        _ => return None,
    })
}

/// Build the full knockout tree (R32 → Final) with each match wired to its feeders.
pub fn knockout_matches() -> Vec<KoMatch> {
    let mut matches: Vec<KoMatch> = ROUND_OF_32_MATCHES
        .iter()
        .map(|m| KoMatch {
            match_number: m.match_number,
            round: 0,
            left: Slot::Group(m.left_slot),
            right: if m.right_slot == "3rd" {
                Slot::ThirdPlace
            } else {
                Slot::Group(m.right_slot)
            },
        })
        .collect();

    let w = Slot::Winner;
    // (match number, left feeder, right feeder)
    let r16 = [
        (89, 74, 77),
        (90, 73, 75),
        (91, 76, 78),
        (92, 79, 80),
        (93, 83, 84),
        (94, 81, 82),
        (95, 86, 88),
        (96, 85, 87),
    ];
    let qf = [(97, 89, 90), (98, 93, 94), (99, 91, 92), (100, 95, 96)];
    let sf = [(101, 97, 98), (102, 99, 100)];

    for (number, left, right) in r16 {
        matches.push(KoMatch {
            match_number: number,
            round: 1,
            left: w(left),
            right: w(right),
        });
    }
    for (number, left, right) in qf {
        matches.push(KoMatch {
            match_number: number,
            round: 2,
            left: w(left),
            right: w(right),
        });
    }
    for (number, left, right) in sf {
        matches.push(KoMatch {
            match_number: number,
            round: 3,
            left: w(left),
            right: w(right),
        });
    }
    matches.push(KoMatch {
        match_number: 104,
        round: 4,
        left: w(101),
        right: w(102),
    });
    // Third-place playoff: the two beaten semi-finalists.
    matches.push(KoMatch {
        match_number: 103,
        round: 5,
        left: Slot::Loser(101),
        right: Slot::Loser(102),
    });

    matches
}

pub fn annex_filters_from_groups(groups: &[GroupState]) -> (String, String) {
    let mut passing = String::new();
    let mut eliminated = String::new();

    for group in groups {
        match group.third_place_status {
            ThirdPlaceStatus::Advanced => passing.push(group.group),
            ThirdPlaceStatus::Eliminated => eliminated.push(group.group),
            ThirdPlaceStatus::Unknown => {}
        }
    }

    (normalize_groups(&passing), normalize_groups(&eliminated))
}

/// The official 2026 group draw: (group, team name, 3-letter code, flag emoji).
/// Flags render in the printed HTML; egui's font cannot draw them on screen.
#[rustfmt::skip]
pub const SEED_TEAMS: [(char, &str, &str, &str); 48] = [
    ('A', "Mexico", "MEX", "🇲🇽"), ('A', "South Korea", "KOR", "🇰🇷"), ('A', "Czechia", "CZE", "🇨🇿"), ('A', "South Africa", "RSA", "🇿🇦"),
    ('B', "Switzerland", "SUI", "🇨🇭"), ('B', "Canada", "CAN", "🇨🇦"), ('B', "Qatar", "QAT", "🇶🇦"), ('B', "Bosnia-Herzegovina", "BIH", "🇧🇦"),
    ('C', "Scotland", "SCO", "🏴󠁧󠁢󠁳󠁣󠁴󠁿"), ('C', "Morocco", "MAR", "🇲🇦"), ('C', "Brazil", "BRA", "🇧🇷"), ('C', "Haiti", "HAI", "🇭🇹"),
    ('D', "United States", "USA", "🇺🇸"), ('D', "Australia", "AUS", "🇦🇺"), ('D', "Türkiye", "TUR", "🇹🇷"), ('D', "Paraguay", "PAR", "🇵🇾"),
    ('E', "Germany", "GER", "🇩🇪"), ('E', "Ivory Coast", "CIV", "🇨🇮"), ('E', "Ecuador", "ECU", "🇪🇨"), ('E', "Curaçao", "CUW", "🇨🇼"),
    ('F', "Sweden", "SWE", "🇸🇪"), ('F', "Japan", "JPN", "🇯🇵"), ('F', "Netherlands", "NED", "🇳🇱"), ('F', "Tunisia", "TUN", "🇹🇳"),
    ('G', "New Zealand", "NZL", "🇳🇿"), ('G', "Iran", "IRN", "🇮🇷"), ('G', "Belgium", "BEL", "🇧🇪"), ('G', "Egypt", "EGY", "🇪🇬"),
    ('H', "Uruguay", "URU", "🇺🇾"), ('H', "Saudi Arabia", "KSA", "🇸🇦"), ('H', "Spain", "ESP", "🇪🇸"), ('H', "Cape Verde", "CPV", "🇨🇻"),
    ('I', "Norway", "NOR", "🇳🇴"), ('I', "France", "FRA", "🇫🇷"), ('I', "Senegal", "SEN", "🇸🇳"), ('I', "Iraq", "IRQ", "🇮🇶"),
    ('J', "Argentina", "ARG", "🇦🇷"), ('J', "Austria", "AUT", "🇦🇹"), ('J', "Jordan", "JOR", "🇯🇴"), ('J', "Algeria", "ALG", "🇩🇿"),
    ('K', "Congo DR", "COD", "🇨🇩"), ('K', "Portugal", "POR", "🇵🇹"), ('K', "Colombia", "COL", "🇨🇴"), ('K', "Uzbekistan", "UZB", "🇺🇿"),
    ('L', "Croatia", "CRO", "🇭🇷"), ('L', "England", "ENG", "🏴󠁧󠁢󠁥󠁮󠁧󠁿"), ('L', "Ghana", "GHA", "🇬🇭"), ('L', "Panama", "PAN", "🇵🇦"),
];

/// Build the group standings from the committed seed table (real teams + flags).
pub fn seed_group_states() -> Vec<GroupState> {
    ('A'..='L')
        .map(|group| GroupState {
            group,
            teams: SEED_TEAMS
                .iter()
                .filter(|(g, ..)| *g == group)
                .map(|(_, name, code, flag)| Team {
                    name: name.to_string(),
                    code: code.to_string(),
                    flag: flag.to_string(),
                })
                .collect(),
            third_place_status: ThirdPlaceStatus::Unknown,
            completed: false,
        })
        .collect()
}

pub fn prediction_report(
    annex: &Annex,
    known_passing: &str,
    known_eliminated: &str,
) -> PredictionReport {
    let passing = normalize_groups(known_passing);
    let eliminated = normalize_groups(known_eliminated);
    let mut errors = Vec::new();

    if let Some(group) = passing.chars().find(|group| eliminated.contains(*group)) {
        errors.push(format!(
            "Group {group} cannot be both passing and eliminated."
        ));
    }

    let matching_allocations: Vec<&Allocation> = if errors.is_empty() {
        annex
            .iter()
            .filter(|(qualified_groups, _)| contains_all_groups(qualified_groups, &passing))
            .filter(|(qualified_groups, _)| contains_no_groups(qualified_groups, &eliminated))
            .map(|(_, allocation)| allocation)
            .collect()
    } else {
        Vec::new()
    };

    let counts = count_allocations(&matching_allocations);
    let predictions = build_predictions(&counts);

    PredictionReport {
        known_passing: passing,
        known_eliminated: eliminated,
        possible_scenarios: matching_allocations.len(),
        predictions,
        errors,
    }
}

pub fn count_matches(annex: &Annex) -> MatchCounts {
    let allocations: Vec<&Allocation> = annex.values().collect();
    count_allocations(&allocations)
}

pub fn format_match_counts(counts: &MatchCounts) -> String {
    let mut report = String::new();

    for prediction in build_predictions(counts) {
        report.push_str(&format!("{} {{\n", prediction.winner_slot));

        for opponent in prediction.opponents {
            report.push_str(&format!(
                "  {}= {} ({:.2}%)\n",
                opponent.opponent, opponent.count, opponent.percentage
            ));
        }

        report.push_str("}\n\n");
    }

    report
}

pub fn parse_annex_table(text: &str) -> Vec<AnnexTableRow> {
    text.lines().filter_map(parse_annex_row).collect()
}

pub fn annex_from_table_text(text: &str) -> Result<Annex, String> {
    annex_from_rows(
        parse_annex_table(text),
        qualified_group_keys_in_option_order(),
    )
}

pub fn qualified_group_keys_in_option_order() -> Vec<String> {
    let mut keys = Vec::new();
    let groups: Vec<char> = ('A'..='L').collect();
    collect_group_keys(&groups, 8, 0, &mut Vec::new(), &mut keys);
    keys.reverse();
    keys
}

pub fn annex_from_rows(rows: Vec<AnnexTableRow>, keys: Vec<String>) -> Result<Annex, String> {
    if rows.len() != keys.len() {
        return Err(format!(
            "Expected {} Annex C rows, found {}.",
            keys.len(),
            rows.len()
        ));
    }

    let mut annex = Annex::new();

    for (index, (row, key)) in rows.into_iter().zip(keys).enumerate() {
        let expected_option = index + 1;

        if row.option != expected_option {
            return Err(format!(
                "Expected option {expected_option}, found option {}.",
                row.option
            ));
        }

        annex.insert(key, row.allocation);
    }

    Ok(annex)
}

fn parse_annex_row(line: &str) -> Option<AnnexTableRow> {
    let tokens: Vec<&str> = line.split_whitespace().collect();

    if tokens.len() != WINNER_SLOTS.len() + 1 {
        return None;
    }

    let option = tokens[0].parse().ok()?;
    let opponents = &tokens[1..];

    if !opponents
        .iter()
        .all(|opponent| is_third_place_entry(opponent))
    {
        return None;
    }

    let allocation = WINNER_SLOTS
        .iter()
        .zip(opponents)
        .map(|(slot, opponent)| ((*slot).to_string(), (*opponent).to_string()))
        .collect();

    Some(AnnexTableRow { option, allocation })
}

fn is_third_place_entry(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some('3'), Some('A'..='L'), None)
    )
}

fn collect_group_keys(
    groups: &[char],
    length: usize,
    start: usize,
    current: &mut Vec<char>,
    keys: &mut Vec<String>,
) {
    if current.len() == length {
        keys.push(current.iter().collect());
        return;
    }

    for index in start..groups.len() {
        current.push(groups[index]);
        collect_group_keys(groups, length, index + 1, current, keys);
        current.pop();
    }
}

fn normalize_groups(input: &str) -> String {
    let mut chars: Vec<char> = input
        .chars()
        .filter(|c| matches!(c.to_ascii_uppercase(), 'A'..='L'))
        .map(|c| c.to_ascii_uppercase())
        .collect();

    chars.sort_unstable();
    chars.dedup();

    chars.into_iter().collect()
}

fn contains_all_groups(qualified_groups: &str, groups: &str) -> bool {
    groups.chars().all(|group| qualified_groups.contains(group))
}

fn contains_no_groups(qualified_groups: &str, groups: &str) -> bool {
    groups
        .chars()
        .all(|group| !qualified_groups.contains(group))
}

fn count_allocations(allocations: &[&Allocation]) -> MatchCounts {
    let mut counts = MatchCounts::new();

    for allocation in allocations {
        for (winner_slot, opponent) in *allocation {
            *counts
                .entry(winner_slot.clone())
                .or_default()
                .entry(opponent.clone())
                .or_default() += 1;
        }
    }

    counts
}

fn build_predictions(counts: &MatchCounts) -> Vec<WinnerPrediction> {
    let mut winner_slots: Vec<&String> = counts.keys().collect();
    winner_slots.sort();

    winner_slots
        .into_iter()
        .map(|winner_slot| {
            let total: usize = counts[winner_slot].values().sum();
            let mut opponents: Vec<OpponentPrediction> = counts[winner_slot]
                .iter()
                .map(|(opponent, count)| OpponentPrediction {
                    opponent: opponent.clone(),
                    count: *count,
                    percentage: rounded_percentage(*count, total),
                })
                .collect();

            opponents.sort_by(|left, right| {
                right
                    .count
                    .cmp(&left.count)
                    .then_with(|| left.opponent.cmp(&right.opponent))
            });

            WinnerPrediction {
                winner_slot: winner_slot.clone(),
                total,
                opponents,
            }
        })
        .collect()
}

fn rounded_percentage(count: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }

    (((count as f64 / total as f64) * 100.0) * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_possible_scenarios_by_known_passing_and_eliminated_groups() {
        let annex = HashMap::from([
            (
                "ABCD".to_string(),
                HashMap::from([("1A".to_string(), "3A".to_string())]),
            ),
            (
                "ABCE".to_string(),
                HashMap::from([("1A".to_string(), "3B".to_string())]),
            ),
            (
                "ACDE".to_string(),
                HashMap::from([("1A".to_string(), "3C".to_string())]),
            ),
        ]);

        let report = prediction_report(&annex, "A C", "E");

        assert_eq!(report.known_passing, "AC");
        assert_eq!(report.known_eliminated, "E");
        assert_eq!(report.possible_scenarios, 1);
        assert_eq!(report.predictions[0].opponents[0].opponent, "3A");
    }

    #[test]
    fn counts_each_opponent_for_each_winner_slot() {
        let annex = HashMap::from([
            (
                "ABCDEFGH".to_string(),
                HashMap::from([
                    ("1A".to_string(), "3A".to_string()),
                    ("1B".to_string(), "3C".to_string()),
                ]),
            ),
            (
                "ABCDEFGI".to_string(),
                HashMap::from([
                    ("1A".to_string(), "3A".to_string()),
                    ("1B".to_string(), "3D".to_string()),
                ]),
            ),
        ]);

        let counts = count_matches(&annex);

        assert_eq!(counts["1A"]["3A"], 2);
        assert_eq!(counts["1B"]["3C"], 1);
        assert_eq!(counts["1B"]["3D"], 1);
    }

    #[test]
    fn formats_counts_grouped_by_winner_slot() {
        let counts = HashMap::from([
            (
                "1B".to_string(),
                HashMap::from([("3D".to_string(), 1), ("3C".to_string(), 3)]),
            ),
            ("1A".to_string(), HashMap::from([("3A".to_string(), 2)])),
        ]);

        let report = format_match_counts(&counts);

        assert_eq!(
            report,
            "1A {\n  3A= 2 (100.00%)\n}\n\n1B {\n  3C= 3 (75.00%)\n  3D= 1 (25.00%)\n}\n\n"
        );
    }

    #[test]
    fn computes_percentages_from_remaining_possible_scenarios() {
        let annex = HashMap::from([
            (
                "ABCD".to_string(),
                HashMap::from([("1A".to_string(), "3A".to_string())]),
            ),
            (
                "ABCE".to_string(),
                HashMap::from([("1A".to_string(), "3A".to_string())]),
            ),
            (
                "ABCF".to_string(),
                HashMap::from([("1A".to_string(), "3B".to_string())]),
            ),
        ]);

        let report = prediction_report(&annex, "A B C", "");
        let opponents = &report.predictions[0].opponents;

        assert_eq!(report.possible_scenarios, 3);
        assert_eq!(opponents[0].opponent, "3A");
        assert_eq!(opponents[0].count, 2);
        assert_eq!(opponents[0].percentage, 66.67);
        assert_eq!(opponents[1].opponent, "3B");
        assert_eq!(opponents[1].count, 1);
        assert_eq!(opponents[1].percentage, 33.33);
    }

    #[test]
    fn parses_only_numbered_rows_with_eight_third_place_entries() {
        let text = "\
ANNEXE
Option   1A   1B   1D   1E   1G   1I   1K   1L
80
1  3E  3J  3I  3F  3H  3G  3L  3K
Annexes
2  3H  3G  3I  3D  3J  3F  3L  3K
3  3E  3J  3I  3D  3H  3G  3L
4  3E  3J  3I  3D  3H  3G  XX  3K
";

        let rows = parse_annex_table(text);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].option, 1);
        assert_eq!(rows[0].allocation["1A"], "3E");
        assert_eq!(rows[0].allocation["1L"], "3K");
        assert_eq!(rows[1].option, 2);
        assert_eq!(rows[1].allocation["1E"], "3D");
    }

    #[test]
    fn generates_qualified_group_keys_in_pdf_option_order() {
        let keys = qualified_group_keys_in_option_order();

        assert_eq!(keys.len(), 495);
        assert_eq!(keys[0], "EFGHIJKL");
        assert_eq!(keys[1], "DFGHIJKL");
        assert_eq!(keys[494], "ABCDEFGH");
    }

    #[test]
    fn builds_annex_from_table_text_by_pairing_rows_with_option_order_keys() {
        let text = "\
1  3E  3J  3I  3F  3H  3G  3L  3K
2  3H  3G  3I  3D  3J  3F  3L  3K
";

        let annex = annex_from_rows(
            parse_annex_table(text),
            vec!["EFGHIJKL".into(), "DFGHIJKL".into()],
        )
        .expect("rows and keys should build an annex");

        assert_eq!(annex["EFGHIJKL"]["1A"], "3E");
        assert_eq!(annex["EFGHIJKL"]["1L"], "3K");
        assert_eq!(annex["DFGHIJKL"]["1A"], "3H");
        assert_eq!(annex["DFGHIJKL"]["1E"], "3D");
    }

    #[test]
    fn converts_third_place_statuses_to_annex_filters() {
        let groups = vec![
            GroupState {
                group: 'C',
                teams: vec![],
                third_place_status: ThirdPlaceStatus::Advanced,
                completed: false,
            },
            GroupState {
                group: 'A',
                teams: vec![],
                third_place_status: ThirdPlaceStatus::Eliminated,
                completed: false,
            },
            GroupState {
                group: 'B',
                teams: vec![],
                third_place_status: ThirdPlaceStatus::Unknown,
                completed: false,
            },
            GroupState {
                group: 'H',
                teams: vec![],
                third_place_status: ThirdPlaceStatus::Advanced,
                completed: false,
            },
        ];

        assert_eq!(
            annex_filters_from_groups(&groups),
            ("CH".into(), "A".into())
        );
    }

    #[test]
    fn cycles_third_place_status_for_bracket_builder() {
        assert_eq!(ThirdPlaceStatus::Unknown.next(), ThirdPlaceStatus::Advanced);
        assert_eq!(
            ThirdPlaceStatus::Advanced.next(),
            ThirdPlaceStatus::Eliminated
        );
        assert_eq!(
            ThirdPlaceStatus::Eliminated.next(),
            ThirdPlaceStatus::Unknown
        );
    }

    #[test]
    fn round_of_32_matches_follow_fixed_match_number_order() {
        assert_eq!(ROUND_OF_32_MATCHES.len(), 16);
        assert_eq!(ROUND_OF_32_MATCHES[0].match_number, 73);
        assert_eq!(ROUND_OF_32_MATCHES[0].left_slot, "2A");
        assert_eq!(ROUND_OF_32_MATCHES[0].right_slot, "2B");
        assert_eq!(ROUND_OF_32_MATCHES[15].match_number, 88);
        assert_eq!(ROUND_OF_32_MATCHES[15].left_slot, "2D");
        assert_eq!(ROUND_OF_32_MATCHES[15].right_slot, "2G");
    }

    #[test]
    fn round_of_32_annex_slots_are_the_eight_first_place_prediction_slots() {
        let dynamic_slots: Vec<&str> = ROUND_OF_32_MATCHES
            .iter()
            .filter(|matchup| matchup.right_slot == "3rd")
            .map(|matchup| matchup.left_slot)
            .collect();

        assert_eq!(
            dynamic_slots,
            vec!["1E", "1I", "1A", "1L", "1D", "1G", "1B", "1K"]
        );
    }

    #[test]
    fn knockout_tree_wires_each_round_to_its_feeders() {
        let matches = knockout_matches();
        let by_number: HashMap<usize, KoMatch> =
            matches.iter().map(|m| (m.match_number, *m)).collect();

        // 16 R32 + 8 R16 + 4 QF + 2 SF + 1 Final + 1 third-place
        assert_eq!(matches.len(), 32);
        assert_eq!(matches.iter().filter(|m| m.round == 0).count(), 16);
        assert_eq!(matches.iter().filter(|m| m.round == 4).count(), 1);

        // Third-place playoff is the two beaten semi-finalists.
        let third = by_number[&103];
        assert_eq!(third.left, Slot::Loser(101));
        assert_eq!(third.right, Slot::Loser(102));

        // R32 match 74 keeps its group slot and third-place opponent.
        let m74 = by_number[&74];
        assert_eq!(m74.left, Slot::Group("1E"));
        assert_eq!(m74.right, Slot::ThirdPlace);

        // R16 match 89 is fed by the winners of M74 and M77.
        let m89 = by_number[&89];
        assert_eq!(m89.left, Slot::Winner(74));
        assert_eq!(m89.right, Slot::Winner(77));

        // Final is the two semi-final winners.
        let final_match = by_number[&104];
        assert_eq!(final_match.round, 4);
        assert_eq!(final_match.left, Slot::Winner(101));
        assert_eq!(final_match.right, Slot::Winner(102));
    }

    #[test]
    fn deserializes_group_seed_data_with_team_names_and_codes() {
        let json = r#"[
            {
                "group": "A",
                "teams": [
                    { "name": "Mexico", "code": "MEX" },
                    { "name": "South Africa", "code": "RSA" },
                    { "name": "South Korea", "code": "KOR" },
                    { "name": "Czechia", "code": "CZE" }
                ],
                "third_place_status": "Unknown"
            }
        ]"#;

        let groups: Vec<GroupState> = serde_json::from_str(json).expect("group seed should parse");

        assert_eq!(groups[0].group, 'A');
        assert_eq!(groups[0].teams[0].name, "Mexico");
        assert_eq!(groups[0].teams[0].code, "MEX");
    }
}
