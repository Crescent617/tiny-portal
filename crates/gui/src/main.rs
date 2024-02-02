#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Box::<MyApp>::default()
        }),
    )
}

struct MyApp {
    src_addr: String,
    dst_addr: String,
    protocol: String,
    stop: Option<tokio::task::JoinHandle<()>>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            src_addr: "".to_string(),
            dst_addr: "".to_string(),
            protocol: "".to_string(),
            stop: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.vertical(|ui| {
                let src_label = ui.label("Src: ");
                ui.text_edit_singleline(&mut self.src_addr)
                    .labelled_by(src_label.id);
                let dst_label = ui.label("Dst: ");
                ui.text_edit_singleline(&mut self.dst_addr)
                    .labelled_by(dst_label.id);
                let protocol_label = ui.label("Protocol: ");
                ui.text_edit_singleline(&mut self.protocol)
                    .labelled_by(protocol_label.id);
                if ui
                    .button(if self.stop.is_none() { "Start" } else { "Stop" })
                    .clicked()
                {
                    if let Some(f) = &self.stop {
                        f.abort();
                    } else {
                        let src_addr = self.src_addr.clone();
                        let dst_addr = self.dst_addr.clone();
                        let protocol = self.protocol.clone();
                        let j = std::thread::spawn(move || {
                            tokio::runtime::Runtime::new()
                                .unwrap()
                                .block_on(async move {});
                        });

                    }
                }
            });
        });
    }
}
