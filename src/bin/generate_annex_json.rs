use std::{env, fs};

use fifa_team3::annex_from_table_text;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input_path = args
        .next()
        .unwrap_or_else(|| "data/annex_c_extracted.txt".to_string());
    let output_path = args
        .next()
        .unwrap_or_else(|| "data/annex_c.json".to_string());

    if args.next().is_some() {
        return Err(
            "Usage: cargo run --bin generate_annex_json -- [input.txt] [output.json]".into(),
        );
    }

    let text = fs::read_to_string(&input_path)?;
    let annex = annex_from_table_text(&text)?;
    let json = serde_json::to_string_pretty(&annex)?;

    fs::write(&output_path, format!("{json}\n"))?;
    println!("Wrote {} scenarios to {output_path}", annex.len());

    Ok(())
}
