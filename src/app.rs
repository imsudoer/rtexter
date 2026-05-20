use crate::engine::{self, EngineConfig};
use crate::macros::MacroRecorder;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct HTexterApp {
    script: String,
    start_delay_ms: u64,
    line_delay_ms: u64,
    write_delay_ms: u64,
    repeat: bool,
    press_enter: bool,

    running: Arc<AtomicBool>,
    progress: Arc<Mutex<(usize, usize)>>,

    recorder: MacroRecorder,
    macro_speed: f32,

    hotkey_start: String,
    hotkey_stop: String,
    hotkey_signal: Arc<Mutex<Option<HotkeyAction>>>,
    hotkeys_installed: bool,

    show_help: bool,
    status: String,
}

#[derive(Copy, Clone, Debug)]
enum HotkeyAction {
    Start,
    Stop,
}

impl HTexterApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let app = Self {
            script: "".to_string(),
            start_delay_ms: 3000,
            line_delay_ms: 100,
            write_delay_ms: 20,
            repeat: false,
            press_enter: true,
            running: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(Mutex::new((0, 0))),
            recorder: MacroRecorder::new(),
            macro_speed: 1.0,
            hotkey_start: "F6".to_string(),
            hotkey_stop: "F7".to_string(),
            hotkey_signal: Arc::new(Mutex::new(None)),
            hotkeys_installed: false,
            show_help: false,
            status: "Готов".to_string(),
        };
        app
    }

    fn install_hotkeys(&mut self) {
        if self.hotkeys_installed {
            return;
        }
        self.hotkeys_installed = true;

        let signal = self.hotkey_signal.clone();
        let start_name = self.hotkey_start.clone();
        let stop_name = self.hotkey_stop.clone();

        thread::spawn(move || {
            let _ = rdev::listen(move |event| {
                if let rdev::EventType::KeyPress(k) = event.event_type {
                    let name = format!("{:?}", k);
                    if name == start_name {
                        *signal.lock().unwrap() = Some(HotkeyAction::Start);
                    } else if name == stop_name {
                        *signal.lock().unwrap() = Some(HotkeyAction::Stop);
                    }
                }
            });
        });
    }

    fn start_execution(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        self.running.store(true, Ordering::SeqCst);
        *self.progress.lock().unwrap() = (0, 0);

        let commands = engine::parse_script(&self.script);
        let cfg = EngineConfig {
            start_delay_ms: self.start_delay_ms,
            line_delay_ms: self.line_delay_ms,
            write_delay_ms: self.write_delay_ms,
            repeat: self.repeat,
            press_enter: self.press_enter,
        };
        let running = self.running.clone();
        let progress = self.progress.clone();

        thread::spawn(move || {
            engine::run(commands, cfg, running, progress);
        });

        self.status = format!("Start delayed by {} ms", self.start_delay_ms);
    }

    fn stop_execution(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.status = "Stopped".to_string();
    }
}

impl eframe::App for HTexterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.hotkeys_installed {
            self.install_hotkeys();
        }

        let action = self.hotkey_signal.lock().unwrap().take();
        if let Some(a) = action {
            match a {
                HotkeyAction::Start => self.start_execution(),
                HotkeyAction::Stop => self.stop_execution(),
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        // tpan
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Load script…").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Text", &["txt", "rtscript"])
                            .pick_file()
                        {
                            if let Ok(s) = std::fs::read_to_string(&path) {
                                self.script = s;
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Save script…").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Text", &["txt", "rtscript"])
                            .save_file()
                        {
                            let _ = std::fs::write(&path, &self.script);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("Support by commands").clicked() {
                        self.show_help = true;
                        ui.close_menu();
                    }
                });

                ui.separator();
                ui.label(format!("Status: {}", self.status));
            });
        });

        // botpan
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (cur, total) = *self.progress.lock().unwrap();
                let pct = if total > 0 {
                    cur as f32 / total as f32
                } else {
                    0.0
                };
                ui.add(
                    egui::ProgressBar::new(pct)
                        .text(format!("{}/{}", cur, total))
                        .desired_width(300.0),
                );
                ui.separator();
                if self.running.load(Ordering::SeqCst) {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, "● Executing");
                } else {
                    ui.colored_label(egui::Color32::LIGHT_GRAY, "○ Waiting");
                }
            });
        });

        // lpan
        egui::SidePanel::left("settings")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.add_space(8.0);

                egui::CollapsingHeader::new("Timings")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label("Delay before start (ms):");
                        ui.add(egui::Slider::new(&mut self.start_delay_ms, 0..=10000));

                        ui.label("Delay between lines (ms):");
                        ui.add(egui::Slider::new(&mut self.line_delay_ms, 0..=3000));

                        ui.label("Delay between symbols (ms):");
                        ui.add(egui::Slider::new(&mut self.write_delay_ms, 0..=200));

                        let cps = if self.write_delay_ms > 0 {
                            1000.0 / self.write_delay_ms as f32
                        } else {
                            f32::INFINITY
                        };
                        ui.label(format!("≈ {:.0} sym/sec", cps));
                    });

                egui::CollapsingHeader::new("Behavior")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(&mut self.repeat, "Autorepeat");
                        ui.checkbox(&mut self.press_enter, "Press Enter before and after");
                    });

                egui::CollapsingHeader::new("Global hotkeys")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Старт:");
                            ui.label(&self.hotkey_start);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Стоп:");
                            ui.label(&self.hotkey_stop);
                        });
                        ui.label("(apply on restart)");
                    });

                ui.add_space(8.0);
                ui.separator();
                ui.heading("Controls");

                ui.horizontal(|ui| {
                    let can_start = !self.running.load(Ordering::SeqCst);
                    if ui
                        .add_enabled(can_start, egui::Button::new("▶ Start"))
                        .clicked()
                    {
                        self.start_execution();
                    }
                    if ui.button("■ Stop").clicked() {
                        self.stop_execution();
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.heading("Macros");

                let is_rec = self.recorder.recording.load(Ordering::SeqCst);
                ui.horizontal(|ui| {
                    if !is_rec {
                        if ui.button("• Record").clicked() {
                            self.recorder.start();
                            self.status =
                                format!("Macro recording (steps: {})", self.recorder.stop_key);
                        }
                    } else {
                        if ui.add(egui::Button::new("■ Stop record")).clicked() {
                            self.recorder.stop();
                        }
                    }
                    if ui.button("▶ Play").clicked() {
                        self.recorder.play(self.macro_speed);
                    }
                });

                ui.label("Playback speed:");
                ui.add(egui::Slider::new(&mut self.macro_speed, 0.1..=5.0));

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("Macros", &["rtmacro"])
                            .save_file()
                        {
                            let _ = self.recorder.save(&p);
                        }
                    }
                    if ui.button("📂 Load").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("Macros", &["rtmacro"])
                            .pick_file()
                        {
                            let _ = self.recorder.load(&p);
                        }
                    }
                });

                let count = self.recorder.events.lock().unwrap().len();
                ui.label(format!("Macro events: {}", count));
            });

        // redactor
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Script");
            ui.label("Every string is a new iteration. Use # for script commands");
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut self.script)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_rows(20),
                    );
                });
        });

        if self.show_help {
            let mut open = true;
            egui::Window::new("Script commands")
                .open(&mut open)
                .default_size([500.0, 400.0])
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("Directives (startswith #):").strong());
                    ui.add_space(6.0);
                    let help = r#"
# MANUAL_SLEEP <ms>      - sleep before start
# MANUAL_DELAY <ms>      - delay between lines
# MANUAL_WRITE_DELAY <ms>- delay between keys
# key <name> [click|press|release]
                          - send key
                          (enter, tab, space, esc, f1..f12, a..z, etc.)
# type "<text>" <ms>    - type any text
# pause <ms>             - pause

Example:
    # MANUAL_SLEEP 2000
    Hello
    # key enter
    World
    # pause 500
    # type "ABC" 10
"#;
                    ui.monospace(help);
                });
            self.show_help = open;
        }
    }
}
