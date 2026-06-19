//! Printable HTML export: bracket on a landscape page, standings on the next.

use std::collections::HashMap;

use eframe::egui::Pos2;
use fifa_team3::{
    KoMatch, PredictionReport, Side, ThirdPlaceStatus, WinnerPrediction, knockout_matches,
};

use crate::APP_NAME;
use crate::app::PredictorApp;
use crate::bracket::{
    COL_GAP, MATCH_H, MATCH_W, ROUND_LABELS, bracket_centers, competitor_code, competitor_label,
    competitor_tag, feeder_match,
};

/// Embed a flag SVG as a nested `<svg>` placed at (x, y) with the given size.
/// Injects placement attributes into the flag file's root element.
fn flag_svg_inline(code: &str, x: f32, y: f32, w: f32, h: f32) -> Option<String> {
    let bytes = crate::flags::flag_svg(code)?;
    let raw = std::str::from_utf8(bytes).ok()?;
    let attrs = format!(
        "<svg x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" preserveAspectRatio=\"xMidYMid meet\""
    );
    Some(raw.replacen("<svg", &attrs, 1))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Open a file path with the OS default handler (browser, for our HTML report).
pub(crate) fn open_path(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(target_os = "linux")]
    let mut cmd = std::process::Command::new("xdg-open");

    cmd.arg(path).spawn().map(|_| ())
}

/// Render the bracket (landscape page) and standings (portrait page) as one HTML doc.
pub(crate) fn build_print_html(app: &PredictorApp, report: &PredictionReport) -> String {
    let predictions: HashMap<&str, &WinnerPrediction> = report
        .predictions
        .iter()
        .map(|p| (p.winner_slot.as_str(), p))
        .collect();
    let matches = knockout_matches();
    let index: HashMap<usize, KoMatch> = matches.iter().map(|m| (m.match_number, *m)).collect();

    let col_w = MATCH_W + COL_GAP;
    let origin = Pos2::new(10.0, 34.0);
    let mut centers = bracket_centers(origin, col_w, &matches);
    let total_w = origin.x + 8.0 * col_w + MATCH_W + 10.0;

    // Stretch vertically so the wide-but-short bracket fills the landscape page
    // (A4 landscape usable area is ~1.45:1) instead of leaving white space below.
    let raw_h = centers.values().map(|p| p.y).fold(0.0_f32, f32::max) + MATCH_H + 20.0;
    let target_h = total_w / 1.45;
    let scale = ((target_h - origin.y) / (raw_h - origin.y)).max(1.0);
    if scale > 1.0 {
        for p in centers.values_mut() {
            p.y = origin.y + (p.y - origin.y) * scale;
        }
    }
    let max_y = centers.values().map(|p| p.y).fold(0.0_f32, f32::max) + MATCH_H + 20.0;

    const LINE: &str = "#999";
    const BORDER: &str = "#cccccc";
    const TEXT: &str = "#111";
    const DIM: &str = "#666";
    const GREEN: &str = "#16a34a";
    const GOLD: &str = "#b8860b";

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg viewBox=\"0 0 {total_w:.0} {max_y:.0}\" xmlns=\"http://www.w3.org/2000/svg\" font-family=\"Arial, Helvetica, sans-serif\">"
    ));

    // Round headers.
    for col in 0..=8 {
        let depth = if col <= 4 { col } else { 8 - col };
        let cx = origin.x + col as f32 * col_w + MATCH_W / 2.0;
        svg.push_str(&format!(
            "<text x=\"{cx:.0}\" y=\"20\" text-anchor=\"middle\" font-size=\"12\" fill=\"{DIM}\">{}</text>",
            ROUND_LABELS[depth]
        ));
    }

    // Connectors.
    for km in matches.iter().filter(|m| m.round >= 1) {
        let child = centers[&km.match_number];
        for feeder in [feeder_match(&km.left), feeder_match(&km.right)] {
            let fc = centers[&feeder];
            let (fx_edge, cx_edge) = if fc.x < child.x {
                (fc.x + MATCH_W, child.x)
            } else {
                (fc.x, child.x + MATCH_W)
            };
            let mid_x = (fx_edge + cx_edge) / 2.0;
            svg.push_str(&format!(
                "<polyline points=\"{fx_edge:.0},{0:.0} {mid_x:.0},{0:.0} {mid_x:.0},{1:.0} {cx_edge:.0},{1:.0}\" fill=\"none\" stroke=\"{LINE}\" stroke-width=\"1.5\"/>",
                fc.y, child.y
            ));
        }
    }

    // Match cards.
    for km in &matches {
        let c = centers[&km.match_number];
        let x = c.x;
        let y = c.y - MATCH_H / 2.0;
        let mid = c.y;
        let pick = app.picks.get(&km.match_number).copied();
        let is_final = km.round == 4;
        let accent = if is_final { GOLD } else { GREEN };

        let caption = if km.round == 5 {
            format!("M{} · 3rd place", km.match_number)
        } else {
            format!("M{}", km.match_number)
        };
        svg.push_str(&format!(
            "<text x=\"{:.0}\" y=\"{:.0}\" font-size=\"9\" fill=\"{DIM}\">{}</text>",
            x + 2.0,
            y - 3.0,
            html_escape(&caption)
        ));
        svg.push_str(&format!(
            "<rect x=\"{x:.0}\" y=\"{y:.0}\" width=\"{MATCH_W:.0}\" height=\"{MATCH_H:.0}\" rx=\"5\" fill=\"#fff\" stroke=\"{BORDER}\"/>"
        ));
        svg.push_str(&format!(
            "<line x1=\"{:.0}\" y1=\"{mid:.0}\" x2=\"{:.0}\" y2=\"{mid:.0}\" stroke=\"{BORDER}\" stroke-width=\"0.5\"/>",
            x + 8.0,
            x + MATCH_W - 8.0
        ));

        for (side, row_top, this_pick) in [
            (Side::Left, y, pick == Some(Side::Left)),
            (Side::Right, mid, pick == Some(Side::Right)),
        ] {
            if this_pick {
                svg.push_str(&format!(
                    "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" fill=\"{accent}\" fill-opacity=\"0.16\"/>",
                    x + 1.0,
                    row_top + 1.0,
                    MATCH_W - 2.0,
                    MATCH_H / 2.0 - 2.0
                ));
            }
            let tag = competitor_tag(km, side, &predictions);
            let name = competitor_label(app, km, side, &index, &predictions);
            let name_fill = if this_pick { accent } else { TEXT };
            let weight = if this_pick { "bold" } else { "normal" };
            let ty = row_top + 18.0;
            svg.push_str(&format!(
                "<text x=\"{:.0}\" y=\"{ty:.0}\" font-size=\"10\" font-family=\"monospace\" fill=\"{DIM}\">{}</text>",
                x + 8.0,
                html_escape(&tag)
            ));
            // Flag for concrete teams, then shift the name right to make room.
            let mut name_x = x + 40.0;
            if let Some(code) = competitor_code(app, &name)
                && let Some(flag) =
                    flag_svg_inline(&code, x + 34.0, row_top + MATCH_H / 4.0 - 6.5, 18.0, 13.0)
            {
                svg.push_str(&flag);
                name_x = x + 58.0;
            }
            svg.push_str(&format!(
                "<text x=\"{name_x:.0}\" y=\"{ty:.0}\" font-size=\"12\" font-weight=\"{weight}\" fill=\"{name_fill}\">{}</text>",
                html_escape(&name)
            ));
            // Checkbox.
            let bx = x + MATCH_W - 21.0;
            let by = row_top + MATCH_H / 4.0 - 7.0;
            if this_pick {
                svg.push_str(&format!(
                    "<rect x=\"{bx:.0}\" y=\"{by:.0}\" width=\"14\" height=\"14\" rx=\"3\" fill=\"{accent}\"/><text x=\"{:.0}\" y=\"{:.0}\" font-size=\"11\" text-anchor=\"middle\" fill=\"#fff\">✓</text>",
                    bx + 7.0,
                    by + 11.0
                ));
            } else {
                svg.push_str(&format!(
                    "<rect x=\"{bx:.0}\" y=\"{by:.0}\" width=\"14\" height=\"14\" rx=\"3\" fill=\"none\" stroke=\"{BORDER}\"/>"
                ));
            }
        }
    }
    svg.push_str("</svg>");

    // Champion heading.
    let champion = app
        .picks
        .get(&104)
        .map(|&side| app.resolve_side(&index[&104], side, &index, &predictions))
        .map(|team| format!("<p class=\"champ\">🏆 Champion: {}</p>", html_escape(&team)))
        .unwrap_or_default();

    // Standings tables. With live data, add Pld/Pts/GF/GA/GD columns.
    let live = !app.live_standings.is_empty();
    let mut tables = String::new();
    for group in &app.groups {
        let colspan = if live { 8 } else { 3 };
        tables.push_str(&format!(
            "<table><thead><tr><th colspan=\"{colspan}\">Group {}</th></tr>",
            group.group
        ));
        if live {
            tables.push_str(
                "<tr class=\"sub\"><th></th><th>Team</th><th>Pld</th><th>Pts</th>\
                 <th>GF</th><th>GA</th><th>GD</th><th></th></tr>",
            );
        }
        tables.push_str("<tbody>");
        for (pos, team) in group.teams.iter().enumerate() {
            let (marker, cls) = match pos {
                0 | 1 => ("Q", "q"),
                2 => match group.third_place_status {
                    ThirdPlaceStatus::Unknown => ("–", "dim"),
                    ThirdPlaceStatus::Advanced => ("Q", "q"),
                    ThirdPlaceStatus::Eliminated => ("X", "x"),
                },
                _ => ("X", "x"),
            };
            let flag = if team.flag.is_empty() {
                String::new()
            } else {
                format!("{} ", team.flag)
            };
            let stats = if live {
                match app.live_stats(group.group, &team.code) {
                    Some(t) => format!(
                        "<td class=\"n\">{}</td><td class=\"n b\">{}</td><td class=\"n\">{}</td>\
                         <td class=\"n\">{}</td><td class=\"n\">{:+}</td>",
                        t.played, t.points, t.goals_for, t.goals_against, t.goal_diff
                    ),
                    None => "<td class=\"n\"></td><td class=\"n\"></td><td class=\"n\"></td>\
                             <td class=\"n\"></td><td class=\"n\"></td>"
                        .to_string(),
                }
            } else {
                String::new()
            };
            tables.push_str(&format!(
                "<tr><td class=\"pos\">{}</td><td>{flag}{} <span class=\"code\">{}</span></td>{stats}<td class=\"{cls}\">{marker}</td></tr>",
                pos + 1,
                html_escape(&team.name),
                html_escape(&team.code)
            ));
        }
        tables.push_str("</tbody></table>");
    }

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{APP_NAME}</title><style>\
@page {{ size: A4 landscape; margin: 8mm; }}\
* {{ box-sizing: border-box; }}\
body {{ font-family: Arial, Helvetica, sans-serif; color: #111; margin: 0; }}\
h1 {{ font-size: 18px; margin: 0 0 6px; }}\
.champ {{ color: {GOLD}; font-weight: bold; font-size: 15px; margin: 2px 0 8px; }}\
.bracket {{ page-break-after: always; }}\
.standings {{ page-break-before: always; padding-top: 4px; }}\
.standings h1 {{ font-size: 16px; margin: 0 0 4px; }}\
svg {{ display: block; width: auto; height: 172mm; max-width: 100%; margin: 0 auto; }}\
.grid {{ display: grid; grid-template-columns: repeat(4, 1fr); grid-template-rows: repeat(3, auto); gap: 8px 14px; align-content: start; }}\
table {{ border-collapse: collapse; width: 100%; font-size: 13px; page-break-inside: avoid; break-inside: avoid; }}\
th {{ background: #f0f0f0; text-align: left; padding: 5px 8px; font-size: 13px; }}\
td {{ border-top: 1px solid #ddd; padding: 5px 8px; }}\
.pos {{ color: #888; width: 22px; }}\
.code {{ color: #888; font-size: 12px; }}\
.sub th {{ background: #f7f7f7; color: #777; font-size: 10px; padding: 3px 6px; text-align: center; }}\
.sub th:nth-child(2) {{ text-align: left; }}\
.n {{ text-align: center; font-variant-numeric: tabular-nums; padding: 5px 6px; }}\
.b {{ font-weight: bold; }}\
.q {{ color: {GREEN}; font-weight: bold; text-align: center; }}\
.x {{ color: #dc2626; font-weight: bold; text-align: center; }}\
.dim {{ color: #888; text-align: center; }}\
</style></head><body>\
<div class=\"bracket\"><h1>Knockout Bracket</h1>{champion}{svg}</div>\
<div class=\"standings\"><h1>Group Standings</h1><div class=\"grid\">{tables}</div></div>\
</body></html>"
    )
}
