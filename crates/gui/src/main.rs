#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;

const CONF_CACHE: &str = ".tiny_portal_cache.json";

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    let home = dirs::home_dir().unwrap();
    let conf_path = home.join(CONF_CACHE);
    let app = if conf_path.exists() {
        let conf = std::fs::read_to_string(&conf_path).unwrap();
        let app: MyApp = serde_json::from_str(&conf).unwrap();
        app
    } else {
        MyApp::default()
    };

    eframe::run_native(
        "My egui App",
        options,
        Box::new(|_| {
            // This gives us image support:
            // egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::new(app)
        }),
    )
}

struct AsyncRuntime(tokio::runtime::Runtime);
impl Default for AsyncRuntime {
    fn default() -> Self {
        Self(tokio::runtime::Runtime::new().unwrap())
    }
}

#[derive(PartialEq, Clone, Copy, serde::Deserialize, serde::Serialize, Default)]
enum Protocol {
    #[default]
    TCP,
    UDP,
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
struct MyApp {
    src_addr: String,
    dst_addr: String,
    protocol: Protocol,
    #[serde(skip)]
    stop_portal: Option<tokio::task::JoinHandle<()>>,
    #[serde(skip)]
    rt: AsyncRuntime,
}

impl MyApp {
    fn start(&mut self) {
        let src_addr = self.src_addr.clone();
        let dst_addr = self.dst_addr.clone();
        let protocol = self.protocol;

        let j = self.rt.0.spawn(async move {
            let res = if protocol == Protocol::TCP {
                tiny_portal::TcpPortForwarder::new(&src_addr, &dst_addr)
                    .start()
                    .await
            } else {
                tiny_portal::UdpPortForwarder::new(&src_addr, &dst_addr)
                    .start()
                    .await
            };

            if let Err(e) = res {
                log::error!("Port forwarder stopped: {:?}", e);
            } else {
                log::info!("Port forwarder stopped");
            }
        });

        self.stop_portal = Some(j);
    }

    fn is_busy(&mut self) -> bool {
        if let Some(j) = &self.stop_portal {
            if j.is_finished() {
                self.stop_portal = None;
            }
        }
        self.stop_portal.is_some()
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let busy = self.is_busy();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Tiny Portal");

            ui.vertical(|ui| {
                ui.group(|ui| {
                    ui.set_enabled(!busy);

                    ui.horizontal(|ui| {
                        let src_label = ui.label("Src: ");
                        ui.text_edit_singleline(&mut self.src_addr)
                            .labelled_by(src_label.id);
                    });
                    ui.horizontal(|ui| {
                        let dst_label = ui.label("Dst: ");
                        ui.text_edit_singleline(&mut self.dst_addr)
                            .labelled_by(dst_label.id);
                    });

                    ui.horizontal(|ui| {
                        let _ = ui.label("Protocol: ");
                        ui.selectable_value(&mut self.protocol, Protocol::TCP, "TCP");
                        ui.selectable_value(&mut self.protocol, Protocol::UDP, "UDP");
                    });
                });

                let btn = ui.button(if !busy { "Start" } else { "Stop" });

                if btn.clicked() {
                    if let Some(f) = self.stop_portal.take() {
                        f.abort();
                    } else {
                        self.start();
                    }
                }
            });
        });
    }
}

impl Drop for MyApp {
    fn drop(&mut self) {
        if let Some(j) = self.stop_portal.take() {
            j.abort();
        }
        let home = dirs::home_dir().unwrap();
        let conf_path = home.join(CONF_CACHE);
        std::fs::write(&conf_path, serde_json::to_string(self).unwrap()).unwrap();
    }
}
