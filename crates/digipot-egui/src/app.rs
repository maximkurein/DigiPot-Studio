//! Application shell: routes views, runs actions, talks to the engine thread.
//!
//! Views never touch the Engine. They read `Shared` (the latest typed `State`
//! published by the engine thread, see `worker.rs`) and queue `Act`s; the
//! shell turns them into `Job`s after the frame, so hardware I/O never blocks
//! drawing. Mutations go through `Engine::handle`, the same RPC the Tauri app
//! and the web server use, so every rule (EEPROM only via confirmed
//! `nv_write`, auto-apply only for volatile registers) is enforced in one place.

use crate::fmt::{eng, hex_w};
use crate::theme::{self, Pal};
use crate::views;
use crate::widgets as w;
use crate::worker::{Job, Reply, Worker};
use digipot_core::api::{State, Txn};
use digipot_core::regs::{self, REGS};
use digipot_core::transport::{PortInfo, UsbDev};
use digipot_core::{about, Engine};
use eframe::egui::{self, Align, Color32, Key, Layout, RichText};
use egui_extras::{Column, TableBuilder};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Pot,
    Overview,
    Memory,
    Registers,
    Board,
    Hardware,
    Learn,
    About,
}

/// Views of the app in sidebar order (⌘/Ctrl + 1…8): (view, icon, label, hint, group).
pub const NAV: [(View, &str, &str, &str, &str); 8] = [
    (View::Pot, "📈", "Потенциометр", "Движок, схема, кривые", "Устройство"),
    (View::Overview, "🔲", "Обзор", "Блок-схема кристалла", "Устройство"),
    (View::Memory, "💾", "EEPROM", "Память, включение питания", "Устройство"),
    (View::Registers, "🔢", "Регистры", "Биты, запись, чтение", "Устройство"),
    (View::Board, "📟", "Плата", "mikroBUS, выводы, питание", "Практика"),
    (View::Hardware, "🔌", "Подключение", "USB-мост, симулятор", "Практика"),
    (View::Learn, "🎓", "Академия", "Как работает кристалл", "Практика"),
    (View::About, "ℹ", "О программе", "Версия, автор, исходный код", "Программа"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Err,
    Info,
}

/// Something a view asks the shell to do after the frame.
pub enum Act {
    Rpc(&'static str, Value),
    Go(View, Option<Option<&'static str>>),
    SelectBlock(Option<&'static str>),
    Focus(Option<&'static str>),
    Toast(Tone, String),
    Export,
    Import,
    Raw(String, Option<u32>),
    ReadNv,
    RefreshPorts,
    ClearLog,
    Console(bool),
    Theme(bool),
}

/// What every view may read, plus the action queue.
pub struct Shared {
    pub st: State,
    pub log: Vec<Txn>,
    pub pal: Pal,
    pub view: View,
    pub block: Option<&'static str>,
    pub focus: Option<&'static str>,
    pub console_open: bool,
    /// Wall-clock seconds for animations.
    pub time: f64,
    /// The engine thread has queued or running jobs (bus I/O in progress).
    pub busy: bool,
    /// Busy for longer than a moment — show the indicator.
    pub busy_shown: bool,
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
    pub fn set_settings(&self, patch: Value) {
        self.rpc("set_settings", json!({ "patch": patch }));
    }
    pub fn go(&self, v: View) {
        self.act(Act::Go(v, None));
    }
    pub fn go_block(&self, v: View, b: Option<&'static str>) {
        self.act(Act::Go(v, Some(b)));
    }
    pub fn toast(&self, t: Tone, s: impl Into<String>) {
        self.act(Act::Toast(t, s.into()));
    }
    pub fn field(&self, name: &str) -> u32 {
        self.st.fields.get(name).copied().unwrap_or(0)
    }
    pub fn metric(&self, key: &str) -> Option<f64> {
        self.st.derived.metrics.iter().find(|m| m.key == key).map(|m| m.value)
    }
    pub fn is_sim(&self) -> bool {
        self.st.link.kind == "sim"
    }
}

/// Per-view UI state that outlives a frame.
#[derive(Default)]
pub struct ViewState {
    pub pot: views::pot::PotState,
    pub memory: views::memory::MemoryState,
    pub registers: views::registers::RegState,
    pub hardware: HwState,
    pub board: views::board::BoardState,
    pub learn: usize,
    pub console: ConsoleState,
}

#[derive(Default)]
pub struct HwState {
    pub ports: Vec<PortInfo>,
    pub usb: Option<Vec<UsbDev>>,
    pub port: String,
    pub baud: u32,
    pub loaded: bool,
}

#[derive(Default, PartialEq, Clone, Copy)]
pub enum LogFilter {
    #[default]
    All,
    Write,
    Read,
    Note,
}

#[derive(Default)]
pub struct ConsoleState {
    pub filter: LogFilter,
    pub q: String,
    pub raw: String,
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
    /// Since when the worker has been busy (for the top-bar indicator).
    busy_since: Option<Instant>,
    t0: Instant,
    shots: Option<crate::shots::Shots>,
}

/// Same file as the Tauri app (`app_config_dir()/digipot-studio.json`), so
/// both front-ends share the board settings and the last configuration.
fn config_path() -> Option<PathBuf> {
    const ID: &str = "com.digipotstudio.desktop";
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let dir = if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library/Application Support").join(ID))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(ID))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or(home.map(|h| h.join(".config"))).map(|c| c.join(ID))
    };
    dir.map(|d| d.join("digipot-studio.json"))
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let get = |k: &str| cc.storage.and_then(|s| s.get_string(k));
        let dark = get("theme").map(|t| t != "light").unwrap_or(true);
        let console_open = get("console").map(|c| c == "1").unwrap_or(false);
        let pal = Pal::get(dark);
        theme::apply(&cc.egui_ctx, &pal);
        let (worker, st, log) = Worker::spawn(Engine::new(config_path()), cc.egui_ctx.clone());
        App {
            worker,
            sh: Shared {
                st,
                log,
                pal,
                view: View::Pot,
                block: None,
                focus: None,
                console_open,
                time: 0.0,
                busy: false,
                busy_shown: false,
                acts: RefCell::new(vec![]),
            },
            vs: ViewState { hardware: HwState { baud: 115_200, ..Default::default() }, ..Default::default() },
            toasts: vec![],
            busy_since: None,
            t0: Instant::now(),
            shots: crate::shots::Shots::from_env(),
        }
    }

    /// Take what the engine thread published since the last frame.
    fn refresh(&mut self, ctx: &egui::Context) {
        let u = self.worker.take();
        if let Some(st) = u.state {
            self.sh.st = st;
        }
        if u.log_reset {
            self.sh.log.clear();
        }
        self.sh.log.extend(u.log);
        let n = self.sh.log.len();
        if n > 2000 {
            self.sh.log.drain(..n - 2000);
        }
        for r in u.replies {
            match r {
                Reply::Toast(t, s) => self.toast(t, s),
                Reply::ExportText(text) => self.save_export(text),
                Reply::Ports(ports, usb) => {
                    let hw = &mut self.vs.hardware;
                    hw.ports = ports;
                    if hw.port.is_empty() {
                        if let Some(p) = hw.ports.iter().find(|p| p.kind == "usb").or(hw.ports.first()) {
                            hw.port = p.path.clone();
                        }
                    }
                    hw.usb = Some(usb);
                }
            }
        }
        let busy = self.worker.busy();
        self.busy_since = if busy { self.busy_since.or(Some(Instant::now())) } else { None };
        self.sh.busy = busy;
        // show the indicator only for jobs that take noticeable time
        self.sh.busy_shown = self.busy_since.is_some_and(|t| t.elapsed() > Duration::from_millis(150));
        if busy && !self.sh.busy_shown {
            ctx.request_repaint_after(Duration::from_millis(160));
        }
    }

    fn send(&mut self, job: Job) {
        if !self.worker.send(job) {
            self.toast(Tone::Err, "Поток движка остановлен — перезапустите приложение");
        }
    }

    fn save_export(&mut self, text: String) {
        if let Some(path) =
            rfd::FileDialog::new().set_file_name("MCP4161_config.txt").add_filter("Текст", &["txt"]).save_file()
        {
            match std::fs::write(&path, text) {
                Ok(()) => self.toast(Tone::Ok, format!("Конфигурация сохранена: {}", path.display())),
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
            Act::Go(v, b) => {
                self.sh.view = v;
                if let Some(b) = b {
                    self.sh.block = b;
                }
            }
            Act::SelectBlock(b) => self.sh.block = b,
            Act::Focus(f) => self.sh.focus = f,
            Act::Toast(t, s) => self.toast(t, s),
            Act::Export => self.send(Job::Export),
            Act::Import => {
                // the dialog stays on the UI thread (macOS requires it)
                let Some(path) = rfd::FileDialog::new().add_filter("Текст", &["txt", "cfg", "csv"]).pick_file()
                else {
                    return;
                };
                match std::fs::read_to_string(&path) {
                    Ok(text) => self.send(Job::Rpc {
                        cmd: "import_config",
                        args: json!({ "text": text }),
                        ok: Some("Конфигурация загружена".into()),
                    }),
                    Err(e) => self.toast(Tone::Err, format!("{}: {}", path.display(), e)),
                }
            }
            Act::Raw(hex, drive) => self.send(Job::Raw { hex, drive }),
            Act::ReadNv => self.send(Job::ReadNv),
            Act::RefreshPorts => {
                self.vs.hardware.loaded = true;
                self.send(Job::Ports);
            }
            Act::ClearLog => self.send(Job::Rpc { cmd: "clear_log", args: json!({}), ok: None }),
            Act::Console(open) => self.sh.console_open = open,
            Act::Theme(dark) => {
                self.sh.pal = Pal::get(dark);
                theme::apply(ctx, &self.sh.pal);
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8];
        ctx.input_mut(|i| {
            for (n, k) in keys.iter().enumerate() {
                if i.consume_key(egui::Modifiers::COMMAND, *k) {
                    self.sh.view = NAV[n].0;
                }
            }
            if i.consume_key(egui::Modifiers::COMMAND, Key::J) {
                self.sh.console_open = !self.sh.console_open;
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sh.time = self.t0.elapsed().as_secs_f64();
        self.refresh(ctx);
        self.shortcuts(ctx);
        if let Some((view, dark, console)) = self.shots.as_ref().and_then(|s| s.current()) {
            self.sh.view = view;
            self.sh.console_open = console;
            if dark != self.sh.pal.dark {
                self.sh.pal = Pal::get(dark);
                theme::apply(ctx, &self.sh.pal);
            }
        }

        let sh = &self.sh;
        let vs = &mut self.vs;
        top_bar(ctx, sh);
        sidebar(ctx, sh);
        if sh.console_open {
            console(ctx, sh, &mut vs.console);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(sh.pal.bg).inner_margin(egui::Margin {
                left: 24,
                right: 24,
                top: 18,
                bottom: 18,
            }))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().id_salt(("view", sh.view as u8)).auto_shrink(false).show(ui, |ui| {
                    ui.set_max_width(ui.available_width().min(1600.0));
                    match sh.view {
                        View::Pot => views::pot::show(ui, sh, &mut vs.pot),
                        View::Overview => views::overview::show(ui, sh),
                        View::Memory => views::memory::show(ui, sh, &mut vs.memory),
                        View::Registers => views::registers::show(ui, sh, &mut vs.registers),
                        View::Board => views::board::show(ui, sh, &mut vs.board),
                        View::Hardware => views::hardware::show(ui, sh, &mut vs.hardware),
                        View::Learn => views::learn::show(ui, sh, &mut vs.learn),
                        View::About => views::about::show(ui, sh),
                    }
                });
            });
        self.show_toasts(ctx);

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
        // live simulator status: the engine thread repaints every 350 ms when it publishes
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("theme", if self.sh.pal.dark { "dark".into() } else { "light".into() });
        storage.set_string("console", if self.sh.console_open { "1".into() } else { "0".into() });
    }
}

impl App {
    fn show_toasts(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.toasts.retain(|t| t.until > now);
        if self.toasts.is_empty() {
            return;
        }
        let p = self.sh.pal;
        let mut dismiss = None;
        egui::Area::new("toasts".into()).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0)).show(ctx, |ui| {
            ui.set_max_width(420.0);
            for (i, t) in self.toasts.iter().enumerate() {
                let c = match t.tone {
                    Tone::Ok => p.ok,
                    Tone::Err => p.err,
                    Tone::Info => p.accent,
                };
                let r = egui::Frame::new()
                    .fill(p.panel)
                    .stroke(egui::Stroke::new(1.0, theme::mix(c, 45.0, p.panel)))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 18,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("●").color(c));
                            ui.add(egui::Label::new(RichText::new(&t.text).color(p.fg)).wrap());
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                if r.clicked() {
                    dismiss = Some(i);
                }
                ui.add_space(6.0);
            }
        });
        if let Some(i) = dismiss {
            self.toasts.remove(i);
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

/// Overall device state for the top-bar pill: (tone, text).
fn state_info(sh: &Shared) -> (Color32, String) {
    let p = &sh.pal;
    if sh.st.link_error.is_some() {
        return (p.err, "Ошибка связи".into());
    }
    match &sh.st.sim {
        None => (p.ok, "Плата подключена".into()),
        Some(s) => {
            let c = match s.state {
                "ok" => p.ok,
                "busy" | "warn" => p.warn,
                "fault" => p.err,
                _ => p.muted,
            };
            (c, s.text.clone())
        }
    }
}

fn top_bar(ctx: &egui::Context, sh: &Shared) {
    let p = sh.pal;
    egui::TopBottomPanel::top("top")
        .exact_height(52.0)
        .frame(
            egui::Frame::new()
                .fill(p.bg2)
                .inner_margin(egui::Margin::symmetric(14, 0))
                .stroke(egui::Stroke::new(1.0, p.line)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // logo
                let (r, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 8.0, p.accent);
                ui.painter().text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "Ω",
                    egui::FontId::proportional(15.0),
                    p.accent_fg,
                );
                ui.vertical(|ui| {
                    ui.add_space(9.0);
                    ui.label(RichText::new("DigiPot Studio").strong().size(13.0));
                    ui.label(
                        RichText::new("Microchip MCP4161 · DIGI POT click (MIKROE-923)").size(10.5).color(p.muted),
                    );
                });
                ui.add_space(6.0);
                ui.separator();

                // top metrics (hidden when the window is narrow)
                if ui.available_width() > 900.0 {
                    for m in sh.st.derived.metrics.iter().filter(|m| m.top).take(5) {
                        let tone = crate::content::block(m.block).map(|b| p.tone(b.tone)).unwrap_or(p.fg);
                        let r = ui
                            .vertical(|ui| {
                                ui.add_space(9.0);
                                ui.label(RichText::new(m.label.to_uppercase()).size(9.5).color(p.muted));
                                ui.label(RichText::new(eng(m.value, m.unit, 4)).size(13.5).strong().color(tone));
                            })
                            .response
                            .interact(egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if r.clicked() {
                            sh.go_block(View::Overview, Some(m.block));
                        }
                        ui.add_space(4.0);
                    }
                }

                if sh.busy_shown {
                    ui.add(egui::Spinner::new().size(14.0).color(p.accent))
                        .on_hover_text("Идёт обмен с кристаллом — интерфейс не ждёт его окончания");
                }
                let (tone, text) = state_info(sh);
                if w::pill(ui, &p, &text, tone, true).on_hover_text("Состояние кристалла").clicked() {
                    sh.go(View::Overview);
                }
                let errors = sh.st.derived.issues.iter().filter(|i| i.level == "error").count();
                if errors > 0 {
                    let t = format!("{} {} конфигурации", errors, if errors == 1 { "ошибка" } else { "ошибки" });
                    if w::pill(ui, &p, &t, p.err, false).clicked() {
                        sh.go(View::Overview);
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let theme_icon = if p.dark { "☀" } else { "🌙" };
                    if w::icon_button(ui, &p, theme_icon, false).on_hover_text("Тема оформления").clicked()
                    {
                        sh.act(Act::Theme(!p.dark));
                    }
                    if w::icon_button(ui, &p, "ℹ", sh.view == View::About)
                        .on_hover_text(format!("О программе — {} v{}, {}", about::NAME, about::VERSION, about::AUTHOR))
                        .clicked()
                    {
                        sh.go(View::About);
                    }
                    if w::icon_button(ui, &p, "🖥", sh.console_open).on_hover_text("Журнал шины (⌘/Ctrl + J)").clicked()
                    {
                        sh.act(Act::Console(!sh.console_open));
                    }
                    if w::icon_button(ui, &p, "⟲", false).on_hover_text("Прочитать все регистры из кристалла").clicked()
                    {
                        sh.rpc("read_all", json!({}));
                    }
                    let dirty = sh.st.dirty.iter().filter(|d| **d).count();
                    let label =
                        if dirty > 0 { format!("Записать ({})", dirty) } else { "Записать".into() };
                    let kind = if dirty > 0 { w::Btn::Primary } else { w::Btn::Subtle };
                    if ui
                        .add_enabled(dirty > 0, w::btn(&p, &label, kind))
                        .on_hover_text("Записать изменённые регистры")
                        .clicked()
                    {
                        sh.rpc("apply", json!({}));
                    }
                    let mut on = sh.st.auto_apply;
                    let r = w::toggle(ui, &p, &mut on)
                        .on_hover_text("Автоматически записывать изменённые регистры в кристалл");
                    if r.changed() {
                        sh.rpc("set_auto_apply", json!({ "on": on }));
                    }
                    ui.label(RichText::new("Авто-запись").size(12.0).color(p.fg2));
                    let (c, t) = if sh.st.link_error.is_some() {
                        (p.err, "Ошибка связи".to_string())
                    } else if sh.is_sim() {
                        (p.refc, "Симулятор".to_string())
                    } else {
                        (p.ok, sh.st.link.name.clone())
                    };
                    let tip = sh.st.link_error.clone().unwrap_or_else(|| sh.st.link.detail.clone());
                    if w::pill(ui, &p, &t, c, false).on_hover_text(tip).clicked() {
                        sh.go(View::Hardware);
                    }
                });
            });
        });
}

fn sidebar(ctx: &egui::Context, sh: &Shared) {
    let p = sh.pal;
    egui::SidePanel::left("nav")
        .exact_width(212.0)
        .resizable(false)
        .frame(egui::Frame::new().fill(p.bg2).inner_margin(10).stroke(egui::Stroke::new(1.0, p.line)))
        .show(ctx, |ui| {
            let mut group = "";
            for (i, (v, icon, label, hint, g)) in NAV.iter().enumerate() {
                if *g != group {
                    group = g;
                    ui.add_space(if i > 0 { 8.0 } else { 2.0 });
                    ui.label(RichText::new(g.to_uppercase()).size(10.0).color(p.muted).strong());
                }
                if w::nav_item(ui, &p, icon, label, hint, i + 1, sh.view == *v).clicked() {
                    sh.go(*v);
                }
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                let r = ui
                    .add(
                        egui::Label::new(
                            RichText::new(format!("{} v{}\n{}", about::NAME, about::VERSION, about::AUTHOR))
                                .size(10.5)
                                .color(p.muted),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_text("О программе: версия, разработчик, исходный код");
                if r.clicked() {
                    sh.go(View::About);
                }
                ui.add_space(4.0);
                w::card_frame(&p).inner_margin(10).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let side = sh.st.derived.metrics.iter().filter(|m| !m.top).take(4);
                    for m in side {
                        w::kv(ui, &p, m.label, &eng(m.value, m.unit, 3));
                    }
                    if let Some(sim) = &sh.st.sim {
                        for s in &sim.signals {
                            w::kv(ui, &p, s.name, &eng(s.value, s.unit, 3));
                        }
                    }
                });
            });
        });
}

fn console(ctx: &egui::Context, sh: &Shared, cs: &mut ConsoleState) {
    let p = sh.pal;
    let bits = regs::DATA_BITS;
    let reg_name = |r: Option<usize>| r.map(|r| REGS.get(r).map(|d| d.name).unwrap_or("?")).unwrap_or("");
    egui::TopBottomPanel::bottom("console")
        .resizable(true)
        .default_height(260.0)
        .height_range(140.0..=600.0)
        .frame(egui::Frame::new().fill(p.panel).inner_margin(egui::Margin::symmetric(10, 6)).stroke(egui::Stroke::new(1.0, p.line)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Журнал шины").strong().size(12.5));
                ui.label(RichText::new(format!("{} записей", sh.log.len())).size(11.0).color(p.muted));
                if let Some(f) = w::segmented(
                    ui,
                    &p,
                    cs.filter,
                    &[
                        (LogFilter::All, "Все"),
                        (LogFilter::Write, "Запись"),
                        (LogFilter::Read, "Чтение"),
                        (LogFilter::Note, "События"),
                    ],
                ) {
                    cs.filter = f;
                }
                ui.add(egui::TextEdit::singleline(&mut cs.q).hint_text("🔍 поле, регистр, hex…").desired_width(150.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if w::icon_button(ui, &p, "✕", false).on_hover_text("Закрыть").clicked() {
                        sh.act(Act::Console(false));
                    }
                    if w::icon_button(ui, &p, "🗑", false).on_hover_text("Очистить журнал").clicked() {
                        sh.act(Act::ClearLog);
                    }
                    let send = w::icon_button(ui, &p, "➤", false).on_hover_text("Отправить кадр").clicked();
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut cs.raw)
                            .hint_text("00 80 · r 0C FF")
                            .font(egui::TextStyle::Monospace)
                            .desired_width(130.0),
                    ).on_hover_text("Произвольный кадр SPI: hex-байты (запись, инкремент 04, декремент 08); «r 0C FF» — чтение 00h по 3-проводной схеме");
                    ui.label(RichText::new("raw").size(11.0).color(p.muted));
                    if send || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                        send_raw(sh, &cs.raw);
                    }
                });
            });
            ui.separator();

            let q = cs.q.trim().to_lowercase();
            let rows: Vec<&Txn> = sh
                .log
                .iter()
                .filter(|t| match cs.filter {
                    LogFilter::Write => matches!(t.op, "write" | "raw" | "pin"),
                    LogFilter::Read => t.op == "read",
                    LogFilter::Note => t.op == "note",
                    LogFilter::All => true,
                })
                .filter(|t| {
                    q.is_empty()
                        || t.note.to_lowercase().contains(&q)
                        || t.changes.iter().any(|c| c.0.to_lowercase().contains(&q))
                        || t.tx.as_deref().unwrap_or("").to_lowercase().contains(q.trim_start_matches("0x"))
                        || reg_name(t.reg).to_lowercase() == q
                })
                .collect();
            if rows.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Журнал пуст — измените любой параметр, и кадры шины появятся здесь.").color(p.muted));
                });
                return;
            }
            let t0 = sh.log.first().map(|t| t.t_ms).unwrap_or(0.0);
            let mono = |s: String, c: Color32| RichText::new(s).monospace().size(11.5).color(c);
            TableBuilder::new(ui)
                .striped(false)
                .stick_to_bottom(true)
                .resizable(false)
                .column(Column::exact(44.0))
                .column(Column::exact(70.0))
                .column(Column::exact(40.0))
                .column(Column::exact(80.0))
                .column(Column::exact(64.0))
                .column(Column::exact(200.0))
                .column(Column::exact(120.0))
                .column(Column::remainder())
                .header(18.0, |mut h| {
                    for title in ["#", "T, МС", "ОП.", "РЕГИСТР", "ЗНАЧЕНИЕ", "TX", "RX", "ИЗМЕНЕНИЯ / СОБЫТИЕ"] {
                        h.col(|ui| {
                            ui.label(RichText::new(title).size(10.0).color(p.muted));
                        });
                    }
                })
                .body(|body| {
                    body.rows(18.0, rows.len(), |mut row| {
                        let t = rows[row.index()];
                        let base = if t.ok { p.fg2 } else { p.err };
                        row.col(|ui| {
                            ui.label(mono(t.id.to_string(), p.muted));
                        });
                        row.col(|ui| {
                            ui.label(mono(format!("{:.1}", t.t_ms - t0), p.muted));
                        });
                        row.col(|ui| {
                            let (s, c) = match t.op {
                                "write" => ("↑ W", p.sig),
                                "read" => ("↓ R", p.clk),
                                "raw" => ("RAW", p.refc),
                                "pin" => ("PIN", p.ctl),
                                _ => ("•", p.muted),
                            };
                            ui.label(mono(s.into(), c));
                        });
                        row.col(|ui| {
                            ui.label(mono(reg_name(t.reg).into(), base));
                        });
                        row.col(|ui| {
                            ui.label(mono(t.value.map(|v| hex_w(v, bits)).unwrap_or_default(), base));
                        });
                        row.col(|ui| {
                            ui.add(egui::Label::new(mono(t.tx.clone().unwrap_or_default(), base)).truncate());
                        });
                        row.col(|ui| {
                            ui.label(mono(t.rx.clone().unwrap_or_default(), base));
                        });
                        row.col(|ui| {
                            let mut job = egui::text::LayoutJob::default();
                            let mf = egui::FontId::monospace(11.5);
                            let pf = egui::FontId::proportional(12.0);
                            let fmtc = |c: Color32, f: &egui::FontId| egui::TextFormat::simple(f.clone(), c);
                            for (n, a, b) in &t.changes {
                                job.append(n, 0.0, fmtc(p.accent, &mf));
                                job.append(&format!(" {}→", a), 0.0, fmtc(p.muted, &mf));
                                job.append(&format!("{}  ", b), 0.0, fmtc(p.fg, &mf));
                            }
                            job.append(&t.note, 0.0, fmtc(if t.ok { p.fg2 } else { p.err }, &pf));
                            ui.add(egui::Label::new(job).truncate());
                        });
                    });
                });
        });
}

fn send_raw(sh: &Shared, raw: &str) {
    // "00 80" → full-duplex frame; "r 0C FF" → 3-wire read (bridge drives 6 bits, then listens)
    let s = raw.trim();
    let (hex, drive) = match s.strip_prefix("r ").or_else(|| s.strip_prefix("R ")) {
        Some(h) => (h.trim(), Some(6)),
        None => (s, None),
    };
    let body = hex.strip_prefix("0x").or_else(|| hex.strip_prefix("0X")).unwrap_or(hex);
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_hexdigit() || c.is_whitespace()) {
        sh.toast(
            Tone::Err,
            "Кадр SPI: hex-байты, например 00 80 (запись) или r 0C FF (чтение 00h по 3-проводной схеме)",
        );
        return;
    }
    sh.act(Act::Raw(hex.to_string(), drive));
}
