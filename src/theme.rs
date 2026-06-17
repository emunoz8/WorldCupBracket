//! Colour palette and theme definitions shared across the UI.

use eframe::egui::Color32;

// Accent colours read well on both themes, so they stay fixed.
pub(crate) const COLOR_GREEN: Color32 = Color32::from_rgb(22, 163, 74);
pub(crate) const COLOR_RED: Color32 = Color32::from_rgb(220, 38, 38);
pub(crate) const COLOR_ACCENT: Color32 = Color32::from_rgb(59, 130, 246);
pub(crate) const COLOR_GOLD: Color32 = Color32::from_rgb(212, 175, 55);

/// Theme-dependent surface and text colours.
#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub(crate) panel: Color32,
    pub(crate) top_bar: Color32,
    pub(crate) card: Color32,
    pub(crate) row: Color32,
    pub(crate) row_alt: Color32,
    pub(crate) header: Color32,
    pub(crate) text: Color32,
    pub(crate) dim: Color32,
    pub(crate) border: Color32,
    pub(crate) line: Color32,
    pub(crate) hover: Color32,
}

impl Palette {
    pub(crate) const DARK: Self = Self {
        panel: Color32::from_rgb(18, 18, 18),
        top_bar: Color32::from_rgb(14, 14, 14),
        card: Color32::from_rgb(34, 34, 34),
        row: Color32::from_rgb(28, 28, 28),
        row_alt: Color32::from_rgb(38, 38, 38),
        header: Color32::from_rgb(28, 28, 28),
        text: Color32::from_rgb(230, 230, 230),
        dim: Color32::from_rgb(130, 130, 130),
        border: Color32::from_rgb(55, 55, 55),
        line: Color32::from_rgb(75, 75, 75),
        hover: Color32::from_rgb(46, 46, 46),
    };

    pub(crate) const LIGHT: Self = Self {
        panel: Color32::from_rgb(242, 242, 245),
        top_bar: Color32::from_rgb(228, 228, 232),
        card: Color32::from_rgb(255, 255, 255),
        row: Color32::from_rgb(250, 250, 252),
        row_alt: Color32::from_rgb(238, 238, 242),
        header: Color32::from_rgb(232, 232, 236),
        text: Color32::from_rgb(24, 24, 28),
        dim: Color32::from_rgb(110, 110, 120),
        border: Color32::from_rgb(205, 205, 212),
        line: Color32::from_rgb(170, 170, 180),
        hover: Color32::from_rgb(224, 224, 230),
    };
}
