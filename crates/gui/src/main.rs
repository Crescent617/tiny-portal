#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::sync::{atomic::AtomicU64, Arc};

use eframe::{egui, Storage};

const SAVE_KEY: &str = "tiny_portal_gui.save";

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };

    eframe::run_native(
        "Tiny Portal GUI",
        options,
        Box::new(|cc| {
            // This gives us image support:
            // egui_extras::install_image_loaders(&cc.egui_ctx);
            if let Some(s) = cc.storage {
                if let Some(s) = s.get_string(SAVE_KEY) {
                    if let Ok(app) = ron::from_str::<MyApp>(&s) {
                        return Box::new(app);
                    }
                }
            }
            Box::new(MyApp::default())
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
    #[serde(skip)]
    conn_cnt: Option<Arc<AtomicU64>>,
}

impl MyApp {
    fn start(&mut self) {
        if self.protocol == Protocol::TCP {
            let p = tiny_portal::TcpPortForwarder::new(&self.src_addr, &self.dst_addr);
            self.conn_cnt.replace(p.get_conn_cnt());
            self.start_portal(p);
        } else {
            let p = tiny_portal::UdpPortForwarder::new(&self.src_addr, &self.dst_addr);
            self.conn_cnt.replace(p.get_conn_cnt());
            self.start_portal(p);
        };
    }

    fn start_portal(&mut self, p: impl tiny_portal::Portal + Send + 'static) {
        let j = self.rt.0.spawn(async move {
            let res = p.start().await;
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

    fn stop(&mut self) {
        if let Some(f) = self.stop_portal.take() {
            log::info!("Stopping port forwarder");
            f.abort();
            self.conn_cnt = None
        }
    }
}

impl eframe::App for MyApp {
    fn save(&mut self, storage: &mut dyn Storage) {
        log::info!("Saving state");
        storage.set_string(SAVE_KEY, ron::to_string(self).unwrap());
    }

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
                    if self.stop_portal.is_some() {
                        log::info!("Stopping port forwarder");
                        self.stop();
                    } else {
                        log::info!("Starting port forwarder");
                        self.start();
                    }
                }

                if let Some(cnt) = &self.conn_cnt {
                    ui.separator();
                    ui.label(format!(
                        "Connections: {}",
                        cnt.load(std::sync::atomic::Ordering::Relaxed)
                    ));
                }
            });
        });
    }
}
