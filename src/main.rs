//! FIFA World Cup 2026 knockout-bracket predictor (eframe/egui desktop app).

// Hide the console window on Windows release builds (keep it for debugging).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bracket;
mod flags;
mod icon;
mod live;
mod print;
mod saves;
mod settings;
mod standings;
mod theme;
mod tutorial;

use eframe::egui;

use crate::app::PredictorApp;
use crate::icon::app_icon;

pub(crate) const APP_NAME: &str = "WC26_Bracket";

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 600.0])
            .with_icon(std::sync::Arc::new(app_icon())),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(PredictorApp::new(cc)))),
    )
}
