mod audio_engine;
mod ui;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    
    // icon
    let icon_bytes = include_bytes!("../logo/icon.png");
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([600.0, 400.0])
            .with_icon(load_icon(icon_bytes)),
        ..Default::default()
    };

    eframe::run_native(
        "exIOMetter - Audio Mixer",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(ui::ExIOMeterApp::new(cc)))
        }),
    )
}

fn load_icon(icon_bytes: &[u8]) -> egui::IconData {
    let image = image::load_from_memory(icon_bytes)
        .expect("Failed to load icon")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    
    egui::IconData {
        rgba,
        width,
        height,
    }
}
