//! Left-panel group standings: info chips, third-place chips, and draggable tables.

use eframe::egui::{self, Color32, Rect, RichText, Sense, Stroke, Vec2};
use fifa_team3::{GroupState, ThirdPlaceStatus};

use crate::app::PredictorApp;
use crate::theme::{COLOR_ACCENT, COLOR_GREEN, COLOR_RED, Palette};

pub(crate) fn info_chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(
            color.r(),
            color.g(),
            color.b(),
            30,
        ))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), 120),
        ))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).color(color));
        });
}

pub(crate) fn third_place_chip(ui: &mut egui::Ui, group: &mut GroupState, pal: Palette) {
    let (text, color) = match group.third_place_status {
        ThirdPlaceStatus::Unknown => (format!("{}-", group.group), pal.dim),
        ThirdPlaceStatus::Advanced => (format!("{}Q", group.group), COLOR_GREEN),
        ThirdPlaceStatus::Eliminated => (format!("{}X", group.group), COLOR_RED),
    };
    if ui
        .add(
            egui::Button::new(RichText::new(text).size(11.0).color(color))
                .fill(pal.card)
                .stroke(Stroke::new(1.0, pal.border))
                .min_size(Vec2::new(30.0, 20.0)),
        )
        .clicked()
    {
        group.third_place_status = group.third_place_status.next();
    }
}

pub(crate) fn group_table(ui: &mut egui::Ui, app: &mut PredictorApp, group_index: usize) {
    let pal = app.pal();
    let group_char = app.groups[group_index].group;
    let status = app.groups[group_index].third_place_status;

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
                        });
                    });
                });

            ui.separator();

            // Team rows
            for position in 0..4 {
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

                if !is_last {
                    ui.add(egui::Separator::default().spacing(0.0));
                }
            }
        });
}

fn standings_row(ui: &mut egui::Ui, app: &mut PredictorApp, group_index: usize, position: usize) {
    let pal = app.pal();
    let is_source = app.dragged == Some((group_index, position));
    let is_drag_in_group = app.dragged.map(|(gi, _)| gi) == Some(group_index);
    let name_color = if is_source { pal.dim } else { pal.text };

    let row_response = ui.horizontal(|ui| {
        // Drag handle
        let (drag_rect, handle) = ui.allocate_exact_size(egui::vec2(16.0, 22.0), Sense::drag());
        let handle_color = if handle.hovered() || handle.dragged() {
            pal.text
        } else {
            pal.dim
        };
        ui.painter().text(
            drag_rect.center(),
            egui::Align2::CENTER_CENTER,
            "⠿",
            egui::FontId::proportional(14.0),
            handle_color,
        );

        if handle.drag_started() {
            app.dragged = Some((group_index, position));
        }

        // Position number
        ui.label(
            RichText::new(format!("{}", position + 1))
                .size(12.0)
                .color(pal.dim)
                .monospace(),
        );
        ui.add_space(6.0);

        // Team name (read-only)
        if let Some(team) = app.groups[group_index].teams.get(position) {
            ui.add_sized(
                [110.0, 22.0],
                egui::Label::new(RichText::new(&team.name).color(name_color).size(13.0)),
            );
            ui.label(RichText::new(&team.code).size(10.0).color(pal.dim));
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

    // Source row: show a dimmed placeholder where the item was lifted from
    if is_source {
        ui.painter()
            .rect_filled(row_rect, 0.0, Color32::from_rgba_unmultiplied(5, 5, 5, 160));
        ui.painter().rect(
            row_rect.shrink(1.5),
            0.0,
            Color32::TRANSPARENT,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(59, 130, 246, 100)),
            egui::StrokeKind::Middle,
        );
    }

    if is_drag_in_group && !is_source {
        let pointer_over = ui.input(|i| {
            i.pointer
                .interact_pos()
                .map(|p| row_rect.contains(p))
                .unwrap_or(false)
        });

        if pointer_over {
            ui.painter().rect_filled(
                row_rect,
                0.0,
                Color32::from_rgba_unmultiplied(59, 130, 246, 55),
            );
            // Left accent bar on the drop target
            ui.painter().rect_filled(
                Rect::from_min_size(row_rect.min, Vec2::new(3.0, row_rect.height())),
                0.0,
                COLOR_ACCENT,
            );

            if ui.input(|i| i.pointer.any_released())
                && let Some((fg, fp)) = app.dragged
                && fg == group_index
            {
                app.groups[group_index].teams.swap(fp, position);
                app.dragged = None;
            }
        }
    }
}
