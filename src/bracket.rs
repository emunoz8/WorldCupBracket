//! The interactive knockout bracket: layout, rendering, and winner picking.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use fifa_team3::{
    KoMatch, PredictionReport, ROUND_OF_32_MATCHES, Side, Slot, WinnerPrediction, knockout_matches,
};

use crate::app::PredictorApp;
use crate::theme::{COLOR_GOLD, COLOR_GREEN, COLOR_RED, Palette};

pub(crate) const MATCH_W: f32 = 200.0;
pub(crate) const MATCH_H: f32 = 58.0;
const PAIR_GAP: f32 = 30.0;
const GROUP_GAP: f32 = 34.0;
const QUAD_GAP: f32 = 46.0;
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
                egui::Button::new(RichText::new("Reset all").size(11.0).color(COLOR_RED))
                    .fill(pal.card)
                    .stroke(Stroke::new(1.0, pal.border)),
            )
            .on_hover_text("Clear standings, qualifiers, and all bracket picks")
            .clicked()
        {
            app.reset_all();
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
    let made_pick = !clicks.is_empty();
    for (number, side) in clicks {
        if app.picks.get(&number) == Some(&side) {
            app.picks.remove(&number);
        } else {
            app.picks.insert(number, side);
        }
    }

    // The first bracket interaction ends the onboarding tutorial.
    if made_pick && app.tutorial == Some(crate::tutorial::TutorialStep::Bracket) {
        crate::tutorial::finish(app);
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

    // Venue line beneath the card: stadium · state, country.
    if let Some((stadium, state, country)) = fifa_team3::match_venue(km.match_number) {
        painter.text(
            Pos2::new(top_left.x + 4.0, top_left.y + MATCH_H + 2.0),
            egui::Align2::LEFT_TOP,
            format!("{stadium} · {state}, {country}"),
            egui::FontId::proportional(8.5),
            pal.dim,
        );
    }

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

    let left_label = competitor_label(app, km, Side::Left, index, predictions);
    let right_label = competitor_label(app, km, Side::Right, index, predictions);
    let left_code = competitor_code(app, &left_label);
    let right_code = competitor_code(app, &right_label);

    // R32 entry teams are "confirmed" — solid, not shaded — once their slot is
    // locked: either the source group is manually marked completed, or live data
    // has mathematically clinched the exact position the slot demands. Later
    // rounds are always solid.
    let confirmed = |side: Side| -> bool {
        if km.round != 0 {
            return true;
        }
        if slot_clinched(app, km, side) {
            return true;
        }
        match side_group_char(km, side, predictions) {
            Some(g) => app.groups.iter().any(|gr| gr.group == g && gr.completed),
            None => false,
        }
    };

    draw_competitor_row(
        ui,
        painter,
        top_rect,
        &competitor_tag(km, Side::Left, predictions),
        &left_label,
        left_code.as_deref(),
        confirmed(Side::Left),
        pick == Some(Side::Left),
        is_final,
        pal,
    );
    draw_competitor_row(
        ui,
        painter,
        bot_rect,
        &competitor_tag(km, Side::Right, predictions),
        &right_label,
        right_code.as_deref(),
        confirmed(Side::Right),
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

    // Kebab on the 3rd-place (right) row: a popup listing every team that could
    // land in this slot with its live probability. Swallows the row's pick click
    // so opening the menu never also flips the winner.
    let mut kebab_hit = false;
    if app.live.live_mode
        && km.right == fifa_team3::Slot::ThirdPlace
        && let fifa_team3::Slot::Group(ws) = km.left
    {
        kebab_hit = third_slot_kebab(ui, painter, app, km, ws, bot_rect, pal);
    }
    if bot_response.clicked() && !kebab_hit {
        clicks.push((km.match_number, Side::Right));
    }
}

/// Draws a "⋮" affordance on an R32 third-place row and, when clicked, a popup of
/// every team that could fill the slot with its simulated probability (from
/// `third_slot_pct`). Returns true if the kebab itself was clicked this frame.
fn third_slot_kebab(
    ui: &egui::Ui,
    painter: &egui::Painter,
    app: &PredictorApp,
    km: &KoMatch,
    winner_slot: &str,
    row: Rect,
    pal: Palette,
) -> bool {
    let kebab_rect = Rect::from_center_size(
        Pos2::new(row.max.x - 30.0, row.center().y),
        Vec2::new(14.0, 18.0),
    );
    let resp = ui.interact(
        kebab_rect,
        egui::Id::new(("kebab", km.match_number)),
        Sense::click(),
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.text(
        kebab_rect.center(),
        egui::Align2::CENTER_CENTER,
        "⋮",
        egui::FontId::proportional(15.0),
        if resp.hovered() { pal.text } else { pal.dim },
    );

    let popup_id = egui::Id::new(("kebab_popup", km.match_number));
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    egui::popup::popup_below_widget(
        ui,
        popup_id,
        &resp,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(180.0);
            ui.label(
                RichText::new("Could fill this slot")
                    .strong()
                    .size(11.0)
                    .color(pal.text),
            );
            ui.add_space(2.0);
            let mut rows: Vec<(String, f32)> = app
                .live
                .third_slot_pct
                .get(winner_slot)
                .map(|m| m.iter().map(|(c, p)| (c.clone(), *p)).collect())
                .unwrap_or_default();
            rows.sort_by(|a, b| b.1.total_cmp(&a.1));
            let shown: Vec<&(String, f32)> = rows.iter().filter(|(_, p)| *p >= 0.005).collect();
            if shown.is_empty() {
                ui.label(
                    RichText::new("No live projection yet")
                        .size(10.0)
                        .color(pal.dim),
                );
            }
            for (code, p) in shown {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(
                        RichText::new(format!("{:>3.0}%", p * 100.0))
                            .monospace()
                            .strong()
                            .size(11.0)
                            .color(pal.text),
                    );
                    ui.label(RichText::new(app.team_name(code)).size(11.0).color(pal.text));
                });
            }
        },
    );
    resp.clicked()
}

#[allow(clippy::too_many_arguments)]
fn draw_competitor_row(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: Rect,
    tag: &str,
    team: &str,
    flag_code: Option<&str>,
    confirmed: bool,
    picked: bool,
    is_final: bool,
    pal: Palette,
) {
    let accent = if is_final { COLOR_GOLD } else { COLOR_GREEN };
    let flag_tint = if confirmed {
        Color32::WHITE
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 105)
    };

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

    // Flag (if this competitor resolves to a concrete team), then name.
    let mut name_x = rect.min.x + 40.0;
    if let Some(code) = flag_code
        && let Some(id) = flag_texture(ui.ctx(), code)
    {
        let fr = Rect::from_min_size(
            Pos2::new(rect.min.x + 36.0, rect.center().y - 6.5),
            Vec2::new(20.0, 13.0),
        );
        painter.image(
            id,
            fr,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            flag_tint,
        );
        name_x = rect.min.x + 62.0;
    }

    // Team name (shaded when the source group's result is not yet final).
    let team_color = if picked {
        accent
    } else if confirmed {
        pal.text
    } else {
        pal.dim
    };
    painter.text(
        Pos2::new(name_x, text_y),
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

/// The 3-letter code for a displayed competitor name, when it is a concrete team
/// (strips any trailing "(NN%)"). Returns None for "Winner M.." / "3rd place" etc.
pub(crate) fn competitor_code(app: &PredictorApp, display_name: &str) -> Option<String> {
    let key = display_name
        .split(" (")
        .next()
        .unwrap_or(display_name)
        .trim();
    app.groups
        .iter()
        .flat_map(|g| &g.teams)
        .find(|t| t.name == key)
        .map(|t| t.code.clone())
}

/// True when an R32 Group slot's occupant has mathematically clinched the exact
/// position the slot demands (e.g. slot "1A" → group A's leader is locked in
/// 1st). Third-place slots aren't covered here (they fall back to completion).
fn slot_clinched(app: &PredictorApp, km: &KoMatch, side: Side) -> bool {
    let slot = match side {
        Side::Left => km.left,
        Side::Right => km.right,
    };
    let Slot::Group(s) = slot else { return false };
    let mut ch = s.chars();
    let pos = ch.next().and_then(|c| c.to_digit(10));
    let grp = ch.next();
    let (Some(pos), Some(grp)) = (pos, grp) else {
        return false;
    };
    let Some(group) = app.groups.iter().find(|gr| gr.group == grp) else {
        return false;
    };
    let Some(team) = group.teams.get((pos as usize).saturating_sub(1)) else {
        return false;
    };
    app.live.clinched.get(&team.code) == Some(&pos)
}

/// The source group letter for an R32 competitor (Group slot or predicted 3rd place).
fn side_group_char(
    km: &KoMatch,
    side: Side,
    predictions: &HashMap<&str, &WinnerPrediction>,
) -> Option<char> {
    let slot = match side {
        Side::Left => km.left,
        Side::Right => km.right,
    };
    match slot {
        Slot::Group(s) => s.chars().nth(1),
        Slot::ThirdPlace => competitor_tag(km, side, predictions)
            .chars()
            .nth(1)
            .filter(|c| c.is_ascii_uppercase()),
        _ => None,
    }
}

/// Load (and cache) the embedded flag SVG for a code as a GPU texture id.
fn flag_texture(ctx: &egui::Context, code: &str) -> Option<egui::TextureId> {
    let bytes = crate::flags::flag_png(code)?;
    let uri = format!("bytes://flag/{code}.png");
    ctx.include_bytes(uri.clone(), bytes);
    match ctx.try_load_texture(
        &uri,
        egui::TextureOptions::LINEAR,
        egui::SizeHint::Size(40, 26),
    ) {
        Ok(egui::load::TexturePoll::Ready { texture }) => Some(texture.id),
        _ => None,
    }
}

/// Slot tag for one competitor. For a third-place opponent this resolves to the
/// predicted group, e.g. `3C`, instead of a generic `3rd`.
pub(crate) fn competitor_tag(
    km: &KoMatch,
    side: Side,
    predictions: &HashMap<&str, &WinnerPrediction>,
) -> String {
    let slot = match side {
        Side::Left => km.left,
        Side::Right => km.right,
    };
    if slot == Slot::ThirdPlace {
        if let Slot::Group(s) = km.left {
            return predictions
                .get(s)
                .and_then(|p| p.opponents.first())
                .map(|o| o.opponent.clone())
                .unwrap_or_else(|| "3rd".to_string());
        }
        return "3rd".to_string();
    }
    slot_tag(&slot)
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
                    // In live mode, show P(this team lands in *this* slot) —
                    // simulated through the Annex (sub-100% until decided). Off
                    // live, fall back to the annex option fraction.
                    let pct = app
                        .third_slot_probability(s, &o.opponent)
                        .unwrap_or(o.percentage);
                    format!("{} ({:.0}%)", app.third_place_team(&o.opponent), pct)
                })
                .unwrap_or_else(|| "3rd place TBD".to_string())
        } else {
            "3rd place".to_string()
        }
    } else {
        app.resolve_side(km, side, index, predictions)
    }
}
