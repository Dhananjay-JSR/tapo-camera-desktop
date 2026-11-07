#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window on Windows release

mod app;
mod audio_in;
mod audio_out;
mod decode;
mod store;
mod stream;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 640.0])
            .with_title("Tapo Desktop"),
        ..Default::default()
    };
    eframe::run_native(
        "Tapo Desktop",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
