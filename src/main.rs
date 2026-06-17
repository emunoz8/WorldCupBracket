//! FIFA World Cup 2026 knockout-bracket predictor (eframe/egui desktop app).

mod app;
mod bracket;
mod icon;
mod print;
mod settings;
mod standings;
mod theme;

use eframe::egui;

use crate::app::PredictorApp;
use crate::icon::app_icon;

pub(crate) const APP_NAME: &str = "Knockout Bracket Predictor";

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
