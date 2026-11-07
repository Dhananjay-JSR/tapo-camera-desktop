#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window on Windows release

mod app;
mod audio_in;
mod audio_out;
mod decode;
#[cfg(all(target_os = "macos", feature = "hw-videotoolbox"))]
mod decode_vt;
mod logbuf;
mod store;
mod stream;
mod video_gl;
mod worker;

fn main() -> eframe::Result<()> {
    logbuf::init();
    logbuf::install_panic_hook();
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
