use raytracing::App;

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        vsync: false,
        ..Default::default()
    };
    eframe::run_native(
        "Ray tracing",
        native_options,
        Box::new(|cc| Box::new(App::new(cc))),
    )
}
