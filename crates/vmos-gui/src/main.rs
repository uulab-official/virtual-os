use eframe::egui;
use std::process::Command;
use std::time::{Duration, Instant};
use vmos_client::{Client, VmGetResult, VmListResult};
use vmos_core::VmStore;

enum Pending {
    None,
    Start(String),
    Stop(String),
    Pause(String),
    Resume(String),
    Delete(String),
    RefreshLogs,
}

struct VmosApp {
    client: Option<Client>,
    vms: Vec<vmos_core::VmSummary>,
    selected: Option<String>,
    details: Option<VmGetResult>,
    status: String,
    last_refresh: Instant,
    create_open: bool,
    snap_open: bool,
    new_name: String,
    new_cpus: u32,
    new_memory: u64,
    new_boot: String,
    new_disk: String,
    snap_name: String,
    logs: Vec<String>,
    events: Vec<String>,
}

impl VmosApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            client: None,
            vms: Vec::new(),
            selected: None,
            details: None,
            status: "connecting...".into(),
            last_refresh: Instant::now(),
            create_open: false,
            snap_open: false,
            new_name: "new-vm".into(),
            new_cpus: 2,
            new_memory: 2048,
            new_boot: "efi".into(),
            new_disk: "root:32M".into(),
            snap_name: "snapshot-1".into(),
            logs: Vec::new(),
            events: Vec::new(),
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
        self.status = "daemon not running".into();
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
            if let Some(name) = &self.selected {
                let params = vmos_client::VmNameParams { name: name.clone() };
                if let Ok(res) = client.call::<_, VmGetResult>("vm.get", &params) {
                    self.details = Some(res);
                }
                let params = vmos_client::LogsParams { name: name.clone(), lines: 50 };
                if let Ok(res) = client.call::<_, vmos_client::LogsResult>("vm.logs", &params) {
                    self.logs = res.lines;
                }
            }
            if let Ok(res) = client.call::<_, vmos_client::EventPollResult>(
                "events.poll",
                &vmos_client::EventPollParams { after: None, limit: Some(20) },
            ) {
                self.events = res.events.iter().map(|e| format!("{} {}", e.seq, serde_json::to_string(&e.kind).unwrap_or_default())).collect();
            }
        }
    }
}

impl eframe::App for VmosApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut pending = Pending::None;

        if self.last_refresh.elapsed() > Duration::from_secs(2) {
            self.refresh();
            self.last_refresh = Instant::now();
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("virtual-os");
                if ui.button("Refresh").clicked() { pending = Pending::RefreshLogs; }
                if ui.button("New VM").clicked() { self.create_open = true; }
                ui.label(&self.status);
            });
        });

        egui::SidePanel::left("vms").min_width(220.0).show(ctx, |ui| {
            ui.heading("Virtual Machines");
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut to_select = None;
                for vm in self.vms.clone() {
                    let selected = self.selected.as_deref() == Some(&vm.name);
                    if ui.selectable_label(selected, format!("{} [{}]", vm.name, vm.state)).clicked() {
                        to_select = Some(vm.name.clone());
                    }
                }
                if let Some(name) = to_select {
                    self.selected = Some(name);
                    self.refresh();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(details) = &self.details {
                let name = details.vm.name.clone();
                ui.heading(&name);
                ui.label(format!("state: {}  cpus: {}  memory: {} MiB", details.vm.state, details.vm.config.cpus, details.vm.config.memory_mib));
                ui.label(format!("boot: {:?}", details.vm.config.boot));
                ui.label(format!("dir: {}", details.vm.dir.display()));
                ui.horizontal(|ui| {
                    if ui.button("Start").clicked() { pending = Pending::Start(name.clone()); }
                    if ui.button("Stop").clicked() { pending = Pending::Stop(name.clone()); }
                    if ui.button("Pause").clicked() { pending = Pending::Pause(name.clone()); }
                    if ui.button("Resume").clicked() { pending = Pending::Resume(name.clone()); }
                    if ui.button("Delete").clicked() { pending = Pending::Delete(name.clone()); }
                    if ui.button("Snapshot").clicked() { self.snap_open = true; }
                });
                ui.separator();
                ui.label("Disks:");
                for disk in &details.vm.disks {
                    ui.label(format!("  {} - {} bytes", disk.id, disk.size_bytes));
                }
                ui.label("Snapshots:");
                for snap in &details.vm.snapshots {
                    ui.label(format!("  {} ({})", snap.name, snap.created_at));
                }
                ui.separator();
                ui.label("Serial log (tail):");
                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                    for line in &self.logs { ui.monospace(line); }
                });
                ui.separator();
                ui.label("Events:");
                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                    for e in &self.events { ui.monospace(e); }
                });
            } else {
                ui.label("Select a VM from the left panel.");
            }
        });

        // execute pending
        match pending {
            Pending::None => {}
            Pending::RefreshLogs => self.refresh(),
            Pending::Start(n) => self.action("vm.start", &n),
            Pending::Stop(n) => self.action("vm.stop", &n),
            Pending::Pause(n) => self.action("vm.pause", &n),
            Pending::Resume(n) => self.action("vm.resume", &n),
            Pending::Delete(n) => {
                let params = vmos_client::VmDeleteParams { name: n.clone(), force: true };
                if let Some(client) = &mut self.client {
                    let _ = client.call::<_, serde_json::Value>("vm.delete", &params);
                }
                self.selected = None;
                self.details = None;
                self.refresh();
            }
        }

        if self.create_open {
            egui::Window::new("Create VM").show(ctx, |ui| {
                ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut self.new_name); });
                ui.horizontal(|ui| { ui.label("CPUs:"); ui.add(egui::DragValue::new(&mut self.new_cpus).range(1..=64)); });
                ui.horizontal(|ui| { ui.label("Memory (MiB):"); ui.add(egui::DragValue::new(&mut self.new_memory).range(128..=131072)); });
                ui.horizontal(|ui| { ui.label("Boot:"); ui.text_edit_singleline(&mut self.new_boot); });
                ui.horizontal(|ui| { ui.label("Disk:"); ui.text_edit_singleline(&mut self.new_disk); });
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        let boot = vmos_core::BootConfig::Efi { variable_store: std::path::PathBuf::from("nvram/efi-vars.fd") };
                        let mut params = vmos_client::VmCreateParams::new(self.new_name.clone(), boot, self.new_cpus, self.new_memory);
                        let parts: Vec<&str> = self.new_disk.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            if let Ok(size) = vmos_core::util::parse_size(parts[1]) {
                                params.disks.push(vmos_client::DiskSpec { id: parts[0].to_string(), size_bytes: size, path: None });
                            }
                        }
                        if let Some(client) = &mut self.client {
                            match client.call::<_, vmos_client::VmGetResult>("vm.create", &params) {
                                Ok(_) => self.status = "created".into(),
                                Err(e) => self.status = format!("create error: {e}"),
                            }
                        }
                        self.create_open = false;
                        self.refresh();
                    }
                    if ui.button("Cancel").clicked() { self.create_open = false; }
                });
            });
        }

        if self.snap_open {
            if let Some(name) = self.selected.clone() {
                egui::Window::new("Snapshot").show(ctx, |ui| {
                    ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut self.snap_name); });
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            let params = vmos_client::SnapshotCreateParams { vm: name.clone(), name: self.snap_name.clone(), description: None };
                            if let Some(client) = &mut self.client {
                                let _ = client.call_long::<_, serde_json::Value>("snapshot.create", &params);
                            }
                            self.snap_open = false;
                            self.refresh();
                        }
                        if ui.button("Cancel").clicked() { self.snap_open = false; }
                    });
                });
            } else {
                self.snap_open = false;
            }
        }
    }
}

impl VmosApp {
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

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native("virtual-os", options, Box::new(|cc| Ok(Box::new(VmosApp::new(cc)))))
}
