#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod backend;
mod scenery;
mod ui;

fn main() -> eframe::Result {
    let data_dir = std::env::var_os("ASTER_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("Aster")
        });
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Aster — Your world, your way")
            .with_icon(app_icon())
            .with_inner_size([1180., 790.])
            .with_min_inner_size([920., 640.]),
        ..Default::default()
    };
    eframe::run_native(
        "Aster Launcher",
        options,
        Box::new(move |cc| Ok(Box::new(ui::Aster::new(cc, data_dir)))),
    )
}

fn app_icon() -> eframe::egui::IconData {
    let mut rgba = vec![0u8; 64 * 64 * 4];
    for y in 0..64 {
        for x in 0..64 {
            let dx = (x as f32 - 31.5).abs() / 25.;
            let dy = (y as f32 - 31.5).abs() / 25.;
            let star = dx.max(dy) + 3.35 * dx.min(dy) < 1.;
            let i = (y * 64 + x) * 4;
            let color = if star {
                [188, 204, 246, 255]
            } else {
                [28, 33, 37, 255]
            };
            rgba[i..i + 4].copy_from_slice(&color);
        }
    }
    eframe::egui::IconData {
        rgba,
        width: 64,
        height: 64,
    }
}
