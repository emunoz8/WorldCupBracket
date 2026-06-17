//! Procedurally drawn application icon.

use eframe::egui;

/// A procedurally drawn app icon: a tournament-bracket glyph on a dark rounded tile.
pub(crate) fn app_icon() -> egui::IconData {
    const SIZE: usize = 256;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    let bg = [24u8, 24, 28, 255];
    let bar = [59u8, 130, 246, 255]; // accent blue
    let node = [212u8, 175, 55, 255]; // gold

    let px = |rgba: &mut [u8], x: usize, y: usize, c: [u8; 4]| {
        let i = (y * SIZE + x) * 4;
        rgba[i..i + 4].copy_from_slice(&c);
    };
    let fill = |rgba: &mut [u8], x0: usize, y0: usize, x1: usize, y1: usize, c: [u8; 4]| {
        for y in y0..y1 {
            for x in x0..x1 {
                px(rgba, x, y, c);
            }
        }
    };

    // Rounded dark tile.
    let r = 48usize;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = if x < r {
                r - x
            } else if x >= SIZE - r {
                x - (SIZE - r - 1)
            } else {
                0
            };
            let dy = if y < r {
                r - y
            } else if y >= SIZE - r {
                y - (SIZE - r - 1)
            } else {
                0
            };
            if dx == 0 || dy == 0 || dx * dx + dy * dy <= r * r {
                px(&mut rgba, x, y, bg);
            }
        }
    }

    // Bracket: two left arms merge into a single stem to the right.
    let t = 14usize;
    fill(&mut rgba, 64, 70, 150, 70 + t, bar); // top arm
    fill(&mut rgba, 64, 172, 150, 172 + t, bar); // bottom arm
    fill(&mut rgba, 150 - t, 70, 150, 172 + t, bar); // vertical connector
    fill(&mut rgba, 150, 121, 200, 121 + t, bar); // stem

    // Gold nodes at the two inputs and the champion output.
    let n = 11usize;
    fill(&mut rgba, 64 - n, 77 - n, 64 + n, 77 + n, node);
    fill(&mut rgba, 64 - n, 179 - n, 64 + n, 179 + n, node);
    fill(&mut rgba, 200 - n, 128 - n, 200 + n, 128 + n, node);

    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}
