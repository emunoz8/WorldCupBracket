# Knockout Bracket Predictor

A desktop app (Rust + [egui]/[eframe]) for building out the FIFA World Cup 2026
knockout bracket. Reorder the group standings, see which third-place teams each
group winner could face in the Round of 32, then click your way from the Round
of 32 all the way to the Final.

## Features

- **Standings editor** — drag to reorder each group's four teams; mark each
  group's third-place team as Advanced / Eliminated / Unknown.
- **Annex C predictions** — from the official third-place allocation table, the
  app shows each first-place team's possible third-place opponents (with
  percentages) and narrows the field as you lock in results.
- **Full clickable bracket** — Round of 32 → Round of 16 → Quarter-finals →
  Semi-finals → Final, plus the third-place playoff. Click a team (or its
  checkbox) to advance it; the winner flows into the next round automatically.
  Click the winning side again to clear the result.
- **Certain opponents resolve** — once only one third-place opponent remains
  possible for a slot, later rounds show the real team name instead of
  "3rd place".
- **Hide standings** — collapse the left panel for a full-width bracket view.
- **Save / Reload** — one **Save** snapshots everything (standings order +
  third-place status, theme, panel visibility, and bracket picks); **Reload**
  restores that snapshot, discarding unsaved changes. Nothing is written until
  you press Save, so a cleared bracket can always be reloaded back.
- **Dark / light theme** — toggle from the top bar; saved with everything else.
- **Champion banner** — once the Final is decided, the winner is shown up top.

## Where State Lives

- **Live save** — written to your OS config directory on first Save:
  - macOS: `~/Library/Application Support/Knockout Bracket Predictor/save.json`
  - Windows: `%APPDATA%\Knockout Bracket Predictor\save.json`
  - Linux: `~/.config/Knockout Bracket Predictor/save.json`
- **Team seed** — `data/teams.json` (committed) supplies the groups and teams
  until your first Save exists.
- **Load order** — `save.json` → `data/teams.json` → built-in `A1..L4`
  placeholders if both are missing.

## Run The Desktop App

```bash
cargo run
```

Opens a window titled **Knockout Bracket Predictor**.

### Using it

- Drag the `⠿` handles in the left panel to reorder teams.
- Toggle each group's third-place status with the per-group button or the
  `3rd place:` chips above the bracket.
- Click a team in any match to advance it; click it again to undo.
- **Save** snapshots standings, theme, panel, and bracket picks to your OS
  config dir; **Reload** restores the last saved snapshot.

## Data Flow

```text
data/FWC26_regulations_AnnexC.pdf
  -> data/annex_c_extracted.txt
  -> cargo run --bin generate_annex_json
  -> data/annex_c.json
  -> cargo run
```

## Generate Annex JSON

The generator reads the extracted Annex C table top-down. A line counts as a
table row only when it has an option number followed by eight third-place
entries such as `3E`.

```bash
cargo run --bin generate_annex_json
```

- Default input: `data/annex_c_extracted.txt`
- Default output: `data/annex_c.json`

## Optional Occurrence Report

```bash
cargo run --bin analyze_matches
```

Writes `data/match_occurrences.txt`.

## Build An Executable

```bash
cargo build --release
```

- macOS/Linux: `target/release/fifa-bracket-predictor`
- Windows: `target/release/fifa-bracket-predictor.exe`

## Tests

```bash
cargo test
```

## License

[MIT](LICENSE).

[egui]: https://github.com/emilk/egui
[eframe]: https://crates.io/crates/eframe
