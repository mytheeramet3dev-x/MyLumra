mod app;
mod engine;
mod net;
mod ui;

use app::LumraApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Lumra Typst Browser v0.2.0...");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 820.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("Lumra - Typst Web Browser"),
        ..Default::default()
    };

    eframe::run_native(
        "Lumra - Typst Web Browser",
        native_options,
        Box::new(|cc| Ok(Box::new(LumraApp::new(cc)))),
    )
}
