//! Named-saves manager: save the current bracket under a name, load others' files.

use eframe::egui::{self, RichText, Stroke};

use crate::app::PredictorApp;
use crate::settings::list_saves;
use crate::theme::{COLOR_GREEN, COLOR_RED};

/// Modal window listing saved brackets with save / load / delete actions.
pub(crate) fn saves_window(app: &mut PredictorApp, ctx: &egui::Context) {
    if !app.show_saves {
        return;
    }
    let pal = app.pal();
    let mut open = true;

    egui::Window::new("Saved brackets")
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
            ui.set_min_width(360.0);

            // Save-as row.
            ui.label(
                RichText::new("Save current bracket as")
                    .size(13.0)
                    .strong()
                    .color(pal.text),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.save_name)
                        .hint_text("e.g. Edwin's picks")
                        .desired_width(240.0),
                );
                if ui
                    .add(egui::Button::new(
                        RichText::new("Save").size(12.0).color(COLOR_GREEN),
                    ))
                    .clicked()
                {
                    let name = app.save_name.clone();
                    app.save_as(&name);
                }
            });

            ui.add_space(6.0);
            // Import / export to any location on disk.
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("⬆ Import file…").size(12.0).color(pal.text),
                        )
                        .fill(pal.card)
                        .stroke(Stroke::new(1.0, pal.border)),
                    )
                    .on_hover_text("Open a .json bracket from anywhere")
                    .clicked()
                {
                    app.import_file();
                }
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("⬇ Export file…").size(12.0).color(pal.text),
                        )
                        .fill(pal.card)
                        .stroke(Stroke::new(1.0, pal.border)),
                    )
                    .on_hover_text("Save the current bracket anywhere to share it")
                    .clicked()
                {
                    app.export_file();
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(
                RichText::new("Saved brackets")
                    .size(13.0)
                    .strong()
                    .color(pal.text),
            );

            let saves = list_saves();
            if saves.is_empty() {
                ui.label(RichText::new("No saves yet.").size(12.0).color(pal.dim));
            }

            egui::ScrollArea::vertical()
                .max_height(280.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for name in saves {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&name).size(13.0).color(pal.text));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(egui::Button::new(
                                            RichText::new("Delete").size(11.0).color(COLOR_RED),
                                        ))
                                        .clicked()
                                    {
                                        app.delete_named(&name);
                                    }
                                    if ui
                                        .add(egui::Button::new(
                                            RichText::new("Load").size(11.0).color(COLOR_GREEN),
                                        ))
                                        .clicked()
                                    {
                                        app.load_named(&name);
                                        app.show_saves = false;
                                    }
                                },
                            );
                        });
                        ui.add_space(2.0);
                    }
                });
        });

    if !open {
        app.show_saves = false;
    }
}
