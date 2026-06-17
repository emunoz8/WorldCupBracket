//! The interactive knockout bracket: layout, rendering, and winner picking.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use fifa_team3::{
    KoMatch, PredictionReport, ROUND_OF_32_MATCHES, Side, Slot, WinnerPrediction, knockout_matches,
};

use crate::app::PredictorApp;
use crate::theme::{COLOR_GOLD, COLOR_GREEN, Palette};

pub(crate) const MATCH_W: f32 = 200.0;
pub(crate) const MATCH_H: f32 = 58.0;
const PAIR_GAP: f32 = 4.0;
const GROUP_GAP: f32 = 26.0;
const QUAD_GAP: f32 = 40.0;
pub(crate) const COL_GAP: f32 = 46.0;
const HEADER_H: f32 = 26.0;
pub(crate) const ROUND_LABELS: [&str; 5] = ["R32", "R16", "QF", "SF", "Final"];

// Bracket layout: 8 matches on left, 8 on right.
// Each side has 2 quads (groups of 4 = 2 pairs).
//
// Left side order (top→bottom): M74, M77 | M73, M75 | M83, M84 | M81, M82
// Right side order (top→bottom): M76, M78 | M79, M80 | M86, M88 | M85, M87

const LEFT_BRACKET: [(usize, usize); 8] = [
    (0, 1),  // M74 index in ROUND_OF_32_MATCHES = 1
    (1, 4),  // M77 = 4
    (2, 0),  // M73 = 0
    (3, 2),  // M75 = 2
    (4, 10), // M83 = 10
    (5, 11), // M84 = 11
    (6, 8),  // M81 = 8
    (7, 9),  // M82 = 9
];

const RIGHT_BRACKET: [(usize, usize); 8] = [
    (0, 3),  // M76 = 3
    (1, 5),  // M78 = 5
    (2, 6),  // M79 = 6
    (3, 7),  // M80 = 7
    (4, 13), // M86 = 13
    (5, 15), // M88 = 15
    (6, 12), // M85 = 12
    (7, 14), // M87 = 14
];

fn match_y(row: usize) -> f32 {
    let quad = row / 4;
    let within_quad = row % 4;
    let pair = within_quad / 2;
    let in_pair = within_quad % 2;

    quad as f32 * (4.0 * MATCH_H + 2.0 * PAIR_GAP + GROUP_GAP + QUAD_GAP)
        + pair as f32 * (2.0 * MATCH_H + PAIR_GAP + GROUP_GAP)
        + in_pair as f32 * (MATCH_H + PAIR_GAP)
}

pub(crate) fn feeder_match(slot: &Slot) -> usize {
    match slot {
        Slot::Winner(n) | Slot::Loser(n) => *n,
        _ => 0,
    }
}

pub(crate) fn slot_tag(slot: &Slot) -> String {
    match slot {
        Slot::Group(s) => (*s).to_string(),
        Slot::ThirdPlace => "3rd".to_string(),
        Slot::Winner(n) => format!("M{n}"),
        Slot::Loser(n) => format!("L{n}"),
    }
}

/// Position every knockout match: returns its top-left x and vertical-centre y.
/// Shared by the on-screen bracket and the printable export.
pub(crate) fn bracket_centers(
    origin: Pos2,
    col_w: f32,
    matches: &[KoMatch],
) -> HashMap<usize, Pos2> {
    let mut centers: HashMap<usize, Pos2> = HashMap::new();
    let mut half: HashMap<usize, i8> = HashMap::new(); // -1 left, 1 right, 0 centre

    for (row, match_idx) in LEFT_BRACKET {
        let number = ROUND_OF_32_MATCHES[match_idx].match_number;
        let y = origin.y + match_y(row) + MATCH_H / 2.0;
        centers.insert(number, Pos2::new(origin.x, y));
        half.insert(number, -1);
    }
    for (row, match_idx) in RIGHT_BRACKET {
        let number = ROUND_OF_32_MATCHES[match_idx].match_number;
        let y = origin.y + match_y(row) + MATCH_H / 2.0;
        centers.insert(number, Pos2::new(origin.x + 8.0 * col_w, y));
        half.insert(number, 1);
    }

    for round in 1..=4u8 {
        for km in matches.iter().filter(|m| m.round == round) {
            let a = feeder_match(&km.left);
            let b = feeder_match(&km.right);
            let ca = centers[&a];
            let cb = centers[&b];
            let h = if half[&a] == half[&b] { half[&a] } else { 0 };
            let col = match h {
                -1 => round as usize,
                1 => 8 - round as usize,
                _ => 4,
            };
            let x = origin.x + col as f32 * col_w;
            let y = (ca.y + cb.y) / 2.0;
            centers.insert(km.match_number, Pos2::new(x, y));
            half.insert(km.match_number, h);
        }
    }

    // Third-place playoff sits just below the Final in the centre column.
    let third_y = centers[&104].y + MATCH_H + 30.0;
    centers.insert(103, Pos2::new(origin.x + 4.0 * col_w, third_y));

    centers
}

pub(crate) fn bracket_view(ui: &mut egui::Ui, app: &mut PredictorApp, report: &PredictionReport) {
    let pal = app.pal();
    let prediction_map: HashMap<&str, &WinnerPrediction> = report
        .predictions
        .iter()
        .map(|p| (p.winner_slot.as_str(), p))
        .collect();

    let matches = knockout_matches();
    let index: HashMap<usize, KoMatch> = matches.iter().map(|m| (m.match_number, *m)).collect();

    let champion = app
        .picks
        .get(&104)
        .map(|&side| app.resolve_side(&index[&104], side, &index, &prediction_map));

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Knockout Bracket")
                .size(16.0)
                .strong()
                .color(pal.text),
        );
        if let Some(team) = &champion {
            ui.label(
                RichText::new(format!("🏆 Champion: {team}"))
                    .size(14.0)
                    .strong()
                    .color(COLOR_GOLD),
            );
        } else {
            ui.label(
                RichText::new("— click a team to advance it")
                    .size(12.0)
                    .color(pal.dim),
            );
        }
        if ui
            .add(
                egui::Button::new(RichText::new("Reset picks").size(11.0).color(pal.dim))
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0, pal.border)),
            )
            .clicked()
        {
            app.picks.clear();
        }
    });
    ui.add_space(8.0);

    let col_w = MATCH_W + COL_GAP;
    let total_w = 8.0 * col_w + MATCH_W;
    let total_h = match_y(8);

    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(total_w.max(ui.available_width()), total_h + HEADER_H + 20.0),
        Sense::hover(),
    );

    let painter = ui.painter_at(rect);
    let origin = rect.min + Vec2::new(0.0, HEADER_H + 10.0);

    // Column headers (R32 … Final … R32, mirrored).
    for col in 0..=8 {
        let depth = if col <= 4 { col } else { 8 - col };
        let cx = origin.x + col as f32 * col_w + MATCH_W / 2.0;
        painter.text(
            Pos2::new(cx, rect.min.y + 4.0),
            egui::Align2::CENTER_TOP,
            ROUND_LABELS[depth],
            egui::FontId::proportional(12.0),
            pal.dim,
        );
    }

    // Each match's position: x = left edge, y = vertical centre.
    let centers = bracket_centers(origin, col_w, &matches);

    // ── Connectors (drawn first, behind the cards) ──
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
            let stroke = Stroke::new(1.5, pal.line);
            painter.line_segment([Pos2::new(fx_edge, fc.y), Pos2::new(mid_x, fc.y)], stroke);
            painter.line_segment([Pos2::new(mid_x, fc.y), Pos2::new(mid_x, child.y)], stroke);
            painter.line_segment(
                [Pos2::new(mid_x, child.y), Pos2::new(cx_edge, child.y)],
                stroke,
            );
        }
    }

    // ── Match cards (collect clicks, apply after the borrow ends) ──
    let mut clicks: Vec<(usize, Side)> = Vec::new();
    for km in &matches {
        let center = centers[&km.match_number];
        let top_left = Pos2::new(center.x, center.y - MATCH_H / 2.0);
        draw_ko_match(
            ui,
            &painter,
            app,
            km,
            top_left,
            &index,
            &prediction_map,
            pal,
            &mut clicks,
        );
    }

    // Clicking the already-winning side again clears the result (undecided).
    for (number, side) in clicks {
        if app.picks.get(&number) == Some(&side) {
            app.picks.remove(&number);
        } else {
            app.picks.insert(number, side);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ko_match(
    ui: &egui::Ui,
    painter: &egui::Painter,
    app: &PredictorApp,
    km: &KoMatch,
    top_left: Pos2,
    index: &HashMap<usize, KoMatch>,
    predictions: &HashMap<&str, &WinnerPrediction>,
    pal: Palette,
    clicks: &mut Vec<(usize, Side)>,
) {
    let rect = Rect::from_min_size(top_left, Vec2::new(MATCH_W, MATCH_H));
    let divider_y = top_left.y + MATCH_H / 2.0;
    let pick = app.picks.get(&km.match_number).copied();
    let is_final = km.round == 4;
    let is_third_place = km.round == 5;

    painter.rect(
        rect,
        5.0,
        pal.card,
        Stroke::new(1.0, pal.border),
        egui::StrokeKind::Middle,
    );

    // Caption above the card.
    let caption = if is_third_place {
        format!("M{} · 3rd place", km.match_number)
    } else {
        format!("M{}", km.match_number)
    };
    painter.text(
        Pos2::new(top_left.x + 4.0, top_left.y - 11.0),
        egui::Align2::LEFT_TOP,
        caption,
        egui::FontId::proportional(10.0),
        pal.dim,
    );

    painter.line_segment(
        [
            Pos2::new(top_left.x + 8.0, divider_y),
            Pos2::new(top_left.x + MATCH_W - 8.0, divider_y),
        ],
        Stroke::new(0.5, pal.border),
    );

    let top_rect = Rect::from_min_size(top_left, Vec2::new(MATCH_W, MATCH_H / 2.0));
    let bot_rect = Rect::from_min_size(
        Pos2::new(top_left.x, divider_y),
        Vec2::new(MATCH_W, MATCH_H / 2.0),
    );

    draw_competitor_row(
        ui,
        painter,
        top_rect,
        &slot_tag(&km.left),
        &competitor_label(app, km, Side::Left, index, predictions),
        pick == Some(Side::Left),
        is_final,
        pal,
    );
    draw_competitor_row(
        ui,
        painter,
        bot_rect,
        &slot_tag(&km.right),
        &competitor_label(app, km, Side::Right, index, predictions),
        pick == Some(Side::Right),
        is_final,
        pal,
    );

    let top_response = ui.interact(
        top_rect,
        egui::Id::new(("ko", km.match_number, 0u8)),
        Sense::click(),
    );
    let bot_response = ui.interact(
        bot_rect,
        egui::Id::new(("ko", km.match_number, 1u8)),
        Sense::click(),
    );
    if top_response.hovered() || bot_response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if top_response.clicked() {
        clicks.push((km.match_number, Side::Left));
    }
    if bot_response.clicked() {
        clicks.push((km.match_number, Side::Right));
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_competitor_row(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: Rect,
    tag: &str,
    team: &str,
    picked: bool,
    is_final: bool,
    pal: Palette,
) {
    let accent = if is_final { COLOR_GOLD } else { COLOR_GREEN };

    if picked {
        painter.rect_filled(
            rect.shrink(1.0),
            3.0,
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 38),
        );
    } else if ui
        .ctx()
        .input(|i| i.pointer.interact_pos().map(|p| rect.contains(p)))
        .unwrap_or(false)
    {
        painter.rect_filled(rect.shrink(1.0), 3.0, pal.hover);
    }

    let text_y = rect.min.y + 6.0;

    // Slot tag.
    painter.text(
        Pos2::new(rect.min.x + 8.0, text_y),
        egui::Align2::LEFT_TOP,
        tag,
        egui::FontId::monospace(10.0),
        pal.dim,
    );

    // Team name.
    let team_color = if picked { accent } else { pal.text };
    painter.text(
        Pos2::new(rect.min.x + 40.0, text_y),
        egui::Align2::LEFT_TOP,
        team,
        egui::FontId::proportional(12.0),
        team_color,
    );

    // Checkbox on the right edge.
    let box_size = 14.0;
    let box_rect = Rect::from_center_size(
        Pos2::new(rect.max.x - 14.0, rect.center().y),
        Vec2::splat(box_size),
    );
    if picked {
        painter.rect_filled(box_rect, 3.0, accent);
        painter.text(
            box_rect.center(),
            egui::Align2::CENTER_CENTER,
            "✓",
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    } else {
        painter.rect(
            box_rect,
            3.0,
            Color32::TRANSPARENT,
            Stroke::new(1.0, pal.border),
            egui::StrokeKind::Middle,
        );
    }
}

/// Display text for one competitor, handling the predicted 3rd-place opponent.
/// Shared by the on-screen bracket and the printable export.
pub(crate) fn competitor_label(
    app: &PredictorApp,
    km: &KoMatch,
    side: Side,
    index: &HashMap<usize, KoMatch>,
    predictions: &HashMap<&str, &WinnerPrediction>,
) -> String {
    let slot = match side {
        Side::Left => km.left,
        Side::Right => km.right,
    };
    if slot == Slot::ThirdPlace {
        if let Slot::Group(s) = km.left {
            predictions
                .get(s)
                .and_then(|p| p.opponents.first())
                .map(|o| {
                    format!(
                        "{} ({:.0}%)",
                        app.third_place_team(&o.opponent),
                        o.percentage
                    )
                })
                .unwrap_or_else(|| "3rd place TBD".to_string())
        } else {
            "3rd place".to_string()
        }
    } else {
        app.resolve_side(km, side, index, predictions)
    }
}
