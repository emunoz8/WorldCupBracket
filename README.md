# WC26_Bracket

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
- **Guided tutorial** — a 3-step onboarding (arrange groups → pick the 4 worst
  3rd-place teams → make your first bracket pick); replayable from `? Tutorial`.
- **Named saves + import/export** — save brackets under a name, load/delete them,
  or import/export `.json` files anywhere to share with others (`📁 Saves`).
- **Print** — `🖨 Print` opens a landscape bracket + group-standings report in
  your browser (Cmd/Ctrl+P to print), with flags.
- **Live mode** — pull real scores and standings; see below.

## Live Mode

Open `🛰 Live`, optionally paste a [football-data.org] token, and turn on **Live
mode** (auto-polls every 20s). It opens a movable **Live Center** with:

- **Today's games & live scores** (local time) — the in-play match breathes green.
- **Possible final standings** — a what-if projection if current scores hold,
  with ▲/▼ movement arrows and the live game highlighted.
- **3rd-place ranking** — the 12 third-place teams ranked, top 8 advancing.
- **Goal alerts** — bottom-right toasts + a beep when a team scores.

Live scores come from **ESPN's public scoreboard** (no key needed); group
**standings / results** additionally use football-data.org if you provide a free
token (env `FOOTBALL_DATA_TOKEN` or the in-app field). The `🛰 Live` window also
shows an **API log** of every request for debugging. Live data is view-only — it
never overwrites your own bracket/standings, and loading a save turns it off.

## Where State Lives

- **Live save** — written to your OS config directory on first Save:
  - macOS: `~/Library/Application Support/WC26_Bracket/save.json`
  - Windows: `%APPDATA%\WC26_Bracket\save.json`
  - Linux: `~/.config/WC26_Bracket/save.json`
- **Team seed** — the official group draw (names, 3-letter codes, flag emoji)
  is baked into the binary as `SEED_TEAMS` in `src/lib.rs`. Flags are embedded
  flag SVGs (`assets/flags/`, via `include_bytes!`).
- **Load order** — `save.json` if present, otherwise the built-in seed teams.

## Run The Desktop App

```bash
cargo run
```

Opens a window titled **WC26_Bracket**.

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

- macOS/Linux: `target/release/wc26_bracket`
- Windows: `target/release/wc26_bracket.exe`

The binary is self-contained — teams, flags, and the Annex C table are embedded,
so nothing in `data/` is needed at runtime.

### Cross-platform builds

**macOS universal** (Intel + Apple Silicon):

```bash
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
lipo -create -output dist/wc26_bracket \
  target/x86_64-apple-darwin/release/wc26_bracket \
  target/aarch64-apple-darwin/release/wc26_bracket
```

**Windows** (from macOS; needs `brew install mingw-w64`):

```bash
rustup target add x86_64-pc-windows-gnu
CARGO_TARGET_DIR=/tmp/wc26_win cargo build --release --target x86_64-pc-windows-gnu
# → /tmp/wc26_win/x86_64-pc-windows-gnu/release/wc26_bracket.exe
```

Built binaries are written to `dist/` (git-ignored); attach them to a GitHub
Release. Note: the Linux build needs ALSA (`libasound2`) for audio.

## Tests

```bash
cargo test
```

## License

[MIT](LICENSE).

[egui]: https://github.com/emilk/egui
[eframe]: https://crates.io/crates/eframe
[football-data.org]: https://www.football-data.org/
