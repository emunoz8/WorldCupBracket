use std::fs;

use fifa_team3::{Annex, count_matches, format_match_counts};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let json = fs::read_to_string("data/annex_c.json")?;
    let annex: Annex = serde_json::from_str(&json)?;
    let counts = count_matches(&annex);
    let report = format_match_counts(&counts);

    fs::write("data/match_occurrences.txt", report)?;
    println!("Wrote match occurrences to data/match_occurrences.txt");

    Ok(())
}
