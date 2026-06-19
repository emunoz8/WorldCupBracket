//! Left-panel group standings: info chips, third-place chips, and draggable tables.

use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};
use fifa_team3::{GroupState, ThirdPlaceStatus};

use crate::app::PredictorApp;
use crate::theme::{COLOR_ACCENT, COLOR_GREEN, COLOR_RED, Palette};

pub(crate) fn info_chip(ui: &mut egui::Ui, text: &str, color: Color32, pal: Palette) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            40,
        ))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            // High-contrast label; the color reads from the fill + border instead.
            ui.label(RichText::new(text).size(12.0).strong().color(pal.text));
        });
}

pub(crate) fn third_place_chip(ui: &mut egui::Ui, group: &mut GroupState, pal: Palette) {
    let (symbol, color) = match group.third_place_status {
        ThirdPlaceStatus::Unknown => ("–", pal.dim),
        ThirdPlaceStatus::Advanced => ("Q", COLOR_GREEN),
        ThirdPlaceStatus::Eliminated => ("X", COLOR_RED),
    };
    // The 3rd-place team is whoever currently sits in row 3 of the group.
    let third = group.teams.get(2);
    let code = third.map(|t| t.code.clone()).unwrap_or_default();

    let response = egui::Frame::new()
        .fill(pal.card)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // Group of origin.
                ui.label(
                    RichText::new(group.group.to_string())
                        .size(11.0)
                        .strong()
                        .color(pal.dim),
                );
                flag_image(ui, &code, Vec2::new(18.0, 12.0));
                ui.label(RichText::new(&code).size(11.0).color(pal.text));
                ui.label(RichText::new(symbol).size(11.0).strong().color(color));
            });
        })
        .response
        .interact(Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        group.third_place_status = group.third_place_status.next();
    }
}

pub(crate) fn group_table(ui: &mut egui::Ui, app: &mut PredictorApp, group_index: usize) {
    let pal = app.pal();
    let group_char = app.groups[group_index].group;
    let status = app.groups[group_index].third_place_status;
    let completed = app.groups[group_index].completed;

    egui::Frame::new()
        .fill(pal.card)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width() - 24.0);

            // Header row
            egui::Frame::new()
                .fill(pal.header)
                .corner_radius(egui::CornerRadius {
                    nw: 6,
                    ne: 6,
                    sw: 0,
                    se: 0,
                })
                .inner_margin(egui::Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Group {group_char}"))
                                .strong()
                                .color(pal.text)
                                .size(13.0),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(status.label()).size(10.0).color(
                                            match status {
                                                ThirdPlaceStatus::Unknown => pal.dim,
                                                ThirdPlaceStatus::Advanced => COLOR_GREEN,
                                                ThirdPlaceStatus::Eliminated => COLOR_RED,
                                            },
                                        ),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, pal.border)),
                                )
                                .clicked()
                            {
                                app.groups[group_index].third_place_status = status.next();
                            }

                            // Mark the group's standings as final (drives bracket shading).
                            let (label, color) = if completed {
                                ("✓ Complete", COLOR_GREEN)
                            } else {
                                ("○ Pending", pal.dim)
                            };
                            if ui
                                .add(
                                    egui::Button::new(RichText::new(label).size(10.0).color(color))
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::new(1.0, pal.border)),
                                )
                                .on_hover_text("Toggle whether this group's result is final")
                                .clicked()
                            {
                                app.groups[group_index].completed = !completed;
                            }
                        });
                    });
                });

            ui.separator();

            // Legend for the live stat column (only when live data is loaded).
            if !app.live_standings.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("stats: Pts · GF:GA · GD")
                            .size(9.0)
                            .italics()
                            .color(pal.dim),
                    );
                });
            }

            // While dragging within this group, the dragged row collapses and a
            // gap opens at the hovered slot so you can see where it will land.
            let drag_here = matches!(app.dragged, Some((gi, _)) if gi == group_index);
            let source_pos = if drag_here {
                app.dragged.map(|(_, p)| p)
            } else {
                None
            };
            let target_pos = if drag_here {
                app.drop_target
                    .and_then(|(gi, p)| (gi == group_index).then_some(p))
            } else {
                None
            };

            // Team rows
            for position in 0..4 {
                if target_pos == Some(position) {
                    gap_placeholder(ui, app);
                }
                if source_pos == Some(position) {
                    continue; // collapse the row being dragged
                }

                let bg = if position % 2 == 0 {
                    pal.row
                } else {
                    pal.row_alt
                };
                let is_last = position == 3;
                let corner = if is_last {
                    egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 6,
                        se: 6,
                    }
                } else {
                    egui::CornerRadius::ZERO
                };

                egui::Frame::new()
                    .fill(bg)
                    .corner_radius(corner)
                    .inner_margin(egui::Margin::symmetric(12, 5))
                    .show(ui, |ui| {
                        standings_row(ui, app, group_index, position);
                    });

                if !is_last && !drag_here {
                    ui.add(egui::Separator::default().spacing(0.0));
                }
            }

            // Dropping below the last team opens a gap at the very bottom (4th).
            if target_pos == Some(4) {
                gap_placeholder(ui, app);
            }
        });
}

/// Draw a team's flag (embedded SVG) at the given size, or reserve the width.
pub(crate) fn flag_image(ui: &mut egui::Ui, code: &str, size: Vec2) {
    if let Some(bytes) = crate::flags::flag_svg(code) {
        ui.add(
            egui::Image::from_bytes(format!("bytes://flag/{code}.svg"), bytes)
                .fit_to_exact_size(size),
        );
    } else {
        ui.add_space(size.x);
    }
}

/// An empty, outlined slot showing where the dragged team will drop.
fn gap_placeholder(ui: &mut egui::Ui, app: &PredictorApp) {
    let name = app
        .dragged
        .and_then(|(g, p)| app.groups[g].teams.get(p))
        .map(|t| t.name.clone());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), Sense::hover());
    let slot = rect.shrink(3.0);
    ui.painter().rect(
        slot,
        4.0,
        Color32::from_rgba_unmultiplied(COLOR_ACCENT.r(), COLOR_ACCENT.g(), COLOR_ACCENT.b(), 30),
        Stroke::new(1.5, COLOR_ACCENT),
        egui::StrokeKind::Middle,
    );
    if let Some(name) = name {
        ui.painter().text(
            slot.center(),
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::proportional(12.0),
            COLOR_ACCENT,
        );
    }
}

fn standings_row(ui: &mut egui::Ui, app: &mut PredictorApp, group_index: usize, position: usize) {
    let pal = app.pal();
    let is_source = app.dragged == Some((group_index, position));
    let is_drag_in_group = app.dragged.map(|(gi, _)| gi) == Some(group_index);
    let name_color = if is_source { pal.dim } else { pal.text };

    // Live stats (points, GF, GA, GD) for this team, if a sync has happened.
    let group_char = app.groups[group_index].group;
    let stats = app.groups[group_index]
        .teams
        .get(position)
        .and_then(|t| app.live_stats(group_char, &t.code))
        .map(|t| (t.points, t.goals_for, t.goals_against, t.goal_diff));

    let row_response = ui.horizontal(|ui| {
        // Drag-handle glyph (purely visual; the whole row is the drag target).
        let (drag_rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 22.0), Sense::hover());
        ui.painter().text(
            drag_rect.center(),
            egui::Align2::CENTER_CENTER,
            "⠿",
            egui::FontId::proportional(14.0),
            if is_source { pal.text } else { pal.dim },
        );

        // Position number
        ui.label(
            RichText::new(format!("{}", position + 1))
                .size(12.0)
                .color(pal.dim)
                .monospace(),
        );
        ui.add_space(6.0);

        // Flag + team name (read-only)
        if let Some(team) = app.groups[group_index].teams.get(position) {
            flag_image(ui, &team.code, egui::vec2(22.0, 15.0));
            ui.add_space(6.0);
            ui.add_sized(
                [104.0, 22.0],
                egui::Label::new(RichText::new(&team.name).color(name_color).size(13.0)),
            );
            ui.label(RichText::new(&team.code).size(10.0).color(pal.dim));
        }

        // Live stats: points, goals for:against, goal difference.
        if let Some((pts, gf, ga, gd)) = stats {
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("{pts}p {gf}:{ga} {gd:+}"))
                    .size(9.5)
                    .monospace()
                    .color(pal.dim),
            );
        }

        // Qualification marker
        let (marker, color) = match position {
            0 | 1 => ("Q", COLOR_GREEN),
            2 => match app.groups[group_index].third_place_status {
                ThirdPlaceStatus::Unknown => ("-", pal.dim),
                ThirdPlaceStatus::Advanced => ("Q", COLOR_GREEN),
                ThirdPlaceStatus::Eliminated => ("X", COLOR_RED),
            },
            _ => ("X", COLOR_RED),
        };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(marker).size(11.0).color(color).strong());
        });
    });

    let row_rect = row_response.response.rect;

    // The whole row is draggable, not just the handle.
    let row_drag = ui.interact(
        row_rect,
        egui::Id::new(("std_row", group_index, position)),
        Sense::drag(),
    );
    if row_drag.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if row_drag.drag_started() {
        app.dragged = Some((group_index, position));
    }

    // While dragging within this group, record the insertion index under the
    // pointer (0..=4) so the table opens a gap there. Hovering a row's lower half
    // targets the slot below it, so dropping past the last team lands in 4th.
    if is_drag_in_group
        && !is_source
        && let Some(p) = ui.input(|i| i.pointer.interact_pos())
        && row_rect.contains(p)
    {
        let insert = if p.y < row_rect.center().y {
            position
        } else {
            position + 1
        };
        app.drop_target = Some((group_index, insert));
    }
}
