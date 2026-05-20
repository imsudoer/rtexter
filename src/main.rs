#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod engine;
mod macros;

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([700.0, 450.0])
            .with_title("RTexter v2.0"),
        ..Default::default()
    };

    eframe::run_native(
        "RTexter",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light()); // light/dark
            Box::new(app::HTexterApp::new(cc))
        }),
    )
}
