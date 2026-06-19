//! Guided onboarding: arrange groups, then pick the 4 worst 3rd-place teams.

use eframe::egui::{self, Color32, RichText, Sense, Stroke};
use fifa_team3::ThirdPlaceStatus;

use crate::app::PredictorApp;
use crate::standings::flag_image;
use crate::theme::{COLOR_ACCENT, COLOR_GREEN, COLOR_RED, Palette};

/// A small pill: "Step N of 3" plus three progress dots.
fn step_header(ui: &mut egui::Ui, pal: Palette, current: u8, title: &str) {
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(
                COLOR_ACCENT.r(),
                COLOR_ACCENT.g(),
                COLOR_ACCENT.b(),
                36,
            ))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(8, 2))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("Step {current} of 3"))
                        .size(11.0)
                        .strong()
                        .color(COLOR_ACCENT),
                );
            });
        ui.add_space(6.0);
        for s in 1..=3u8 {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(11.0, 11.0), Sense::hover());
            let (color, r) = if s == current {
                (COLOR_ACCENT, 5.0)
            } else if s < current {
                (COLOR_GREEN, 4.0)
            } else {
                (pal.dim, 3.5)
            };
            ui.painter().circle_filled(rect.center(), r, color);
        }
        ui.add_space(8.0);
        ui.label(RichText::new(title).size(15.0).strong().color(pal.text));
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TutorialStep {
    /// Arrange each group's final standings.
    Groups,
    /// Pick the four worst 3rd-place teams (the ones eliminated).
    ThirdPlace,
    /// Guide the user to make their first bracket pick.
    Bracket,
}

/// Render the active tutorial overlay, if any.
pub(crate) fn run(app: &mut PredictorApp, ctx: &egui::Context) {
    match app.tutorial {
        Some(TutorialStep::Groups) => groups_step(app, ctx),
        Some(TutorialStep::ThirdPlace) => third_place_step(app, ctx),
        Some(TutorialStep::Bracket) => bracket_step(app, ctx),
        None => {}
    }
}

/// Called by the bracket when the first pick is made, ending the tutorial.
pub(crate) fn finish(app: &mut PredictorApp) {
    app.tutorial = None;
    app.tutorial_seen = true;
    app.save_state();
}

/// Step 1 — a modal grid of groups; drag teams into final standings order.
fn groups_step(app: &mut PredictorApp, ctx: &egui::Context) {
    let pal = app.pal();

    egui::Area::new(egui::Id::new("tutorial_dim_groups"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .interactable(false)
        .show(ctx, |ui| {
            ui.painter()
                .rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(160));
        });

    egui::Window::new("arrange_groups")
        .order(egui::Order::Foreground)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::new()
                .fill(pal.panel)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(16)),
        )
        .show(ctx, |ui| {
            ui.set_max_width(720.0);
            step_header(ui, pal, 1, "Arrange the groups");
            ui.add_space(4.0);
            ui.label(
                RichText::new("Drag teams within each group into their final standings order.")
                    .size(12.0)
                    .color(pal.dim),
            );
            ui.add_space(8.0);

            let max_grid_h = (ctx.screen_rect().height() - 200.0).max(240.0);
            egui::ScrollArea::vertical()
                .max_height(max_grid_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Grid::new("tutorial_groups_grid")
                        .spacing(egui::vec2(10.0, 10.0))
                        .show(ui, |ui| {
                            for gi in 0..app.groups.len() {
                                group_order_card(ui, app, gi);
                                if (gi + 1) % 4 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("Skip tutorial").size(12.0).color(pal.dim))
                            .fill(pal.card)
                            .stroke(Stroke::new(1.0, pal.border)),
                    )
                    .clicked()
                {
                    finish(app);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Next ▶").size(13.0).color(Color32::WHITE),
                            )
                            .fill(COLOR_ACCENT),
                        )
                        .clicked()
                    {
                        // Finishing the ordering step locks every group as final.
                        for group in &mut app.groups {
                            group.completed = true;
                        }
                        app.tutorial = Some(TutorialStep::ThirdPlace);
                    }
                });
            });
        });
}

/// One group's draggable standings card for the step-1 modal. Uses the shared
/// `dragged` / `drop_target` state, so the move is applied by the app's release
/// handler exactly like the side panel.
fn group_order_card(ui: &mut egui::Ui, app: &mut PredictorApp, gi: usize) {
    let pal = app.pal();
    let gc = app.groups[gi].group;
    let rows: Vec<(String, String)> = app.groups[gi]
        .teams
        .iter()
        .map(|t| (t.code.clone(), t.name.clone()))
        .collect();

    egui::Frame::new()
        .fill(pal.card)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(158.0);
                ui.spacing_mut().item_spacing.y = 7.0;
                ui.label(
                    RichText::new(format!("Group {gc}"))
                        .size(12.0)
                        .strong()
                        .color(pal.dim),
                );
                for (pos, (code, _name)) in rows.iter().enumerate() {
                    let is_source = app.dragged == Some((gi, pos));
                    let resp = ui
                        .horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(
                                RichText::new(format!("{}", pos + 1))
                                    .size(12.0)
                                    .monospace()
                                    .color(pal.dim),
                            );
                            ui.label(RichText::new("⠿").size(13.0).color(pal.dim));
                            flag_image(ui, code, egui::vec2(24.0, 16.0));
                            ui.label(
                                RichText::new(code)
                                    .size(13.0)
                                    .monospace()
                                    .color(if is_source { pal.dim } else { pal.text }),
                            );
                        })
                        .response;
                    let rect = resp.rect;

                    let drag =
                        ui.interact(rect, egui::Id::new(("tut_drag", gi, pos)), Sense::drag());
                    if drag.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                    if drag.drag_started() {
                        app.dragged = Some((gi, pos));
                    }

                    // Record the insertion point and draw a bar while dragging here.
                    if app.dragged.map(|(g, _)| g) == Some(gi)
                        && !is_source
                        && let Some(p) = ui.input(|i| i.pointer.interact_pos())
                        && rect.contains(p)
                    {
                        let above = p.y < rect.center().y;
                        app.drop_target = Some((gi, if above { pos } else { pos + 1 }));
                        let y = if above { rect.top() } else { rect.bottom() };
                        ui.painter()
                            .hline(rect.x_range(), y, Stroke::new(2.0, COLOR_ACCENT));
                    }
                    if is_source {
                        ui.painter()
                            .rect_filled(rect, 0.0, Color32::from_black_alpha(50));
                    }
                }
            });
        });
}

/// Step 2 — a modal to pick the four worst 3rd-place teams.
fn third_place_step(app: &mut PredictorApp, ctx: &egui::Context) {
    let pal = app.pal();

    // Dim the rest of the UI behind the modal.
    egui::Area::new(egui::Id::new("tutorial_dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .interactable(false)
        .show(ctx, |ui| {
            ui.painter()
                .rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(160));
        });

    egui::Window::new("pick_worst_third")
        .order(egui::Order::Foreground)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::new()
                .fill(pal.panel)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(16)),
        )
        .show(ctx, |ui| {
            ui.set_max_width(720.0);
            step_header(ui, pal, 2, "Worst 3rd-place teams");
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "8 of 12 third-place teams advance. Click the 4 worst to eliminate them. \
                     The 3rd-place team in each group is highlighted.",
                )
                .size(12.0)
                .color(pal.dim),
            );
            ui.add_space(8.0);

            // Selection counter as a colored pill.
            let count = app.worst.len();
            let ready = count == 4;
            let pill = if ready { COLOR_GREEN } else { COLOR_ACCENT };
            egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(
                    pill.r(),
                    pill.g(),
                    pill.b(),
                    36,
                ))
                .stroke(Stroke::new(1.0, pill))
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(10, 3))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(if ready {
                            "✓ 4 of 4 selected".to_string()
                        } else {
                            format!("{count} of 4 selected")
                        })
                        .size(12.0)
                        .strong()
                        .color(pill),
                    );
                });
            ui.add_space(8.0);

            // Cap the height so the modal never exceeds the screen (which would
            // clip the lower cards and their flags); scroll if it doesn't fit.
            let max_grid_h = (ctx.screen_rect().height() - 220.0).max(240.0);
            egui::ScrollArea::vertical()
                .max_height(max_grid_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Grid::new("tutorial_third_grid")
                        .spacing(egui::vec2(10.0, 10.0))
                        .show(ui, |ui| {
                            for gi in 0..app.groups.len() {
                                group_preview(ui, app, gi);
                                if (gi + 1) % 4 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("◀ Back").size(12.0).color(pal.text))
                            .fill(pal.card)
                            .stroke(Stroke::new(1.0, pal.border)),
                    )
                    .clicked()
                {
                    app.tutorial = Some(TutorialStep::Groups);
                }
                if ui
                    .add(
                        egui::Button::new(RichText::new("Skip tutorial").size(12.0).color(pal.dim))
                            .fill(pal.card)
                            .stroke(Stroke::new(1.0, pal.border)),
                    )
                    .clicked()
                {
                    finish(app);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let confirm = egui::Button::new(
                        RichText::new("Confirm ▶").size(13.0).color(Color32::WHITE),
                    )
                    .fill(if ready { COLOR_GREEN } else { pal.border });
                    if ui.add_enabled(ready, confirm).clicked() {
                        apply_then_bracket(app);
                    }
                });
            });
        });
}

fn apply_then_bracket(app: &mut PredictorApp) {
    for group in &mut app.groups {
        group.third_place_status = if app.worst.contains(&group.group) {
            ThirdPlaceStatus::Eliminated
        } else {
            ThirdPlaceStatus::Advanced
        };
    }
    // Persist the standings now, then hand off to the final bracket hint.
    app.save_state();
    app.tutorial = Some(TutorialStep::Bracket);
}

/// Step 3 — a banner nudging the first bracket pick; dismissed on that pick.
fn bracket_step(app: &mut PredictorApp, ctx: &egui::Context) {
    let pal = app.pal();
    egui::TopBottomPanel::bottom("tutorial_bracket")
        .exact_height(56.0)
        .frame(
            egui::Frame::new()
                .fill(pal.top_bar)
                .inner_margin(egui::Margin::symmetric(16, 8)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                step_header(ui, pal, 3, "Build your bracket");
                ui.add_space(10.0);
                ui.label(
                    RichText::new(
                        "Click a team in any match to advance it. Your first pick finishes the tour.",
                    )
                    .size(13.0)
                    .color(pal.dim),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Got it").size(12.0).color(pal.dim))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        finish(app);
                    }
                });
            });
        });
}

/// One group's compact card: all four teams as flag + code, the 3rd-place row
/// highlighted (it's the one that may be eliminated).
fn group_preview(ui: &mut egui::Ui, app: &mut PredictorApp, gi: usize) {
    let pal = app.pal();
    let (gc, codes) = {
        let group = &app.groups[gi];
        (
            group.group,
            group
                .teams
                .iter()
                .map(|t| t.code.clone())
                .collect::<Vec<_>>(),
        )
    };
    let selected = app.worst.contains(&gc);

    let fill = if selected {
        Color32::from_rgba_unmultiplied(COLOR_RED.r(), COLOR_RED.g(), COLOR_RED.b(), 28)
    } else {
        pal.card
    };
    let response = egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected { COLOR_RED } else { pal.border },
        ))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(158.0);
                ui.spacing_mut().item_spacing.y = 7.0;
                ui.label(
                    RichText::new(format!("Group {gc}"))
                        .size(12.0)
                        .strong()
                        .color(pal.dim),
                );
                for (i, code) in codes.iter().enumerate() {
                    let is_third = i == 2;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        // The 3rd-place team gets a larger flag and bolder code;
                        // the rest match the step-1 rows.
                        let flag_size = if is_third {
                            egui::vec2(36.0, 24.0)
                        } else {
                            egui::vec2(24.0, 16.0)
                        };
                        flag_image(ui, code, flag_size);
                        let color = if is_third {
                            if selected { COLOR_RED } else { pal.text }
                        } else {
                            pal.dim
                        };
                        let size = if is_third { 16.0 } else { 13.0 };
                        let text = RichText::new(code).size(size).color(color).monospace();
                        ui.label(if is_third { text.strong() } else { text });
                        if is_third {
                            ui.label(RichText::new("3rd").size(10.0).color(pal.dim));
                        }
                    });
                }
            });
        })
        .response
        .interact(Sense::click());

    // Hover ring (when not already selected) for affordance.
    if response.hovered() && !selected {
        ui.painter().rect_stroke(
            response.rect,
            6.0,
            Stroke::new(1.5, COLOR_ACCENT),
            egui::StrokeKind::Inside,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // Corner badge when selected for elimination.
    if selected {
        let c = response.rect.right_top() + egui::vec2(-9.0, 9.0);
        ui.painter().circle_filled(c, 8.0, COLOR_RED);
        ui.painter().text(
            c,
            egui::Align2::CENTER_CENTER,
            "✗",
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
    if response.clicked() {
        if selected {
            app.worst.remove(&gc);
        } else if app.worst.len() < 4 {
            app.worst.insert(gc);
        }
    }
}
