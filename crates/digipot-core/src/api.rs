//! Engine: single owner of the configuration, the hardware link and the bus
//! transaction log. The Tauri app and the HTTP server both expose it through
//! one JSON RPC: `handle(cmd, args) -> json`.
//!
//! Adding a feature = a new `cmd` arm here + its types in `src/lib/types.ts`.
//!
//! MCP4161 specifics: auto-apply writes only the volatile registers (WIPER0,
//! TCON). EEPROM locations (NV_WIPER0, 06h…0Fh) are written only by the explicit
//! `nv_write` command with `confirm: true` — each write costs one of ~1 million
//! endurance cycles and takes tWC ≤ 10 ms.

use crate::about;
use crate::bus::{self, Frame};
use crate::model::{self, Derived, Settings};
use crate::regs::{self, Words, FIELDS, FULL_SCALE, NUM_REGS, REGS};
use crate::sim::VirtualChip;
use crate::transport::{self, Link, SerialBridge, PINS};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const LOG_CAP: usize = 4000;

#[derive(Serialize, Clone, Debug)]
pub struct Txn {
    pub id: u64,
    pub t_ms: f64,
    /// "write" | "read" | "raw" | "pin" | "note"
    pub op: &'static str,
    pub reg: Option<usize>,
    pub tx: Option<String>,
    pub rx: Option<String>,
    pub value: Option<u32>,
    /// (field, old, new)
    pub changes: Vec<(String, u32, u32)>,
    pub note: String,
    pub ok: bool,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Persist {
    settings: Settings,
    regs: Vec<u32>,
    auto_apply: bool,
}

#[derive(Serialize, Clone, Debug)]
struct Readback {
    words: Vec<Option<u32>>,
    matches: Vec<Option<bool>>,
}

impl Readback {
    fn empty() -> Self {
        Readback { words: vec![None; NUM_REGS], matches: vec![None; NUM_REGS] }
    }
}

pub struct Engine {
    /// Desired configuration (what the UI edits). NV entries mirror the chip.
    regs: Words,
    /// What we believe the chip holds; `None` = unknown (fresh hardware link).
    dev: Option<Words>,
    settings: Settings,
    link: Box<dyn Link>,
    auto_apply: bool,
    log: VecDeque<Txn>,
    next_id: u64,
    t0: Instant,
    cfg_path: Option<PathBuf>,
    readback: Readback,
    link_error: Option<String>,
    /// EEPROM writes issued in this session (endurance awareness).
    ee_writes: u64,
}

fn arg<T: for<'de> Deserialize<'de>>(v: &Value, key: &str) -> Result<T> {
    serde_json::from_value(v.get(key).cloned().unwrap_or(Value::Null)).map_err(|e| Error::Invalid(format!("параметр '{}': {}", key, e)))
}

fn opt<T: for<'de> Deserialize<'de>>(v: &Value, key: &str) -> Option<T> {
    v.get(key).and_then(|x| serde_json::from_value(x.clone()).ok())
}

fn sanitized(w: &Words) -> Words {
    std::array::from_fn(|r| regs::sanitize(r, w[r]))
}

fn nv_field(name: &str) -> bool {
    regs::field(name).map(|f| regs::nv(f.reg)).unwrap_or(false)
}

const NV_ONLY: &str = "EEPROM записывается только командой «Записать в EEPROM» с подтверждением";

impl Engine {
    pub fn new(cfg_path: Option<PathBuf>) -> Self {
        let mut p = Persist { auto_apply: true, ..Default::default() };
        if let Some(path) = &cfg_path {
            if let Ok(txt) = std::fs::read_to_string(path) {
                if let Ok(x) = serde_json::from_str::<Persist>(&txt) {
                    p = x;
                }
            }
        }
        let por = sanitized(&regs::defaults());
        let mut regs = regs::starter();
        if p.regs.len() == NUM_REGS {
            for (i, w) in p.regs.iter().enumerate() {
                regs[i] = if regs::nv(i) { por[i] } else { regs::sanitize(i, *w) };
            }
        }
        let sim = VirtualChip::new(p.settings.clone());
        let mut e = Engine {
            regs,
            dev: Some(por),
            settings: p.settings,
            link: Box::new(sim),
            auto_apply: p.auto_apply,
            log: VecDeque::new(),
            next_id: 1,
            t0: Instant::now(),
            cfg_path,
            readback: Readback::empty(),
            link_error: None,
            ee_writes: 0,
        };
        e.note("Запуск: подключён виртуальный MCP4161 (симулятор)");
        if e.auto_apply {
            let _ = e.apply();
        }
        e
    }

    fn persist(&self) {
        if let Some(path) = &self.cfg_path {
            let p = Persist { settings: self.settings.clone(), regs: self.regs.to_vec(), auto_apply: self.auto_apply };
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, serde_json::to_string_pretty(&p).unwrap_or_default());
        }
    }

    // ───────────── log ─────────────

    fn push_log(&mut self, mut t: Txn) {
        t.id = self.next_id;
        self.next_id += 1;
        t.t_ms = self.t0.elapsed().as_secs_f64() * 1e3;
        self.log.push_back(t);
        while self.log.len() > LOG_CAP {
            self.log.pop_front();
        }
    }

    fn txn(op: &'static str, note: impl Into<String>) -> Txn {
        Txn { id: 0, t_ms: 0.0, op, reg: None, tx: None, rx: None, value: None, changes: vec![], note: note.into(), ok: true }
    }

    fn note(&mut self, msg: impl Into<String>) {
        self.push_log(Self::txn("note", msg));
    }

    fn fail(&mut self, e: &Error) {
        self.link_error = Some(e.to_string());
    }

    // ───────────── bus ─────────────

    fn transfer(&mut self, f: &Frame) -> Result<Vec<u8>> {
        let r = match f {
            Frame::Spi(tx) => self.link.spi(tx),
            Frame::Spi3 { tx, drive_bits } => self.link.spi3(tx, *drive_bits),
            Frame::I2cWrite { addr, data } => self.link.i2c_write(*addr, data).map(|_| vec![]),
            Frame::I2cRead { addr, wr, n } => self.link.i2c_read(*addr, wr, *n),
        };
        match &r {
            Ok(_) => self.link_error = None,
            Err(e) => self.fail(e),
        }
        // The simulator explains what the real chip would silently reject.
        if let Some(msg) = self.link.as_sim().and_then(|s| s.take_error()) {
            let mut t = Self::txn("note", format!("симулятор: {}", msg));
            t.ok = false;
            self.push_log(t);
        }
        r
    }

    fn frame_tx(f: &Frame) -> String {
        match f {
            Frame::Spi(tx) => bus::hex(tx),
            Frame::Spi3 { tx, drive_bits } => format!("{} (3-пров., ведёт {} бит)", bus::hex(tx), drive_bits),
            Frame::I2cWrite { addr, data } => format!("{:02X}: {}", addr, bus::hex(data)),
            Frame::I2cRead { addr, wr, n } => format!("{:02X}: {} → {} Б", addr, bus::hex(wr), n),
        }
    }

    fn write_reg_value(&mut self, r: usize, value: u32, note: &str) -> Result<()> {
        let value = regs::sanitize(r, value);
        let prev = self.dev.map(|d| d[r]);
        let mut changes = vec![];
        let (mut a, mut b) = ([0u32; NUM_REGS], [0u32; NUM_REGS]);
        a[r] = prev.unwrap_or(REGS[r].reset);
        b[r] = value;
        for f in regs::fields_of(r).filter(|f| f.access == regs::Access::Rw) {
            let (o, n) = (f.get(&a), f.get(&b));
            if o != n || prev.is_none() {
                changes.push((f.name.to_string(), o, n));
            }
        }
        let frame = bus::write_frame(r, value);
        let res = self.transfer(&frame);
        let mut t = Self::txn("write", note);
        t.reg = Some(r);
        t.tx = Some(Self::frame_tx(&frame));
        t.value = Some(value);
        t.changes = changes;
        if let Err(e) = &res {
            t.ok = false;
            t.note = format!("{} — ОШИБКА: {}", note, e);
        }
        self.push_log(t);
        res?;
        let mut d = self.dev.unwrap_or_else(|| sanitized(&regs::defaults()));
        d[r] = value;
        self.dev = Some(d);
        Ok(())
    }

    fn write_reg(&mut self, r: usize, note: &str) -> Result<()> {
        self.write_reg_value(r, self.regs[r], note)
    }

    pub fn write_all(&mut self) -> Result<Vec<usize>> {
        let mut v = vec![];
        for r in regs::write_order() {
            self.write_reg(r, "полная запись конфигурации")?;
            v.push(r);
        }
        Ok(v)
    }

    /// Write only volatile registers whose desired value differs from the chip.
    pub fn apply(&mut self) -> Result<Vec<usize>> {
        let Some(dev) = self.dev else {
            return self.write_all();
        };
        let desired = sanitized(&self.regs);
        let changed: Vec<usize> = regs::write_order().into_iter().filter(|&r| desired[r] != dev[r]).collect();
        for &r in &changed {
            self.write_reg(r, "изменение конфигурации")?;
        }
        Ok(changed)
    }

    fn after_change(&mut self) -> Result<()> {
        self.persist();
        if self.auto_apply {
            self.apply()?;
        }
        Ok(())
    }

    fn dirty(&self) -> Vec<bool> {
        let desired = sanitized(&self.regs);
        (0..NUM_REGS).map(|r| regs::writable(r) && self.dev.map(|d| d[r] != desired[r]).unwrap_or(true)).collect()
    }

    /// Increment / decrement commands: the chip moves the wiper itself.
    fn step(&mut self, up: bool, n: usize) -> Result<u32> {
        let n = n.clamp(1, 256);
        let frame = bus::step_frame(regs::R_WIPER0, up, n);
        let res = self.transfer(&frame);
        let mut t = Self::txn("write", format!("{} движка ×{}", if up { "инкремент" } else { "декремент" }, n));
        t.reg = Some(regs::R_WIPER0);
        t.tx = Some(Self::frame_tx(&frame));
        t.ok = res.is_ok();
        self.push_log(t);
        res?;
        // same limits as the chip (Table 5-1)
        let mut w = self.dev.map(|d| d[regs::R_WIPER0]).unwrap_or(self.regs[regs::R_WIPER0]);
        for _ in 0..n {
            if up && w < FULL_SCALE {
                w += 1;
            } else if !up && w > 0 && w <= FULL_SCALE {
                w -= 1;
            }
        }
        if let Some(d) = self.dev.as_mut() {
            d[regs::R_WIPER0] = w;
        }
        self.regs[regs::R_WIPER0] = w;
        self.persist();
        Ok(w)
    }

    /// One EEPROM write, then wait tWC max so the next command is accepted.
    fn nv_write(&mut self, r: usize, value: u32) -> Result<()> {
        if !regs::nv_writable(r) {
            return Err(Error::Invalid(format!("{} — не ячейка EEPROM", REGS[r].name)));
        }
        self.ee_writes += 1;
        let note = format!("запись EEPROM {} (№{} за сеанс; ресурс ~1 млн циклов, tWC ≤ 10 мс)", REGS[r].name, self.ee_writes);
        self.write_reg_value(r, value, &note)?;
        std::thread::sleep(Duration::from_secs_f64(model::TWC_MAX_S));
        self.regs[r] = regs::sanitize(r, value);
        Ok(())
    }

    // ───────────── readback ─────────────

    fn read_raw(&mut self, r: usize) -> Result<(Frame, Vec<u8>, Option<u32>)> {
        let frame = bus::read_frame(r).ok_or_else(|| Error::Invalid("эта шина не поддерживает чтение регистров".into()))?;
        let rx = self.transfer(&frame)?;
        let v = bus::parse_read(&rx);
        Ok((frame, rx, v))
    }

    pub fn read_register(&mut self, r: usize) -> Result<u32> {
        let res = self.read_raw(r);
        let mut t = Self::txn("read", format!("чтение {}", REGS[r].name));
        t.reg = Some(r);
        match &res {
            Ok((f, rx, v)) => {
                t.tx = Some(Self::frame_tx(f));
                t.rx = Some(bus::hex(rx));
                t.value = *v;
                match v {
                    None => {
                        t.ok = false;
                        t.note = format!("{} — CMDERR = 0: кристалл отклонил команду (идёт запись EEPROM?)", t.note);
                    }
                    Some(0x1FF) if r == regs::R_STATUS => {
                        t.ok = false;
                        t.note = format!("{} — одни единицы: линию SDI/SDO никто не ведёт (питание click, CS, проводка?)", t.note);
                    }
                    _ => {}
                }
            }
            Err(e) => {
                t.ok = false;
                t.note = format!("{} — ОШИБКА: {}", t.note, e);
            }
        }
        self.push_log(t);
        let (_, _, v) = res?;
        let v = v.ok_or_else(|| Error::Link(format!("{}: CMDERR — команда отклонена", REGS[r].name)))?;
        let expect = self.dev.map(|d| d[r]).unwrap_or(regs::sanitize(r, self.regs[r]));
        let mask = regs::defined_mask(r) & !regs::status_mask(r);
        self.readback.words[r] = Some(v);
        self.readback.matches[r] = Some(mask == 0 || (v & mask) == (expect & mask));
        Ok(v)
    }

    /// Read everything and take the chip's state as the current configuration.
    fn adopt(&mut self) -> Result<()> {
        let mut w = self.regs;
        for (r, slot) in w.iter_mut().enumerate() {
            *slot = self.read_register(r)?;
        }
        self.dev = Some(sanitized(&w));
        self.regs = sanitized(&w);
        self.readback = Readback::empty();
        self.persist();
        Ok(())
    }

    // ───────────── derived data ─────────────

    pub fn derived(&self) -> Derived {
        model::derive(&self.regs, &self.settings)
    }

    fn sim_status(&mut self) -> Option<crate::sim::SimStatus> {
        self.link.as_sim().map(|s| s.poll())
    }

    fn snapshot(&mut self) -> Value {
        let d = self.derived();
        let sim = self.sim_status();
        let meter = self.link.as_sim().map(|s| s.meter());
        json!({
            "regs": self.regs,
            "desired": sanitized(&self.regs),
            "dev_regs": self.dev,
            "dirty": self.dirty(),
            "fields": regs::decode(&self.regs),
            "derived": d,
            "settings": self.settings,
            "link": self.link.info(),
            "link_error": self.link_error,
            "auto_apply": self.auto_apply,
            "pins": {},
            "inputs": {},
            "log_head": self.next_id - 1,
            "readback": self.readback,
            "sim": sim,
            "meter": meter,
            "ee_writes": self.ee_writes,
        })
    }

    fn status(&mut self) -> Value {
        let sim = self.sim_status();
        let meter = self.link.as_sim().map(|s| s.meter());
        json!({ "sim": sim, "meter": meter, "inputs": {}, "log_head": self.next_id - 1, "link_error": self.link_error })
    }

    // ───────────── config files ─────────────

    fn export_text(&self) -> String {
        let d = self.derived();
        let mut s = format!("# DigiPot Studio — конфигурация MCP4161\n# {}\n", bus::describe());
        for m in d.metrics.iter().filter(|m| m.top) {
            s.push_str(&format!("# {} = {} {}\n", m.label, m.value, m.unit));
        }
        s.push_str("# адрес\tрегистр\tзначение\n");
        for r in regs::write_order() {
            s.push_str(&format!("0x{:02X}\t{}\t0x{:03X}\n", REGS[r].addr, REGS[r].name, regs::sanitize(r, self.regs[r])));
        }
        s.push_str("# EEPROM (справочно, при импорте не записывается):\n");
        for r in (0..NUM_REGS).filter(|&r| regs::nv(r)) {
            s.push_str(&format!("# 0x{:02X}\t{}\t0x{:03X}\n", REGS[r].addr, REGS[r].name, self.regs[r]));
        }
        s
    }

    fn import_text(&mut self, text: &str) -> Result<usize> {
        let num = |t: &str| -> Option<u32> {
            let t = t.trim();
            match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                Some(h) => u32::from_str_radix(h, 16).ok(),
                None => t.parse().ok(),
            }
        };
        let mut n = 0;
        let mut w = self.regs;
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//")) {
            let toks: Vec<&str> = line.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '=' || c == ':').filter(|t| !t.is_empty()).collect();
            if toks.len() < 2 {
                continue;
            }
            let reg = REGS.iter().position(|r| toks.iter().any(|t| t.eq_ignore_ascii_case(r.name))).or_else(|| num(toks[0]).and_then(|a| regs::reg_by_addr(a as u16)));
            if let (Some(r), Some(v)) = (reg, num(toks[toks.len() - 1])) {
                if regs::writable(r) {
                    w[r] = regs::sanitize(r, v);
                    n += 1;
                }
            }
        }
        if n == 0 {
            return Err(Error::Invalid("не найдено ни одной строки «адрес/имя значение» для WIPER0 или TCON".into()));
        }
        self.regs = w;
        Ok(n)
    }

    // ───────────── RPC ─────────────

    pub fn handle(&mut self, cmd: &str, args: Value) -> Result<Value> {
        let a = &args;
        match cmd {
            "get_state" => Ok(self.snapshot()),
            "get_meta" => Ok(json!({
                "regs": REGS, "fields": FIELDS, "defaults": regs::defaults(), "pins": PINS,
                "nv": (0..NUM_REGS).map(regs::nv).collect::<Vec<_>>(),
                "rab_options": model::RAB_OPTIONS,
                "version": about::VERSION,
                "app": { "name": about::NAME, "version": about::VERSION, "release_date": about::RELEASE_DATE,
                         "author": about::AUTHOR, "license": about::LICENSE,
                         "repository": about::REPOSITORY, "mirror": about::MIRROR },
                "chip": { "name": "MCP4161", "vendor": "Microchip", "board": "DIGI POT click (MIKROE-923)",
                          "bus": bus::describe(), "data_bits": regs::DATA_BITS, "can_read": bus::can_read(),
                          "commit_reg": regs::COMMIT_REG, "full_scale": FULL_SCALE },
            })),
            "status" => Ok(self.status()),

            "set_field" => {
                let name: String = arg(a, "name")?;
                let value: u32 = arg(a, "value")?;
                if nv_field(&name) {
                    return Err(Error::Invalid(NV_ONLY.into()));
                }
                if !regs::set(&mut self.regs, &name, value) {
                    return Err(Error::Invalid(format!("поле {} не принимает значение {}", name, value)));
                }
                self.after_change()?;
                Ok(self.snapshot())
            }
            "set_fields" => {
                let values: BTreeMap<String, u32> = arg(a, "values")?;
                let mut w = self.regs;
                for (k, v) in &values {
                    if nv_field(k) {
                        return Err(Error::Invalid(NV_ONLY.into()));
                    }
                    if !regs::set(&mut w, k, *v) {
                        return Err(Error::Invalid(format!("поле {} не принимает значение {}", k, v)));
                    }
                }
                self.regs = w;
                self.after_change()?;
                Ok(self.snapshot())
            }
            "set_register" => {
                let i: usize = arg(a, "index")?;
                let v: u32 = arg(a, "value")?;
                if i >= NUM_REGS {
                    return Err(Error::Invalid(format!("индекс регистра 0…{}", NUM_REGS - 1)));
                }
                if regs::nv(i) {
                    return Err(Error::Invalid(NV_ONLY.into()));
                }
                self.regs[i] = regs::sanitize(i, v);
                self.after_change()?;
                Ok(self.snapshot())
            }
            "reset_defaults" => {
                let starter = opt::<bool>(a, "starter").unwrap_or(false);
                let base = if starter { regs::starter() } else { regs::defaults() };
                for r in regs::write_order() {
                    self.regs[r] = base[r];
                }
                self.after_change()?;
                Ok(self.snapshot())
            }
            "set_auto_apply" => {
                self.auto_apply = arg(a, "on")?;
                self.persist();
                if self.auto_apply {
                    self.apply()?;
                }
                Ok(self.snapshot())
            }
            "apply" => {
                if self.apply()?.is_empty() {
                    self.note("Нет изменений для записи");
                }
                Ok(self.snapshot())
            }
            "write_all" => {
                self.write_all()?;
                Ok(self.snapshot())
            }
            "write_register" => {
                let i: usize = arg(a, "index")?;
                if i >= NUM_REGS || !regs::writable(i) {
                    return Err(Error::Invalid("регистр не записывается этой командой".into()));
                }
                self.write_reg(i, "запись по запросу")?;
                Ok(self.snapshot())
            }
            "step" => {
                let up: bool = arg(a, "up")?;
                let n: usize = opt(a, "n").unwrap_or(1);
                self.step(up, n)?;
                Ok(self.snapshot())
            }
            "goto" => {
                // {"target": "rwb" | "vw" | "code", "value": f64}
                let target: String = arg(a, "target")?;
                let v: f64 = arg(a, "value")?;
                let code = match target.as_str() {
                    "rwb" => model::code_for_rwb(v, &self.settings),
                    "vw" => model::code_for_vw(v, &self.settings),
                    "code" => (v.round().clamp(0.0, FULL_SCALE as f64)) as u32,
                    t => return Err(Error::Invalid(format!("неизвестная цель {}", t))),
                };
                self.regs[regs::R_WIPER0] = code;
                self.after_change()?;
                Ok(self.snapshot())
            }
            "nv_write" => {
                let i: usize = arg(a, "index")?;
                let v: u32 = arg(a, "value")?;
                if !opt::<bool>(a, "confirm").unwrap_or(false) {
                    return Err(Error::Invalid("запись в EEPROM требует подтверждения (confirm: true)".into()));
                }
                if i >= NUM_REGS {
                    return Err(Error::Invalid("нет такого регистра".into()));
                }
                self.nv_write(i, v)?;
                Ok(self.snapshot())
            }
            "power_cycle" => {
                let Some(sim) = self.link.as_sim() else {
                    return Err(Error::Invalid("только для симулятора: на плате снимите и подайте питание click вручную, затем «Прочитать всё»".into()));
                };
                sim.power_on();
                let w = sim.words();
                self.dev = Some(w);
                self.regs = w;
                self.readback = Readback::empty();
                self.persist();
                self.note(format!("Питание выключено и включено: движок загружен из EEPROM (NV_WIPER0 = {:03X}h), TCON = 1FFh", w[regs::R_NV_WIPER0]));
                Ok(self.snapshot())
            }
            "curve" => {
                let c = model::curve(&self.regs, &self.settings);
                let meter = self.link.as_sim().map(|s| s.meter());
                Ok(json!({ "curve": c, "meter": meter }))
            }
            "read_register" => {
                let i: usize = arg(a, "index")?;
                if i >= NUM_REGS {
                    return Err(Error::Invalid("нет такого регистра".into()));
                }
                let v = self.read_register(i)?;
                Ok(json!({ "value": v, "state": self.snapshot() }))
            }
            "read_all" => {
                for r in 0..NUM_REGS {
                    self.read_register(r)?;
                }
                Ok(self.snapshot())
            }
            "adopt" => {
                self.adopt()?;
                self.note("Конфигурация взята из кристалла");
                Ok(self.snapshot())
            }
            "raw" => {
                // {"hex": "0080"} → full-duplex SPI; {"hex": "0FFF", "drive": 6} → 3-wire read
                let hex: String = opt(a, "hex").unwrap_or_default();
                let bytes = if hex.trim().is_empty() { vec![] } else { bus::unhex(&hex).ok_or_else(|| Error::Invalid("ожидаются hex-байты, например 00 80".into()))? };
                if bytes.is_empty() {
                    return Err(Error::Invalid("пустой кадр".into()));
                }
                let frame = match opt::<u32>(a, "drive") {
                    Some(d) => Frame::Spi3 { tx: bytes, drive_bits: d },
                    None => Frame::Spi(bytes),
                };
                let res = self.transfer(&frame);
                let mut t = Self::txn("raw", "произвольный кадр");
                t.tx = Some(Self::frame_tx(&frame));
                t.rx = res.as_ref().ok().map(|x| bus::hex(x));
                t.ok = res.is_ok();
                self.push_log(t);
                let rx = res?;
                // the chip state is unknown after a raw frame
                self.dev = None;
                Ok(json!({ "rx": bus::hex(&rx), "state": self.snapshot() }))
            }
            "set_settings" => {
                let patch: serde_json::Map<String, Value> = arg(a, "patch")?;
                let mut v = serde_json::to_value(&self.settings)?;
                for (k, x) in patch {
                    if v.get(&k).is_none() {
                        return Err(Error::Invalid(format!("неизвестная настройка {}", k)));
                    }
                    v[k] = x;
                }
                self.settings = serde_json::from_value(v)?;
                let env = self.settings.clone();
                if let Some(sim) = self.link.as_sim() {
                    sim.set_env(env);
                }
                self.persist();
                Ok(self.snapshot())
            }
            "log" => {
                let since: u64 = opt(a, "since").unwrap_or(0);
                let entries: Vec<&Txn> = self.log.iter().filter(|t| t.id > since).collect();
                Ok(json!({ "entries": entries, "head": self.next_id - 1 }))
            }
            "clear_log" => {
                self.log.clear();
                Ok(json!({ "head": self.next_id - 1 }))
            }
            "list_ports" => Ok(serde_json::to_value(transport::list_ports())?),
            "scan_usb" => Ok(serde_json::to_value(transport::scan_usb()?)?),
            "connect" => {
                let kind: String = arg(a, "kind")?;
                match kind.as_str() {
                    "sim" => {
                        self.link = Box::new(VirtualChip::new(self.settings.clone()));
                        let por = sanitized(&regs::defaults());
                        for r in (0..NUM_REGS).filter(|&r| regs::nv(r)) {
                            self.regs[r] = por[r];
                        }
                        self.dev = Some(por);
                        self.note("Подключён симулятор");
                    }
                    "serial" => {
                        let path: String = arg(a, "path")?;
                        let baud: u32 = opt(a, "baud").unwrap_or(115_200);
                        let b = SerialBridge::open(&path, baud)?;
                        let detail = b.info().detail;
                        self.link = Box::new(b);
                        self.dev = None; // unknown until the first full write
                        self.note(format!("Подключён мост {} ({})", path, detail));
                        // learn the EEPROM contents; a bridge without 3-wire reads just skips this
                        for r in (0..NUM_REGS).filter(|&r| regs::nv(r)) {
                            match self.read_register(r) {
                                Ok(v) => self.regs[r] = v,
                                Err(_) => break,
                            }
                        }
                    }
                    k => return Err(Error::Invalid(format!("неизвестный тип подключения {}", k))),
                }
                self.link_error = None;
                self.readback = Readback::empty();
                if self.auto_apply {
                    self.apply()?;
                }
                Ok(self.snapshot())
            }
            "export_config" => Ok(json!({ "text": self.export_text() })),
            "import_config" => {
                let text: String = arg(a, "text")?;
                let n = self.import_text(&text)?;
                self.note(format!("Импорт: {} регистров", n));
                self.after_change()?;
                Ok(self.snapshot())
            }
            _ => Err(Error::UnknownCommand(cmd.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_round_trip() {
        let mut e = Engine::new(None);
        let meta = e.handle("get_meta", json!({})).unwrap();
        assert_eq!(meta["fields"].as_array().unwrap().len(), FIELDS.len());
        let s = e.handle("set_field", json!({"name": "WIPER0", "value": 0x40})).unwrap();
        assert_eq!(s["fields"]["WIPER0"], 0x40);
        assert!(s["dirty"].as_array().unwrap().iter().all(|d| d == false), "auto-apply wrote everything");
        assert!(e.handle("set_field", json!({"name": "EEWA", "value": 1})).is_err(), "read-only field");
        assert!(e.handle("set_field", json!({"name": "NV_WIPER0", "value": 1})).is_err(), "EEPROM only via nv_write");
        assert!(e.handle("nope", json!({})).is_err());
    }

    #[test]
    fn readback_matches_after_write() {
        let mut e = Engine::new(None);
        e.handle("write_all", json!({})).unwrap();
        let s = e.handle("read_all", json!({})).unwrap();
        for (r, def) in REGS.iter().enumerate() {
            assert_eq!(s["readback"]["matches"][r], true, "{}", def.name);
        }
    }

    #[test]
    fn export_import_round_trip() {
        let mut e = Engine::new(None);
        e.handle("set_field", json!({"name": "WIPER0", "value": 0x33})).unwrap();
        let text = e.handle("export_config", json!({})).unwrap()["text"].as_str().unwrap().to_string();
        e.handle("reset_defaults", json!({})).unwrap();
        let s = e.handle("import_config", json!({ "text": text })).unwrap();
        assert_eq!(s["fields"]["WIPER0"], 0x33);
    }

    #[test]
    fn step_tracks_chip_limits() {
        let mut e = Engine::new(None);
        e.handle("goto", json!({"target": "code", "value": 254})).unwrap();
        let s = e.handle("step", json!({"up": true, "n": 5})).unwrap();
        assert_eq!(s["fields"]["WIPER0"], 0x100);
        let v = e.handle("read_register", json!({"index": regs::R_WIPER0})).unwrap();
        assert_eq!(v["value"], 0x100, "simulator agrees");
    }

    #[test]
    fn nv_write_needs_confirm_and_survives_power_cycle() {
        let mut e = Engine::new(None);
        assert!(e.handle("nv_write", json!({"index": regs::R_NV_WIPER0, "value": 0x20})).is_err());
        e.handle("nv_write", json!({"index": regs::R_NV_WIPER0, "value": 0x20, "confirm": true})).unwrap();
        e.handle("goto", json!({"target": "code", "value": 200})).unwrap();
        let s = e.handle("power_cycle", json!({})).unwrap();
        assert_eq!(s["fields"]["WIPER0"], 0x20);
        assert_eq!(s["ee_writes"], 1);
    }

    #[test]
    fn settings_and_curve() {
        let mut e = Engine::new(None);
        e.handle("set_settings", json!({"patch": {"rab_nominal": 50000.0}})).unwrap();
        let c = e.handle("curve", json!({})).unwrap();
        assert_eq!(c["curve"]["code"].as_array().unwrap().len(), 257);
        assert!(e.handle("set_settings", json!({"patch": {"bogus": 1}})).is_err());
    }
}
