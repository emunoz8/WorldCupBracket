//! Live-mode UI: toasts, the Live Center, and the data window.

use std::time::Duration;

use eframe::egui::{self, Color32, RichText, Sense, Stroke};

use crate::app::PredictorApp;
use crate::standings::flag_image;
use crate::theme::{COLOR_ACCENT, COLOR_GREEN, COLOR_RED, Palette};

use super::*;

/// Render the stack of live-mode notifications in the bottom-right corner.
pub(crate) fn toasts_overlay(app: &mut PredictorApp, ctx: &egui::Context) {
    app.live
        .toasts
        .retain(|t| t.created.elapsed().as_secs_f32() < 8.0);
    if app.live.toasts.is_empty() {
        return;
    }
    ctx.request_repaint_after(Duration::from_millis(500));
    let pal = app.pal();

    egui::Area::new(egui::Id::new("live_toasts"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
        .show(ctx, |ui| {
            ui.set_max_width(300.0);
            for toast in app.live.toasts.iter().rev().take(6) {
                let accent = match toast.kind {
                    AlertKind::Up => COLOR_GREEN,
                    AlertKind::Down => COLOR_RED,
                    AlertKind::Info => COLOR_ACCENT,
                };
                egui::Frame::new()
                    .fill(pal.card)
                    .stroke(Stroke::new(1.5, accent))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.set_min_width(220.0);
                        ui.label(RichText::new(&toast.text).size(12.0).color(pal.text));
                    });
                ui.add_space(6.0);
            }
        });
}

/// Bottom-center window listing today's fixtures with live/final scores.
/// One movable, collapsible Live Center: today's games on top, a 4×3 grid of
/// projected group tables on the left, and the 3rd-place ranking on the right.
pub(crate) fn live_center_window(app: &mut PredictorApp, ctx: &egui::Context) {
    if !app.live.show_live_center {
        return;
    }
    let pal = app.pal();
    let fixtures = app.live.today_fixtures.clone();
    // Standings are finished-only; this grid is the in-play "what-if" projection,
    // with arrows comparing the projection back to the finished standings.
    let proj = project_standings(
        &app.live.live_standings,
        &app.live.today_fixtures,
        &app.live.remaining,
    );
    // Rank the 3rd-place teams off the *projection* so the table tracks in-play
    // scores (matching the "possible final standings" grid), not just finished games.
    let proj_third = third_place_ranking(&proj);
    let feed: HashMap<String, u32> = app
        .live
        .live_standings
        .iter()
        .flat_map(|s| s.teams.iter().map(|t| (t.code.clone(), t.position)))
        .collect();
    // Codes of teams currently in a live fixture (for the breathing highlight).
    let playing: std::collections::HashSet<String> = fixtures
        .iter()
        .filter(|f| f.status.is_live())
        .flat_map(|f| [f.home_code.clone(), f.away_code.clone()])
        .collect();
    let mut open = true;
    let screen = ctx.screen_rect();

    egui::Window::new(RichText::new("Live center").size(14.0))
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_pos(egui::pos2((screen.center().x - 540.0).max(20.0), 60.0))
        .default_size(egui::vec2(1060.0, (screen.height() - 120.0).max(420.0)))
        .frame(
            egui::Frame::new()
                .fill(pal.panel)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(12)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(1000.0);
            egui::ScrollArea::vertical()
                .max_height((screen.height() - 150.0).max(320.0))
                .show(ui, |ui| {
                    // ── Today's games (with live scores) ──
                    if !fixtures.is_empty() {
                        ui.label(
                            RichText::new("Today's games & live scores (local time)")
                                .size(12.0)
                                .strong()
                                .color(pal.dim),
                        );
                        ui.add_space(2.0);
                        for f in &fixtures {
                            fixture_row(ui, f, pal);
                        }
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);
                    }

                    if proj.is_empty() {
                        let msg = if app.live.live_rx.is_some() {
                            "Syncing live data…"
                        } else {
                            "Sync live data to see possible standings."
                        };
                        ui.label(RichText::new(msg).size(12.0).color(pal.dim));
                        return;
                    }

                    // ── Projected group tables (left) + 3rd-place race (right) ──
                    ui.label(
                        RichText::new("Possible final standings (if live scores hold)")
                            .size(12.0)
                            .strong()
                            .color(pal.dim),
                    );
                    ui.add_space(4.0);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            egui::Grid::new("live_center_groups")
                                .spacing(egui::vec2(8.0, 8.0))
                                .show(ui, |ui| {
                                    for (i, s) in proj.iter().enumerate() {
                                        projected_group_card(ui, s, &feed, &playing, pal);
                                        if (i + 1) % 4 == 0 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        });
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("3rd-place ranking — top 8 advance")
                                    .size(12.0)
                                    .strong()
                                    .color(pal.text),
                            );
                            ui.add_space(4.0);
                            third_place_table(ui, app, &proj_third, pal);
                        });
                    });

                    // ── "What needs to happen" — per-group qualification scenarios ──
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);
                    scenarios_section(ui, app, pal);
                });
        });

    if !open {
        app.live.show_live_center = false;
    }
}

/// Per-group "what needs to happen". Groups start collapsed; expanding one
/// computes its scenarios on demand (cached until the next sync), so opening the
/// Live Center never brute-forces all 12 groups up front.
fn scenarios_section(ui: &mut egui::Ui, app: &mut PredictorApp, pal: Palette) {
    ui.label(
        RichText::new("What needs to happen — expand a group")
            .size(12.0)
            .strong()
            .color(pal.dim),
    );
    ui.add_space(4.0);

    let groups: Vec<char> = app.live.live_standings.iter().map(|s| s.group).collect();
    for group in groups {
        egui::CollapsingHeader::new(
            RichText::new(format!("Group {group}"))
                .size(12.5)
                .strong()
                .color(pal.text),
        )
        .id_salt(("scenarios", group))
        .default_open(false)
        .show(ui, |ui| {
            // Compute (and cache) only when the group is actually expanded.
            if !app.live.scenario_cache.contains_key(&group) {
                let teams = app
                    .live
                    .live_standings
                    .iter()
                    .find(|s| s.group == group)
                    .map(|s| s.teams.clone());
                if let Some(teams) = teams {
                    let scen = crate::live::group_scenarios(&teams, &app.live.remaining);
                    app.live.scenario_cache.insert(group, scen);
                }
            }
            match app.live.scenario_cache.get(&group) {
                Some(crate::live::GroupScenarios::Done) => {
                    ui.label(
                        RichText::new("Group complete — positions are final.")
                            .size(11.0)
                            .italics()
                            .color(pal.dim),
                    );
                }
                Some(crate::live::GroupScenarios::TooEarly(n)) => {
                    ui.label(
                        RichText::new(format!(
                            "{n} matches left — scenarios appear once the group nears its last round."
                        ))
                        .size(11.0)
                        .italics()
                        .color(pal.dim),
                    );
                }
                Some(crate::live::GroupScenarios::Ready(teams)) => {
                    for t in teams.iter() {
                        scenario_team_row(ui, t, pal);
                    }
                }
                None => {}
            }
        });
    }
}

/// One team's qualification outlook: reachable finishes + per-result branches.
fn scenario_team_row(ui: &mut egui::Ui, t: &crate::live::TeamScenario, pal: Palette) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        flag_image(ui, &t.code, egui::vec2(20.0, 13.0));
        ui.add_sized(
            [150.0, 16.0],
            egui::Label::new(RichText::new(&t.name).size(12.5).strong().color(pal.text)),
        );
        let reach = t
            .possible
            .iter()
            .map(|p| super::sync::ordinal(*p))
            .collect::<Vec<_>>()
            .join(", ");
        let (txt, col) = if t.possible == [1] {
            ("clinched 1st".to_string(), COLOR_GREEN)
        } else if t.possible.iter().all(|p| *p <= 2) {
            (format!("can finish {reach} — qualified"), COLOR_GREEN)
        } else if t.possible.iter().all(|p| *p > 2) {
            (format!("can finish {reach}"), COLOR_RED)
        } else {
            (format!("can finish {reach}"), pal.dim)
        };
        ui.label(RichText::new(txt).size(11.0).color(col));
    });
    let fin = |best: u32, worst: u32| {
        if best == worst {
            super::sync::ordinal(best)
        } else {
            format!("{}-{}", super::sync::ordinal(best), super::sync::ordinal(worst))
        }
    };
    let col_for = |best: u32, worst: u32| {
        if worst <= 2 {
            COLOR_GREEN
        } else if best > 2 {
            COLOR_RED
        } else {
            pal.text
        }
    };
    // One monospace tree line, indented under the team header.
    let line = |ui: &mut egui::Ui, text: String, color: Color32| {
        ui.horizontal(|ui| {
            ui.add_space(30.0);
            ui.label(RichText::new(text).monospace().size(11.0).color(color));
        });
    };

    let nb = t.branches.len();
    for (bi, b) in t.branches.iter().enumerate() {
        let col = col_for(b.best, b.worst);
        let blast = bi + 1 == nb;
        let bstem = if blast { "`-" } else { "+-" };
        let bcont = if blast { "   " } else { "|  " };
        // Independent: this result fixes the finish no matter the other games.
        let independent = b.best == b.worst
            && b.conditions
                .iter()
                .all(|c| c.best == b.best && c.worst == b.worst && c.detail.is_empty());

        let mut head = format!("{bstem} {:<5}-> {}", b.own.label(), fin(b.best, b.worst));
        if independent {
            head.push_str("   (no other results matter)");
        }
        line(ui, head, col);
        if independent {
            continue;
        }

        let nc = b.conditions.len();
        for (ci, c) in b.conditions.iter().enumerate() {
            let ccol = col_for(c.best, c.worst);
            let clast = ci + 1 == nc;
            let cstem = if clast { "`-" } else { "+-" };
            let ccont = if clast { "   " } else { "|  " };
            let cond = if c.other.is_empty() {
                format!("{bcont}{cstem} -> {}", fin(c.best, c.worst))
            } else {
                format!("{bcont}{cstem} {:<34}-> {}", c.other, fin(c.best, c.worst))
            };
            line(ui, cond, ccol);
            // When a grid is shown it tells the margin story — skip the text line.
            if let Some(g) = &c.gd {
                gd_grid(ui, g, pal);
            } else if !c.detail.is_empty() {
                line(ui, format!("{bcont}{ccont}    > {}", c.detail), pal.dim);
            }
        }
    }
    ui.add_space(2.0);
}

/// The 2-D goal-difference grid: this team's own margin across the top, the
/// threat team's winning margin down the side. Each cell is green (hold the
/// place), red (lose it), or amber (goal-difference tie → goals-for decides).
/// The green/red boundary is the "goal-difference of goal-differences" diagonal.
fn gd_grid(ui: &mut egui::Ui, g: &crate::live::GdViz, pal: Palette) {
    const CELL: f32 = 18.0;
    const AMBER: Color32 = Color32::from_rgb(217, 174, 24);
    let ncols = g.own_margins.len();
    let left = 56.0_f32; // indent
    let label_w = 168.0_f32; // room for "Germany beats Ecuador by"

    // Caption (x-axis label) on its own line, aligned over the number columns.
    ui.horizontal(|ui| {
        ui.add_space(left + label_w);
        ui.label(RichText::new(&g.own_axis).size(9.5).color(pal.dim));
    });
    // Column header: this team's own margin.
    ui.horizontal(|ui| {
        ui.add_space(left + label_w);
        for &om in &g.own_margins {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(CELL, 13.0), Sense::hover());
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                om.to_string(),
                egui::FontId::monospace(10.0),
                pal.dim,
            );
        }
    });

    // One row per threat-team winning margin — largest at the top, so the
    // break-even (amber) diagonal runs bottom-left → top-right like a chart.
    for (ri, &tm) in g.threat_margins.iter().enumerate().rev() {
        ui.horizontal(|ui| {
            ui.add_space(left);
            ui.add_sized(
                [label_w, CELL],
                egui::Label::new(
                    RichText::new(format!("{} {tm}", g.threat_axis))
                        .size(9.0)
                        .color(pal.dim),
                ),
            );
            for ci in 0..ncols {
                let cell = g.grid[ri * ncols + ci];
                let col = match cell {
                    crate::live::GdCell::Hold => COLOR_GREEN,
                    crate::live::GdCell::Lose => COLOR_RED,
                    crate::live::GdCell::Tie => AMBER,
                };
                let (rect, _) = ui.allocate_exact_size(egui::vec2(CELL, CELL), Sense::hover());
                ui.painter().rect_filled(
                    rect.shrink(1.0),
                    3.0,
                    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 60),
                );
            }
        });
    }

    // Legend.
    ui.horizontal(|ui| {
        ui.add_space(left);
        ui.label(
            RichText::new(format!(
                "green = {} (hold)   red = {} (lose)   amber = goals-for decides",
                super::sync::ordinal(g.hold),
                super::sync::ordinal(g.lose),
            ))
            .size(9.0)
            .color(pal.dim),
        );
    });
}

/// One today's-fixture row; LIVE matches get a breathing green background.
fn fixture_row(ui: &mut egui::Ui, f: &LiveFixture, pal: Palette) {
    let live = f.status.is_live();
    let fill = if live {
        let t = ui.input(|i| i.time);
        let pulse = ((t * 2.4).sin() * 0.5 + 0.5) as f32;
        let alpha = (40.0 + 50.0 * pulse) as u8;
        ui.ctx().request_repaint();
        Color32::from_rgba_unmultiplied(22, 163, 74, alpha)
    } else {
        Color32::TRANSPARENT
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(5.0)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let chip = if live { COLOR_RED } else { pal.dim };
                ui.add_sized(
                    [44.0, 16.0],
                    egui::Label::new(
                        RichText::new(f.status.label())
                            .size(11.0)
                            .strong()
                            .color(chip),
                    ),
                );
                flag_image(ui, &f.home_code, egui::vec2(18.0, 12.0));
                ui.add_sized(
                    [120.0, 16.0],
                    egui::Label::new(RichText::new(&f.home).size(12.0).color(pal.text)),
                );
                let mid = match f.score {
                    Some((h, a)) => format!("{h} – {a}"),
                    None => "v".to_string(),
                };
                ui.add_sized(
                    [44.0, 16.0],
                    egui::Label::new(
                        RichText::new(mid)
                            .size(12.0)
                            .strong()
                            .monospace()
                            .color(pal.text),
                    ),
                );
                ui.add_sized(
                    [120.0, 16.0],
                    egui::Label::new(RichText::new(&f.away).size(12.0).color(pal.text)),
                );
                flag_image(ui, &f.away_code, egui::vec2(18.0, 12.0));
            });
        });
}

/// One projected group card with ▲/▼ arrows vs the current (feed) position.
fn projected_group_card(
    ui: &mut egui::Ui,
    s: &LiveStanding,
    feed: &HashMap<String, u32>,
    playing: &std::collections::HashSet<String>,
    pal: Palette,
) {
    egui::Frame::new()
        .fill(pal.card)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(7))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(176.0);
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(
                    RichText::new(format!("Group {}", s.group))
                        .size(11.0)
                        .strong()
                        .color(pal.dim),
                );
                for t in &s.teams {
                    let (arrow, ac) = match feed.get(&t.code).copied() {
                        Some(old) if t.position < old => ("+", COLOR_GREEN),
                        Some(old) if t.position > old => ("-", COLOR_RED),
                        _ => (" ", pal.dim),
                    };
                    // Teams in a live game get a breathing green row.
                    let fill = if playing.contains(&t.code) {
                        let time = ui.input(|i| i.time);
                        let pulse = ((time * 2.4).sin() * 0.5 + 0.5) as f32;
                        let alpha = (38.0 + 46.0 * pulse) as u8;
                        ui.ctx().request_repaint();
                        Color32::from_rgba_unmultiplied(22, 163, 74, alpha)
                    } else {
                        Color32::TRANSPARENT
                    };
                    egui::Frame::new()
                        .fill(fill)
                        .corner_radius(3.0)
                        .inner_margin(egui::Margin::symmetric(2, 1))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                ui.label(RichText::new(arrow).size(11.0).strong().color(ac));
                                flag_image(ui, &t.code, egui::vec2(18.0, 12.0));
                                ui.add_sized(
                                    [34.0, 14.0],
                                    egui::Label::new(
                                        RichText::new(&t.code)
                                            .monospace()
                                            .size(12.0)
                                            .color(pal.text),
                                    ),
                                );
                                ui.label(
                                    RichText::new(format!("{}p {:+}", t.points, t.goal_diff))
                                        .size(10.0)
                                        .color(pal.dim),
                                );
                            });
                        });
                }
            });
        });
}

/// The Live-data window: API token entry + "Sync now" + status.
pub(crate) fn live_window(app: &mut PredictorApp, ctx: &egui::Context) {
    if !app.live.show_live {
        return;
    }
    let pal = app.pal();
    let mut open = true;

    egui::Window::new("Live data (football-data.org)")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::new()
                .fill(pal.panel)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(14)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.label(
                RichText::new("Pull real group standings and apply them to the bracket.")
                    .size(12.0)
                    .color(pal.dim),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("API token")
                    .size(12.0)
                    .strong()
                    .color(pal.text),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.live.api_key)
                    .password(true)
                    .hint_text("football-data.org token")
                    .desired_width(340.0),
            );
            ui.add_space(8.0);

            let syncing = app.live.live_rx.is_some();
            ui.horizontal(|ui| {
                let label = if syncing { "Syncing…" } else { "Sync now" };
                if ui
                    .add_enabled(
                        !syncing,
                        egui::Button::new(
                            RichText::new(label)
                                .size(13.0)
                                .color(crate::theme::COLOR_GREEN),
                        )
                        .fill(pal.card)
                        .stroke(Stroke::new(1.0, pal.border)),
                    )
                    .clicked()
                {
                    app.start_live_sync(ctx.clone(), "manual");
                }
            });

            ui.add_space(4.0);
            let mut live_mode = app.live.live_mode;
            if ui
                .checkbox(&mut live_mode, "Live mode — auto-poll every 20s + alerts")
                .changed()
            {
                app.live.live_mode = live_mode;
                app.live.last_poll = None; // poll right away when turned on
                if live_mode {
                    app.live.toasts.push(Toast::new(
                        "Live mode on — polling every 20s".to_string(),
                        AlertKind::Info,
                    ));
                }
            }
            ui.checkbox(
                &mut app.live.show_live_center,
                "Show Live Center (games, standings, 3rd place)",
            );

            if let Some(status) = &app.live.live_status {
                ui.add_space(8.0);
                ui.label(RichText::new(status).size(12.0).color(pal.dim));
            }
        });

    if !open {
        app.live.show_live = false;
    }
}

/// The cross-group 3rd-place ranking, with a cutoff line after the top 8.
fn third_place_table(
    ui: &mut egui::Ui,
    app: &PredictorApp,
    ranks: &[crate::live::ThirdPlaceRank],
    pal: Palette,
) {
    // Column header, aligned to the fixed-width left columns of each row.
    let head = |ui: &mut egui::Ui, w: f32, t: &str| {
        ui.add_sized(
            [w, 14.0],
            egui::Label::new(RichText::new(t).size(9.5).strong().color(pal.dim)),
        )
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        head(ui, 12.0, ""); // movement arrow
        head(ui, 20.0, "#");
        head(ui, 14.0, "Grp");
        ui.add_space(20.0); // flag
        head(ui, 120.0, "Team");
        head(ui, 38.0, "Adv%")
            .on_hover_text("Simulated chance of reaching the Round of 32");
        head(ui, 28.0, ""); // IN/OUT clinch tag
        head(ui, 44.0, "Pld");
        head(ui, 44.0, "Pts");
        head(ui, 44.0, "GD");
        head(ui, 40.0, "GF");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            head(ui, 34.0, "Status");
        });
    });
    ui.add_space(2.0);

    for (i, r) in ranks.iter().enumerate() {
        if i == 8 {
            ui.add_space(3.0);
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width().min(460.0), 2.0),
                Sense::hover(),
            );
            ui.painter()
                .hline(rect.x_range(), rect.center().y, Stroke::new(1.5, COLOR_RED));
            ui.label(
                RichText::new("cutoff — below this line is eliminated")
                    .size(9.0)
                    .color(COLOR_RED),
            );
            ui.add_space(3.0);
        }
        let color = if r.advances { COLOR_GREEN } else { COLOR_RED };
        // Movement arrow vs the previous poll.
        let (arrow, arrow_color) = match app.live.third_delta.get(&r.code).copied() {
            Some(d) if d < 0 => ("+", COLOR_GREEN),
            Some(d) if d > 0 => ("-", COLOR_RED),
            Some(_) => ("=", pal.dim),
            None => (" ", pal.dim),
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_sized(
                [12.0, 16.0],
                egui::Label::new(RichText::new(arrow).size(12.0).strong().color(arrow_color)),
            );
            ui.add_sized(
                [20.0, 16.0],
                egui::Label::new(
                    RichText::new(format!("{:>2}", i + 1))
                        .monospace()
                        .size(12.0)
                        .color(pal.dim),
                ),
            );
            ui.add_sized(
                [14.0, 16.0],
                egui::Label::new(
                    RichText::new(r.group.to_string())
                        .monospace()
                        .size(12.0)
                        .strong()
                        .color(pal.dim),
                ),
            );
            flag_image(ui, &r.code, egui::vec2(20.0, 13.0));
            let outlook = app.live.third_outlook.get(&r.code).copied().unwrap_or_default();
            let mut name = RichText::new(&r.name).size(12.0).color(pal.text);
            if outlook.eliminated {
                name = name.strikethrough().color(COLOR_RED);
            } else if outlook.clinched {
                name = name.color(COLOR_GREEN);
            }
            ui.add_sized([120.0, 16.0], egui::Label::new(name));
            // Advance probability + a guaranteed IN / OUT tag.
            let (tag, tag_col) = if outlook.clinched {
                ("IN", COLOR_GREEN)
            } else if outlook.eliminated {
                ("OUT", COLOR_RED)
            } else {
                ("", pal.dim)
            };
            let pct = (outlook.pct * 100.0).round() as i32;
            let pct_col = if pct >= 80 {
                COLOR_GREEN
            } else if pct <= 20 {
                COLOR_RED
            } else {
                pal.text
            };
            // Fixed-width columns (numbers only) so they align under the header.
            ui.add_sized(
                [38.0, 16.0],
                egui::Label::new(
                    RichText::new(format!("{pct}%"))
                        .size(11.0)
                        .strong()
                        .monospace()
                        .color(pct_col),
                ),
            )
            .on_hover_text("Simulated chance of reaching the Round of 32");
            ui.add_sized(
                [28.0, 16.0],
                egui::Label::new(RichText::new(tag).size(9.5).strong().color(tag_col)),
            )
            .on_hover_text(match (outlook.clinched, outlook.eliminated) {
                (true, _) => "Guaranteed a top-8 third — advances",
                (_, true) => "Mathematically eliminated from the best-thirds race",
                _ => "",
            });
            let col = |ui: &mut egui::Ui, w: f32, t: String, c: Color32, strong: bool| {
                let mut rt = RichText::new(t).size(11.0).monospace().color(c);
                if strong {
                    rt = rt.strong();
                }
                ui.add_sized([w, 16.0], egui::Label::new(rt));
            };
            col(ui, 44.0, format!("{}", r.played), pal.dim, false);
            col(ui, 44.0, format!("{}", r.points), pal.text, true);
            col(ui, 44.0, format!("{:+}", r.goal_diff), pal.dim, false);
            col(ui, 40.0, format!("{}", r.goals_for), pal.dim, false);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_sized(
                    [34.0, 16.0],
                    egui::Label::new(
                        RichText::new(if r.advances { "ADV" } else { "OUT" })
                            .size(11.0)
                            .strong()
                            .color(color),
                    ),
                );
            });
        });
    }
}
