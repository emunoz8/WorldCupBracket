//! Application state and the top-level egui frame.

use std::collections::HashMap;
use std::fs;

use eframe::egui::{self, Color32, Rect, RichText, Stroke, Vec2};
use fifa_team3::{
    Annex, GroupState, KoMatch, PredictionReport, Side, Slot, WinnerPrediction,
    annex_filters_from_groups, prediction_report,
};

use crate::APP_NAME;
use crate::bracket::bracket_view;
use crate::print::{build_print_html, open_path};
use crate::settings::{AppState, TEAMS_PATH, save_path};
use crate::standings::{group_table, info_chip, third_place_chip};
use crate::theme::{COLOR_ACCENT, COLOR_GREEN, COLOR_RED, Palette};

pub(crate) struct PredictorApp {
    annex: Annex,
    pub(crate) groups: Vec<GroupState>,
    pub(crate) dragged: Option<(usize, usize)>,
    /// User-selected winner for each knockout match number.
    pub(crate) picks: HashMap<usize, Side>,
    /// Whether the left standings panel is visible.
    show_standings: bool,
    /// Dark theme when true, light theme when false.
    dark: bool,
    /// Transient status message for save/reload feedback.
    status: Option<String>,
    load_error: Option<String>,
}

impl PredictorApp {
    pub(crate) fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let annex_result = fs::read_to_string("data/annex_c.json")
            .map_err(|e| e.to_string())
            .and_then(|json| serde_json::from_str(&json).map_err(|e| e.to_string()));

        // Prefer the live save; before the first Save exists, seed groups from the
        // committed teams.json; fall back to placeholder slots only if both are gone.
        let state: AppState = fs::read_to_string(save_path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_else(|| {
                let mut state = AppState::default();
                if let Some(groups) = fs::read_to_string(TEAMS_PATH)
                    .ok()
                    .and_then(|json| serde_json::from_str(&json).ok())
                {
                    state.groups = groups;
                }
                state
            });

        let (annex, load_error) = match annex_result {
            Ok(annex) => (annex, None),
            Err(e) => (Annex::new(), Some(e)),
        };

        Self {
            annex,
            groups: state.groups,
            dragged: None,
            picks: state.picks,
            show_standings: state.show_standings,
            dark: state.dark,
            status: None,
            load_error,
        }
    }

    /// Build a printable HTML report and open it in the default browser.
    fn print_bracket(&mut self, report: &PredictionReport) {
        let html = build_print_html(self, report);
        let mut path = std::env::temp_dir();
        path.push("fifa_bracket_print.html");
        self.status = Some(match fs::write(&path, html) {
            Ok(()) => match open_path(&path) {
                Ok(()) => "Opened print view in your browser (use Cmd/Ctrl+P)".to_string(),
                Err(e) => format!("Saved {} but could not open it: {e}", path.display()),
            },
            Err(e) => format!("Print failed: {e}"),
        });
    }

    fn report(&self) -> PredictionReport {
        let (passing, eliminated) = annex_filters_from_groups(&self.groups);
        prediction_report(&self.annex, &passing, &eliminated)
    }

    pub(crate) fn pal(&self) -> Palette {
        if self.dark {
            Palette::DARK
        } else {
            Palette::LIGHT
        }
    }

    /// Snapshot everything (standings + theme + panel + bracket picks) to disk.
    fn save_state(&mut self) {
        let state = AppState {
            dark: self.dark,
            show_standings: self.show_standings,
            picks: self.picks.clone(),
            groups: self.groups.clone(),
        };
        let path = save_path();
        self.status = Some(
            match serde_json::to_string_pretty(&state)
                .map_err(|e| e.to_string())
                .and_then(|json| {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    fs::write(&path, json).map_err(|e| e.to_string())
                }) {
                Ok(()) => "Saved standings, theme, and bracket".to_string(),
                Err(e) => format!("Save failed: {e}"),
            },
        );
    }

    /// Restore the full saved state from disk, discarding unsaved changes.
    fn reload_state(&mut self) {
        self.status = Some(
            match fs::read_to_string(save_path())
                .map_err(|e| e.to_string())
                .and_then(|json| serde_json::from_str::<AppState>(&json).map_err(|e| e.to_string()))
            {
                Ok(state) => {
                    self.groups = state.groups;
                    self.picks = state.picks;
                    self.show_standings = state.show_standings;
                    self.dark = state.dark;
                    "Reloaded saved settings".to_string()
                }
                Err(e) => format!("Reload failed: {e}"),
            },
        );
    }

    fn team_for_slot(&self, slot: &str) -> String {
        let mut chars = slot.chars();
        let position = chars.next().and_then(|v| v.to_digit(10)).unwrap_or(0) as usize;
        let group = chars.next().unwrap_or(' ');
        self.groups
            .iter()
            .find(|s| s.group == group)
            .and_then(|s| s.teams.get(position.saturating_sub(1)))
            .map(|t| t.name.clone())
            .unwrap_or_else(|| slot.to_string())
    }

    pub(crate) fn third_place_team(&self, opponent: &str) -> String {
        opponent
            .chars()
            .nth(1)
            .and_then(|group| {
                self.groups
                    .iter()
                    .find(|s| s.group == group)
                    .and_then(|s| s.teams.get(2))
                    .map(|t| t.name.clone())
            })
            .unwrap_or_else(|| opponent.to_string())
    }

    /// Display name for one competitor of a match, following picks into earlier rounds.
    pub(crate) fn resolve_side(
        &self,
        km: &KoMatch,
        side: Side,
        index: &HashMap<usize, KoMatch>,
        predictions: &HashMap<&str, &WinnerPrediction>,
    ) -> String {
        let slot = match side {
            Side::Left => km.left,
            Side::Right => km.right,
        };
        match slot {
            Slot::Group(s) => self.team_for_slot(s),
            // A third-place opponent: name it only once a single opponent remains.
            Slot::ThirdPlace => self.certain_third_place(km, predictions),
            Slot::Winner(m) => match (self.picks.get(&m), index.get(&m)) {
                (Some(picked), Some(child)) => {
                    self.resolve_side(child, *picked, index, predictions)
                }
                _ => format!("Winner M{m}"),
            },
            Slot::Loser(m) => match (self.picks.get(&m), index.get(&m)) {
                (Some(Side::Left), Some(child)) => {
                    self.resolve_side(child, Side::Right, index, predictions)
                }
                (Some(Side::Right), Some(child)) => {
                    self.resolve_side(child, Side::Left, index, predictions)
                }
                _ => format!("Loser M{m}"),
            },
        }
    }

    /// The third-place opponent's real team name if exactly one remains possible, else "3rd place".
    fn certain_third_place(
        &self,
        km: &KoMatch,
        predictions: &HashMap<&str, &WinnerPrediction>,
    ) -> String {
        if let Slot::Group(winner_slot) = km.left
            && let Some(prediction) = predictions.get(winner_slot)
            && prediction.opponents.len() == 1
        {
            return self.third_place_team(&prediction.opponents[0].opponent);
        }
        "3rd place".to_string()
    }
}

impl eframe::App for PredictorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let report = self.report();
        let pal = self.pal();

        ctx.set_visuals(if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        let mut style = (*ctx.style()).clone();
        style.visuals.panel_fill = pal.panel;
        style.visuals.window_fill = pal.card;
        ctx.set_style(style);

        egui::TopBottomPanel::top("top_bar")
            .exact_height(52.0)
            .frame(
                egui::Frame::new()
                    .fill(pal.top_bar)
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let toggle_label = if self.show_standings {
                        "◀ Hide standings"
                    } else {
                        "Show standings ▶"
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(toggle_label).size(12.0).color(pal.text),
                            )
                            .fill(pal.card)
                            .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.show_standings = !self.show_standings;
                    }
                    let theme_label = if self.dark { "☀ Light" } else { "🌙 Dark" };
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(theme_label).size(12.0).color(pal.text),
                            )
                            .fill(pal.card)
                            .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.dark = !self.dark;
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Save").size(12.0).color(COLOR_GREEN))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.save_state();
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Reload").size(12.0).color(pal.dim))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.reload_state();
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("🖨 Print").size(12.0).color(pal.text))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .clicked()
                    {
                        self.print_bracket(&report);
                    }
                    ui.add_space(12.0);
                    ui.label(RichText::new(APP_NAME).size(20.0).strong().color(pal.text));
                    ui.add_space(16.0);
                    if let Some(status) = &self.status {
                        ui.label(RichText::new(status).size(12.0).color(pal.dim));
                    }
                    info_chip(
                        ui,
                        &format!("{} scenarios", report.possible_scenarios),
                        COLOR_ACCENT,
                    );
                    if !report.known_passing.is_empty() {
                        info_chip(
                            ui,
                            &format!("Advanced: {}", report.known_passing),
                            COLOR_GREEN,
                        );
                    }
                    if !report.known_eliminated.is_empty() {
                        info_chip(ui, &format!("Out: {}", report.known_eliminated), COLOR_RED);
                    }
                    if let Some(err) = &self.load_error {
                        ui.label(RichText::new(err).color(COLOR_RED).size(12.0));
                    }
                });
            });

        if self.show_standings {
            egui::SidePanel::left("groups_panel")
                .resizable(true)
                .default_width(340.0)
                .width_range(280.0..=480.0)
                .frame(
                    egui::Frame::new()
                        .fill(pal.panel)
                        .inner_margin(egui::Margin::same(0)),
                )
                .show(ctx, |ui| {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Standings")
                                .size(18.0)
                                .strong()
                                .color(pal.text),
                        );
                        ui.label(RichText::new("drag to reorder").size(11.0).color(pal.dim));
                    });
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(2.0);
                            for group_index in 0..self.groups.len() {
                                group_table(ui, self, group_index);
                                ui.add_space(10.0);
                            }
                        });
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(pal.panel)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                if !report.errors.is_empty() {
                    for error in &report.errors {
                        ui.label(RichText::new(error).color(COLOR_RED));
                    }
                }

                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("3rd place:").size(12.0).color(pal.dim));
                    for group in &mut self.groups {
                        third_place_chip(ui, group, pal);
                    }
                });
                ui.add_space(8.0);

                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        bracket_view(ui, self, &report);
                    });
            });

        // Floating ghost card following cursor while dragging
        if let Some((gi, pi)) = self.dragged {
            ctx.request_repaint();
            if let Some(cursor) = ctx.input(|i| i.pointer.interact_pos()) {
                let layer = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("drag_ghost"),
                ));
                let ghost_rect =
                    Rect::from_min_size(cursor + Vec2::new(14.0, -15.0), Vec2::new(180.0, 30.0));
                layer.rect(
                    ghost_rect,
                    4.0,
                    Color32::from_rgb(40, 40, 55),
                    Stroke::new(1.5, COLOR_ACCENT),
                    egui::StrokeKind::Middle,
                );
                if let Some(team) = self.groups[gi].teams.get(pi) {
                    layer.text(
                        ghost_rect.min + Vec2::new(10.0, 8.0),
                        egui::Align2::LEFT_TOP,
                        &team.name,
                        egui::FontId::proportional(12.0),
                        pal.text,
                    );
                }
            }
        }

        if ctx.input(|i| i.pointer.any_released()) {
            self.dragged = None;
        }
    }
}
