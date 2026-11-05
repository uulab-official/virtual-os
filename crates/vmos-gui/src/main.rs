use eframe::egui;
use std::process::Command;
use std::time::Duration;
use vmos_client::{Client, VmListResult};
use vmos_core::VmStore;

struct VmosApp {
    client: Option<Client>,
    vms: Vec<vmos_core::VmSummary>,
    status: String,
    create_open: bool,
    new_name: String,
    new_cpus: u32,
    new_memory: u64,
    new_boot: String,
}

impl VmosApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            client: None,
            vms: Vec::new(),
            status: "connecting...".into(),
            create_open: false,
            new_name: "new-vm".into(),
            new_cpus: 2,
            new_memory: 2048,
            new_boot: "efi".into(),
        };
        app.connect();
        app
    }

    fn connect(&mut self) {
        let socket = VmStore::default_root().join("daemon.sock");
        if let Some(client) = Client::connect_if_running(&socket) {
            self.client = Some(client);
            self.status = "connected".into();
            self.refresh();
            return;
        }
        // try auto-start
        let _ = Command::new("vmos").arg("daemon").arg("start").status();
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if let Some(client) = Client::connect_if_running(&socket) {
                self.client = Some(client);
                self.status = "connected".into();
                self.refresh();
                return;
            }
        }
        self.status = "daemon not running (run: vmos daemon start)".into();
    }

    fn refresh(&mut self) {
        if let Some(client) = &mut self.client {
            match client.call::<_, VmListResult>("vm.list", &vmos_client::Empty {}) {
                Ok(result) => {
                    self.vms = result.vms;
                    self.status = format!("{} VMs", self.vms.len());
                }
                Err(err) => self.status = format!("error: {err}"),
            }
        }
    }

    fn action(&mut self, method: &str, name: &str) {
        if let Some(client) = &mut self.client {
            let params = vmos_client::VmNameParams { name: name.to_string() };
            let res: Result<serde_json::Value, _> = if method == "vm.start" {
                client.call_long::<_, serde_json::Value>(method, &params)
            } else {
                client.call::<_, serde_json::Value>(method, &params)
            };
            match res {
                Ok(_) => self.status = format!("{method} ok"),
                Err(err) => self.status = format!("{method} error: {err}"),
            }
            self.refresh();
        }
    }
}

impl eframe::App for VmosApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("virtual-os");
                if ui.button("Refresh").clicked() {
                    self.refresh();
                }
                if ui.button("New VM").clicked() {
                    self.create_open = true;
                }
                ui.label(&self.status);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for vm in self.vms.clone() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("{} ({})", vm.name, vm.state));
                            if ui.button("Start").clicked() {
                                self.action("vm.start", &vm.name);
                            }
                            if ui.button("Stop").clicked() {
                                self.action("vm.stop", &vm.name);
                            }
                            if ui.button("Pause").clicked() {
                                self.action("vm.pause", &vm.name);
                            }
                            if ui.button("Resume").clicked() {
                                self.action("vm.resume", &vm.name);
                            }
                            if ui.button("Delete").clicked() {
                                let params = vmos_client::VmDeleteParams {
                                    name: vm.name.clone(),
                                    force: true,
                                };
                                if let Some(client) = &mut self.client {
                                    let _ = client.call::<_, serde_json::Value>("vm.delete", &params);
                                }
                                self.refresh();
                            }
                        });
                        ui.label(format!("cpus: {}  memory: {}  boot: {}", vm.cpus, vm.memory_mib, vm.boot_kind));
                    });
                }
            });
        });

        if self.create_open {
            egui::Window::new("Create VM").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.new_name);
                });
                ui.horizontal(|ui| {
                    ui.label("CPUs:");
                    ui.add(egui::DragValue::new(&mut self.new_cpus).range(1..=64));
                });
                ui.horizontal(|ui| {
                    ui.label("Memory (MiB):");
                    ui.add(egui::DragValue::new(&mut self.new_memory).range(128..=131072));
                });
                ui.horizontal(|ui| {
                    ui.label("Boot:");
                    ui.text_edit_singleline(&mut self.new_boot);
                });
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        let boot = match self.new_boot.as_str() {
                            "linux" => vmos_core::BootConfig::Efi {
                                variable_store: std::path::PathBuf::from("nvram/efi-vars.fd"),
                            },
                            _ => vmos_core::BootConfig::Efi {
                                variable_store: std::path::PathBuf::from("nvram/efi-vars.fd"),
                            },
                        };
                        let params = vmos_client::VmCreateParams::new(
                            self.new_name.clone(),
                            boot,
                            self.new_cpus,
                            self.new_memory,
                        );
                        if let Some(client) = &mut self.client {
                            match client.call::<_, vmos_client::VmGetResult>("vm.create", &params) {
                                Ok(_) => self.status = "created".into(),
                                Err(e) => self.status = format!("create error: {e}"),
                            }
                        }
                        self.create_open = false;
                        self.refresh();
                    }
                    if ui.button("Cancel").clicked() {
                        self.create_open = false;
                    }
                });
            });
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "virtual-os",
        options,
        Box::new(|cc| Ok(Box::new(VmosApp::new(cc)))),
    )
}
