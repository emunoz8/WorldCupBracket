//! Application state and the top-level egui frame.

use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

use eframe::egui::{self, Rect, RichText, Stroke, Vec2};
use fifa_team3::{
    Annex, GroupState, KoMatch, PredictionReport, Side, Slot, WinnerPrediction,
    annex_filters_from_groups, prediction_report, seed_group_states,
};

use crate::APP_NAME;
use crate::bracket::bracket_view;
use crate::print::{build_print_html, open_path};
use crate::settings::{AppState, save_path};
use crate::standings::{group_table, info_chip, third_place_chip};
use crate::theme::{COLOR_ACCENT, COLOR_GREEN, COLOR_RED, Palette};

pub(crate) struct PredictorApp {
    pub(crate) annex: Annex,
    pub(crate) groups: Vec<GroupState>,
    pub(crate) dragged: Option<(usize, usize)>,
    /// Row a drag is currently hovering over (group, position) — where a gap opens.
    pub(crate) drop_target: Option<(usize, usize)>,
    /// User-selected winner for each knockout match number.
    pub(crate) picks: HashMap<usize, Side>,
    /// Whether the left standings panel is visible.
    show_standings: bool,
    /// Dark theme when true, light theme when false.
    dark: bool,
    /// Transient status message for save/reload feedback.
    status: Option<String>,
    load_error: Option<String>,
    /// Active onboarding step, or None when the tutorial is not running.
    pub(crate) tutorial: Option<crate::tutorial::TutorialStep>,
    /// Group letters selected as the 4 worst 3rd-place teams during the tutorial.
    pub(crate) worst: std::collections::HashSet<char>,
    /// Whether the tutorial has been completed/skipped (persisted).
    pub(crate) tutorial_seen: bool,
    /// Whether the named-saves manager window is open.
    pub(crate) show_saves: bool,
    /// Text-field buffer for naming a new save.
    pub(crate) save_name: String,
    /// All live-mode state (windows, sync, standings, alerts).
    pub(crate) live: crate::live::LiveState,
}

impl PredictorApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Enable image (incl. SVG) loaders so flag textures can be drawn.
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Annex C is embedded at compile time so the binary is self-contained.
        const ANNEX_JSON: &str = include_str!("../data/annex_c.json");
        let annex_result = serde_json::from_str(ANNEX_JSON).map_err(|e| e.to_string());

        // Start from a clean slate every launch (seed teams, no picks, no results).
        // Only UI preferences (theme, panel, tutorial-seen) are remembered; the
        // bracket/standings are loaded only when the user opens a saved profile.
        let saved: Option<AppState> = fs::read_to_string(save_path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok());

        let (annex, load_error) = match annex_result {
            Ok(annex) => (annex, None),
            Err(e) => (Annex::new(), Some(e)),
        };

        let dark = saved.as_ref().is_none_or(|s| s.dark);
        let show_standings = saved.as_ref().is_none_or(|s| s.show_standings);
        let tutorial_seen = saved.as_ref().is_some_and(|s| s.tutorial_seen);

        // Auto-run the tutorial until it has been completed/skipped once.
        let tutorial = (!tutorial_seen).then_some(crate::tutorial::TutorialStep::Groups);

        Self {
            annex,
            groups: seed_group_states(),
            dragged: None,
            drop_target: None,
            picks: HashMap::new(),
            show_standings,
            dark,
            status: None,
            load_error,
            tutorial,
            worst: std::collections::HashSet::new(),
            tutorial_seen,
            show_saves: false,
            save_name: String::new(),
            live: crate::live::LiveState::default(),
        }
    }

    /// Reset everything to zero: seed teams, no standings results, no bracket picks.
    pub(crate) fn reset_all(&mut self) {
        self.groups = seed_group_states();
        self.picks.clear();
        self.status = Some("Reset to a clean slate".to_string());
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

    pub(crate) fn report(&self) -> PredictionReport {
        let (passing, eliminated) = annex_filters_from_groups(&self.groups);
        prediction_report(&self.annex, &passing, &eliminated)
    }

    /// Live stats for a team (by group + code), if a sync has been done.
    pub(crate) fn live_stats(&self, group: char, code: &str) -> Option<&crate::live::LiveTeam> {
        let cc = crate::live::canonical_code(code);
        self.live
            .live_standings
            .iter()
            .find(|s| s.group == group)?
            .teams
            .iter()
            .find(|t| t.code == cc)
    }

    pub(crate) fn pal(&self) -> Palette {
        if self.dark {
            Palette::DARK
        } else {
            Palette::LIGHT
        }
    }

    /// Capture the current UI/standings/bracket into a serializable snapshot.
    fn snapshot(&self) -> AppState {
        AppState {
            dark: self.dark,
            show_standings: self.show_standings,
            picks: self.picks.clone(),
            groups: self.groups.clone(),
            tutorial_seen: self.tutorial_seen,
        }
    }

    /// Apply a loaded snapshot onto the live app state. Loading turns off the live
    /// feed so a freshly loaded bracket/standings can't be overwritten by a sync.
    fn apply_state(&mut self, state: AppState) {
        self.groups = state.groups;
        self.picks = state.picks;
        self.show_standings = state.show_standings;
        self.dark = state.dark;
        self.tutorial_seen = state.tutorial_seen;
        self.live.live_mode = false;
        self.live.live_rx = None; // drop any in-flight sync
    }

    fn write_snapshot(&self, path: &std::path::Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.snapshot()).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, json).map_err(|e| e.to_string())
    }

    /// Snapshot everything to the current-session file (auto-restored on launch).
    pub(crate) fn save_state(&mut self) {
        self.status = Some(match self.write_snapshot(&save_path()) {
            Ok(()) => "Saved standings, theme, and bracket".to_string(),
            Err(e) => format!("Save failed: {e}"),
        });
    }

    /// Restore the current-session file, discarding unsaved changes.
    fn reload_state(&mut self) {
        self.status = Some(
            match fs::read_to_string(save_path())
                .map_err(|e| e.to_string())
                .and_then(|json| serde_json::from_str::<AppState>(&json).map_err(|e| e.to_string()))
            {
                Ok(state) => {
                    self.apply_state(state);
                    "Reloaded current session".to_string()
                }
                Err(e) => format!("Reload failed: {e}"),
            },
        );
    }

    /// Save the current state under a chosen name (a separate, shareable file).
    pub(crate) fn save_as(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.status = Some("Enter a name to save".to_string());
            return;
        }
        let path = crate::settings::named_save_path(name);
        self.status = Some(match self.write_snapshot(&path) {
            Ok(()) => {
                let _ = self.write_snapshot(&save_path()); // keep current session in sync
                format!("Saved as \"{name}\"")
            }
            Err(e) => format!("Save failed: {e}"),
        });
    }

    /// Load a named save (e.g. someone else's bracket) into the live state.
    pub(crate) fn load_named(&mut self, name: &str) {
        self.status = Some(
            match fs::read_to_string(crate::settings::named_save_path(name))
                .map_err(|e| e.to_string())
                .and_then(|json| serde_json::from_str::<AppState>(&json).map_err(|e| e.to_string()))
            {
                Ok(state) => {
                    self.apply_state(state);
                    let _ = self.write_snapshot(&save_path());
                    format!("Loaded \"{name}\"")
                }
                Err(e) => format!("Load failed: {e}"),
            },
        );
    }

    /// Export the current bracket to any location via a native save dialog.
    pub(crate) fn export_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Bracket", &["json"])
            .set_file_name("bracket.json")
            .save_file()
        {
            self.status = Some(match self.write_snapshot(&path) {
                Ok(()) => format!("Exported to {}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            });
        }
    }

    /// Import a bracket from any location via a native open dialog.
    pub(crate) fn import_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Bracket", &["json"])
            .pick_file()
        {
            self.status = Some(
                match fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|json| {
                        serde_json::from_str::<AppState>(&json).map_err(|e| e.to_string())
                    }) {
                    Ok(state) => {
                        self.apply_state(state);
                        let _ = self.write_snapshot(&save_path());
                        self.show_saves = false;
                        format!("Imported {}", path.display())
                    }
                    Err(e) => format!("Import failed: {e}"),
                },
            );
        }
    }

    /// Delete a named save file.
    pub(crate) fn delete_named(&mut self, name: &str) {
        self.status = Some(
            match fs::remove_file(crate::settings::named_save_path(name)) {
                Ok(()) => format!("Deleted \"{name}\""),
                Err(e) => format!("Delete failed: {e}"),
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

    /// Live probability (0..=100) that the team currently projected into a group
    /// winner's R32 third-place slot actually lands there — P(team in this slot),
    /// simulated through the Annex. `winner_slot` is the group-winner key ("1A");
    /// `opponent` is the annex third key ("3C") naming the group whose third is
    /// projected there. `None` outside live mode or before any sim has run, so the
    /// bracket falls back to the annex option fraction.
    pub(crate) fn third_slot_probability(&self, winner_slot: &str, opponent: &str) -> Option<f64> {
        if !self.live.live_mode {
            return None;
        }
        let group = opponent.chars().nth(1)?;
        let code = &self.groups.iter().find(|s| s.group == group)?.teams.get(2)?.code;
        let slot = self.live.third_slot_pct.get(winner_slot)?;
        Some(f64::from(slot.get(code).copied().unwrap_or(0.0)) * 100.0)
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
        self.poll_live();

        // Keep repainting while a sync is in flight so the spinner animates and
        // poll_live picks up the result promptly.
        if self.live.live_rx.is_some() {
            ctx.request_repaint();
        }

        // Live mode: poll scores about once a minute.
        if self.live.live_mode {
            ctx.request_repaint_after(Duration::from_secs(1));
            let due = self
                .live
                .last_poll
                .is_none_or(|t| t.elapsed().as_secs() >= 20);
            if due && self.live.live_rx.is_none() {
                self.live.last_poll = Some(Instant::now());
                self.start_live_sync(ctx.clone(), "auto");
            }
        }

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
                            egui::Button::new(RichText::new("📁 Saves").size(12.0).color(pal.text))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .on_hover_text("Save under a name / load others' brackets")
                        .clicked()
                    {
                        self.show_saves = !self.show_saves;
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("🛰 Live").size(12.0).color(pal.text))
                                .fill(pal.card)
                                .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .on_hover_text("Pull real standings from football-data.org")
                        .clicked()
                    {
                        self.live.show_live = !self.live.show_live;
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
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("? Tutorial").size(12.0).color(pal.text),
                            )
                            .fill(pal.card)
                            .stroke(Stroke::new(1.0, pal.border)),
                        )
                        .on_hover_text("Replay the guided setup")
                        .clicked()
                    {
                        self.worst.clear();
                        self.tutorial = Some(crate::tutorial::TutorialStep::Groups);
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
                        pal,
                    );
                    if !report.known_passing.is_empty() {
                        info_chip(
                            ui,
                            &format!("Advanced: {}", report.known_passing),
                            COLOR_GREEN,
                            pal,
                        );
                    }
                    if !report.known_eliminated.is_empty() {
                        info_chip(
                            ui,
                            &format!("Out: {}", report.known_eliminated),
                            COLOR_RED,
                            pal,
                        );
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
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let all_done = self.groups.iter().all(|g| g.completed);
                        let (label, color) = if all_done {
                            ("✓ All groups complete — mark pending", COLOR_GREEN)
                        } else {
                            ("Complete all groups", pal.text)
                        };
                        if ui
                            .add(
                                egui::Button::new(RichText::new(label).size(12.0).color(color))
                                    .fill(pal.card)
                                    .stroke(Stroke::new(1.0, pal.border)),
                            )
                            .on_hover_text("Lock every group's standings as final")
                            .clicked()
                        {
                            let value = !all_done;
                            for group in &mut self.groups {
                                group.completed = value;
                            }
                        }
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
                            // Bottom padding so the last group (L) can scroll fully
                            // clear of the panel edge while reordering.
                            ui.add_space(160.0);
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
                    pal.card,
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

        // On release, drop the dragged team into the hovered slot (insertion move).
        if ctx.input(|i| i.pointer.any_released()) {
            if let (Some((from_group, from_pos)), Some((to_group, to_pos))) =
                (self.dragged, self.drop_target)
                && from_group == to_group
                // to_pos is an insertion index (0..=len); dropping at from_pos or
                // just after it is a no-op.
                && to_pos != from_pos
                && to_pos != from_pos + 1
            {
                let team = self.groups[from_group].teams.remove(from_pos);
                // Removing an earlier element shifts later insertion points down by one.
                let dest = if from_pos < to_pos {
                    to_pos - 1
                } else {
                    to_pos
                };
                self.groups[from_group].teams.insert(dest, team);
            }
            self.dragged = None;
            self.drop_target = None;
        }

        // Named-saves manager + onboarding overlay, drawn on top of everything.
        crate::saves::saves_window(self, ctx);
        crate::live::live_window(self, ctx);
        crate::live::live_center_window(self, ctx);
        crate::live::toasts_overlay(self, ctx);
        crate::tutorial::run(self, ctx);
    }
}
