//! Application shell: routes views, runs actions, talks to the engine thread.
//!
//! Views never touch the Engine. They read `Shared` (the latest typed `State`
//! published by the engine thread) and queue `Act`s; the shell turns them into
//! `Job`s after the frame, so I/O never blocks drawing. Mutations go through
//! `Engine::handle`, the same RPC the Tauri app uses, so every rule of the core
//! is enforced in one place.

use crate::theme::{self, Pal};
use crate::views;
use crate::worker::{Job, Reply, Worker};
use __CORE__::{Engine, State};
use eframe::egui::{self, Color32, Key, RichText};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Main,
    About,
}

/// Sidebar order = ⌘/Ctrl + 1…N.
pub const NAV: &[(View, &str, &str)] = &[(View::Main, "◎", "Главная"), (View::About, "ℹ", "О программе")];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Ok,
    Err,
    Info,
}

/// Something a view asks the shell to do after the frame.
pub enum Act {
    Rpc(&'static str, Value),
    Go(View),
    Toast(Tone, String),
    Export,
    Import,
    Theme(bool),
}

/// What every view may read, plus the action queue.
pub struct Shared {
    pub st: State,
    pub pal: Pal,
    pub view: View,
    /// The engine thread has queued or running jobs.
    pub busy: bool,
    acts: RefCell<Vec<Act>>,
}

impl Shared {
    pub fn act(&self, a: Act) {
        self.acts.borrow_mut().push(a);
    }
    pub fn rpc(&self, cmd: &'static str, args: Value) {
        self.act(Act::Rpc(cmd, args));
    }
    pub fn set_field(&self, name: &str, value: u32) {
        self.rpc("set_field", json!({ "name": name, "value": value }));
    }
    pub fn toast(&self, t: Tone, s: impl Into<String>) {
        self.act(Act::Toast(t, s.into()));
    }
}

/// Per-view UI state that outlives a frame.
#[derive(Default)]
pub struct ViewState {
    pub main: views::main::MainState,
}

struct Toast {
    tone: Tone,
    text: String,
    until: Instant,
}

pub struct App {
    worker: Worker,
    sh: Shared,
    vs: ViewState,
    toasts: Vec<Toast>,
    shots: Option<crate::shots::Shots>,
}

/// Same file as the Tauri app (`app_config_dir()/<file>`), so both front-ends
/// share settings. `ID` = `identifier` from tauri.conf.json.
pub fn config_path() -> Option<PathBuf> {
    const ID: &str = "__TAURI_ID__";
    const FILE: &str = "__CONFIG_FILE__";
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let dir = if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library/Application Support").join(ID))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(ID))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or(home.map(|h| h.join(".config"))).map(|c| c.join(ID))
    };
    dir.map(|d| d.join(FILE))
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let dark = cc.storage.and_then(|s| s.get_string("theme")).map(|t| t != "light").unwrap_or(true);
        let pal = Pal::get(dark);
        theme::apply(&cc.egui_ctx, &pal);
        // adapt to the core's constructor
        let (worker, st) = Worker::spawn(Engine::new(config_path()), cc.egui_ctx.clone());
        App {
            worker,
            sh: Shared { st, pal, view: View::Main, busy: false, acts: RefCell::new(vec![]) },
            vs: ViewState::default(),
            toasts: vec![],
            shots: crate::shots::Shots::from_env(),
        }
    }

    /// Take what the engine thread published since the last frame.
    fn refresh(&mut self) {
        let u = self.worker.take();
        if let Some(st) = u.state {
            self.sh.st = st;
        }
        for r in u.replies {
            match r {
                Reply::Toast(t, s) => self.toast(t, s),
                Reply::ExportText(text) => self.save_export(text),
            }
        }
        self.sh.busy = self.worker.busy();
    }

    fn send(&mut self, job: Job) {
        if !self.worker.send(job) {
            self.toast(Tone::Err, "Поток движка остановлен — перезапустите приложение");
        }
    }

    fn save_export(&mut self, text: String) {
        // dialogs stay on the UI thread (macOS requires it)
        if let Some(path) = rfd::FileDialog::new().set_file_name("config.txt").save_file() {
            match std::fs::write(&path, text) {
                Ok(()) => self.toast(Tone::Ok, format!("Сохранено: {}", path.display())),
                Err(e) => self.toast(Tone::Err, format!("{}: {}", path.display(), e)),
            }
        }
    }

    fn toast(&mut self, tone: Tone, text: impl Into<String>) {
        let ms = if tone == Tone::Err { 6500 } else { 3200 };
        self.toasts.push(Toast { tone, text: text.into(), until: Instant::now() + Duration::from_millis(ms) });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    fn run(&mut self, ctx: &egui::Context, a: Act) {
        match a {
            Act::Rpc(cmd, args) => self.send(Job::Rpc { cmd, args, ok: None }),
            Act::Go(v) => self.sh.view = v,
            Act::Toast(t, s) => self.toast(t, s),
            Act::Export => self.send(Job::Export),
            Act::Import => {
                let Some(path) = rfd::FileDialog::new().pick_file() else { return };
                match std::fs::read_to_string(&path) {
                    Ok(text) => self.send(Job::Rpc {
                        cmd: "import_config",
                        args: json!({ "text": text }),
                        ok: Some("Загружено".into()),
                    }),
                    Err(e) => self.toast(Tone::Err, format!("{}: {}", path.display(), e)),
                }
            }
            Act::Theme(dark) => {
                self.sh.pal = Pal::get(dark);
                theme::apply(ctx, &self.sh.pal);
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        const KEYS: [Key; 9] =
            [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
        ctx.input_mut(|i| {
            for (n, k) in KEYS.iter().enumerate().take(NAV.len()) {
                if i.consume_key(egui::Modifiers::COMMAND, *k) {
                    self.sh.view = NAV[n].0;
                }
            }
        });
    }

    fn show_toasts(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.toasts.retain(|t| t.until > now);
        if self.toasts.is_empty() {
            return;
        }
        let p = self.sh.pal;
        egui::Area::new("toasts".into()).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0)).show(ctx, |ui| {
            ui.set_max_width(420.0);
            for t in &self.toasts {
                let c = match t.tone {
                    Tone::Ok => p.ok,
                    Tone::Err => p.err,
                    Tone::Info => p.accent,
                };
                egui::Frame::new()
                    .fill(p.panel)
                    .stroke(egui::Stroke::new(1.0, c))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("●").color(c));
                            ui.add(egui::Label::new(RichText::new(&t.text).color(p.fg)).wrap());
                        });
                    });
                ui.add_space(6.0);
            }
        });
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn sidebar(ctx: &egui::Context, sh: &Shared) {
    egui::SidePanel::left("nav").resizable(false).exact_width(200.0).show(ctx, |ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("__TITLE__").strong().size(15.0));
        ui.add_space(10.0);
        for (v, icon, label) in NAV {
            let sel = sh.view == *v;
            let text = RichText::new(format!("{}  {}", icon, label)).color(if sel { sh.pal.accent } else { sh.pal.fg });
            if ui.selectable_label(sel, text).clicked() {
                sh.act(Act::Go(*v));
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            let dark = sh.pal.dark;
            if ui.button(if dark { "☀ Светлая тема" } else { "☾ Тёмная тема" }).clicked() {
                sh.act(Act::Theme(!dark));
            }
            if sh.busy {
                ui.label(RichText::new("обмен…").color(Color32::GRAY));
            }
        });
    });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.refresh();
        self.shortcuts(ctx);
        if let Some((view, dark, _)) = self.shots.as_ref().and_then(|s| s.current()) {
            self.sh.view = view;
            if dark != self.sh.pal.dark {
                self.sh.pal = Pal::get(dark);
                theme::apply(ctx, &self.sh.pal);
            }
        }

        let sh = &self.sh;
        let vs = &mut self.vs;
        sidebar(ctx, sh);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(sh.pal.bg).inner_margin(20)).show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt(("view", sh.view as u8)).auto_shrink(false).show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(1600.0));
                match sh.view {
                    View::Main => views::main::show(ui, sh, &mut vs.main),
                    View::About => views::about::show(ui, sh),
                }
            });
        });
        self.show_toasts(ctx);

        // after the frame: run what the views queued
        let acts: Vec<Act> = self.sh.acts.borrow_mut().drain(..).collect();
        if !acts.is_empty() {
            for a in acts {
                self.run(ctx, a);
            }
            ctx.request_repaint();
        }
        if let Some(s) = self.shots.as_mut() {
            s.tick(ctx);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("theme", if self.sh.pal.dark { "dark".into() } else { "light".into() });
    }
}
