//! Virtual MCP4161 on a DIGI POT click: answers the same SPI frames as the real
//! chip, so the whole app (and `cargo test`) works without hardware.
//!
//! Modelled (DS22059B): 16 × 9-bit memory (Table 4-1); power-on recall of the NV
//! wiper and TCON = 1FFh (§4.1.1); EEPROM write cycle with EEWA = 1 for tWC, during
//! which only volatile addresses accept commands (Register 4-1); increment /
//! decrement limits (Table 5-1); valid address/command combinations and CMDERR
//! (Table 7-2, §7.3), including "another command after an NV write in the same
//! CS window"; continuous commands (§7.4). Board specifics: MISO is not
//! connected, so a full-duplex read sees only the pulled-up line, and a read
//! with the host still driving SDI is a drive conflict (§6.1.3 note).
//! WiperLock / WP need high-voltage commands (CS at VIHH), which the click
//! cannot produce, so they stay off.

use crate::bus::{self, Cmd};
use crate::model::{self, Settings};
use crate::regs::{Words, FULL_SCALE, REGS};
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug)]
pub struct SimSignal {
    pub name: &'static str,
    pub value: f64,
    pub unit: &'static str,
}

#[derive(Serialize, Clone, Debug)]
pub struct SimStatus {
    /// "ok" | "busy" | "warn" | "fault" | "off"
    pub state: &'static str,
    pub text: String,
    pub signals: Vec<SimSignal>,
    /// Input pin levels as the chip drives them.
    pub pins: Vec<(String, bool)>,
}

/// What a virtual multimeter would show on the terminals of this particular part.
#[derive(Serialize, Clone, Debug)]
pub struct Meter {
    pub rab: f64,
    pub rwb: f64,
    pub rwa: f64,
    pub vw: f64,
}

const ADDR_WIPER0: u8 = 0x00;
const ADDR_WIPER1: u8 = 0x01;
const ADDR_NV_WIPER0: u8 = 0x02;
const ADDR_TCON: u8 = 0x04;
const ADDR_STATUS: u8 = 0x05;
/// tWC used by the simulator (datasheet: 5 ms typ, 10 ms max).
const SIM_TWC: Duration = Duration::from_millis(6);

fn volatile(addr: u8) -> bool {
    matches!(addr, 0x00 | 0x01 | 0x04 | 0x05)
}

pub struct VirtualChip {
    /// Memory by address (9 bits used).
    mem: [u16; 16],
    env: Settings,
    /// Pending EEPROM write: (address, value, completes at).
    ee_pending: Option<(u8, u16, Instant)>,
    ee_writes: u64,
    cmd_errors: u64,
    conflicts: u64,
    last_error: Option<String>,
}

impl VirtualChip {
    pub fn new(env: Settings) -> Self {
        let mut mem = [0x1FFu16; 16];
        for r in REGS {
            mem[r.addr as usize] = r.reset as u16;
        }
        let mut c =
            VirtualChip { mem, env, ee_pending: None, ee_writes: 0, cmd_errors: 0, conflicts: 0, last_error: None };
        c.power_on();
        c
    }

    /// POR/BOR: volatile wiper ← NV wiper, TCON ← 1FFh (§4.1.1).
    pub fn power_on(&mut self) {
        self.finish_eeprom(true);
        self.mem[ADDR_WIPER0 as usize] = self.mem[ADDR_NV_WIPER0 as usize];
        self.mem[ADDR_TCON as usize] = 0x1FF;
        self.mem[ADDR_STATUS as usize] = 0x1E0;
    }

    pub fn set_env(&mut self, env: Settings) {
        self.env = env;
    }

    /// Register words as the chip holds them now.
    pub fn words(&mut self) -> Words {
        self.finish_eeprom(false);
        std::array::from_fn(|r| self.mem[REGS[r].addr as usize] as u32)
    }

    pub fn ee_writes(&self) -> u64 {
        self.ee_writes
    }

    fn ee_busy(&mut self) -> bool {
        self.finish_eeprom(false);
        self.ee_pending.is_some()
    }

    /// Complete the EEPROM write when tWC has elapsed (or now, if `force`).
    fn finish_eeprom(&mut self, force: bool) {
        if let Some((addr, v, at)) = self.ee_pending {
            if force || Instant::now() >= at {
                self.mem[addr as usize] = v;
                self.ee_pending = None;
            }
        }
    }

    fn status_word(&mut self) -> u16 {
        // D8:D5 = 1111, EEWA, WL1 = WL0 = 0, SHDN = 0 (no pin on MCP41X1), WP = 0
        0x1E0 | if self.ee_busy() { 1 << 4 } else { 0 }
    }

    fn read(&mut self, addr: u8) -> u16 {
        if addr == ADDR_STATUS {
            return self.status_word();
        }
        self.finish_eeprom(false);
        self.mem[addr as usize] & 0x1FF
    }

    /// Execute one command. `Err` = CMDERR (the chip ignores the rest of the frame).
    fn exec(&mut self, c: Cmd, after_nv_write: bool) -> Result<Option<u16>, String> {
        let addr = match c {
            Cmd::Write { addr, .. } | Cmd::Read { addr } | Cmd::Inc { addr } | Cmd::Dec { addr } => addr,
        };
        if after_nv_write {
            return Err("команда после записи в EEPROM в том же окне CS (§7.3)".into());
        }
        if self.ee_busy() && !volatile(addr) {
            return Err(format!("{:02X}h: идёт запись EEPROM (EEWA = 1), доступны только 00h, 01h, 04h, 05h", addr));
        }
        match c {
            Cmd::Read { .. } => Ok(Some(self.read(addr))),
            Cmd::Write { addr: ADDR_STATUS, .. } => Err("05h STATUS только для чтения".into()),
            Cmd::Write { addr, data } if volatile(addr) => {
                if addr != ADDR_WIPER1 {
                    self.mem[addr as usize] = if addr == ADDR_TCON { data | 0x100 } else { data };
                }
                Ok(None)
            }
            Cmd::Write { addr, data } => {
                // EEPROM: 02h, 03h, 06h…0Fh — value lands after tWC
                if addr != 0x03 {
                    self.ee_pending = Some((addr, data & 0x1FF, Instant::now() + SIM_TWC));
                    self.ee_writes += 1;
                }
                Ok(None)
            }
            Cmd::Inc { addr } | Cmd::Dec { addr } if addr == ADDR_WIPER0 || addr == ADDR_WIPER1 => {
                if addr == ADDR_WIPER0 {
                    let w = &mut self.mem[addr as usize];
                    // Table 5-1: no increment at ≥ 100h, no decrement at 0 or above 100h
                    match c {
                        Cmd::Inc { .. } if (*w as u32) < FULL_SCALE => *w += 1,
                        Cmd::Dec { .. } if *w > 0 && (*w as u32) <= FULL_SCALE => *w -= 1,
                        _ => {}
                    }
                }
                Ok(None)
            }
            Cmd::Inc { addr } | Cmd::Dec { addr } => {
                Err(format!("{:02X}h: инкремент/декремент разрешены только для оперативного движка", addr))
            }
        }
    }

    /// Run all commands of one CS window; returns read results in order.
    fn frame(&mut self, tx: &[u8]) -> Vec<Option<u16>> {
        let (cmds, complete) = bus::parse(tx);
        let mut out = vec![];
        let mut nv_written = false;
        for c in cmds {
            match self.exec(c, nv_written) {
                Ok(v) => out.push(v),
                Err(e) => {
                    self.cmd_errors += 1;
                    self.last_error = Some(e);
                    out.push(None);
                    break;
                }
            }
            nv_written = matches!(c, Cmd::Write { addr, .. } if !volatile(addr));
        }
        if !complete {
            self.last_error = Some("кадр оборвался посреди 16-битной команды — она не выполнена".into());
        }
        out
    }

    // ───────────── bus side ─────────────

    /// Full-duplex SPI. On the click MISO is not connected: the host sees its
    /// own pulled-up line (all ones). A read command here means the host keeps
    /// driving SDI while the chip drives SDO on the same pin — a drive conflict.
    pub fn spi(&mut self, tx: &[u8]) -> Vec<u8> {
        let (cmds, _) = bus::parse(tx);
        if cmds.iter().any(|c| matches!(c, Cmd::Read { .. })) {
            self.conflicts += 1;
            self.last_error =
                Some("чтение полнодуплексным кадром: мост и кристалл одновременно управляют линией SDI/SDO".into());
        }
        self.frame(tx);
        vec![0xFF; tx.len()]
    }

    /// 3-wire SPI read (host drives the first `drive_bits`, then listens).
    pub fn spi3(&mut self, tx: &[u8], drive_bits: u32) -> Vec<u8> {
        let (cmds, _) = bus::parse(tx);
        match cmds.first() {
            Some(Cmd::Read { .. }) if drive_bits == bus::CMD_BITS && tx.len() == 2 => {
                let v = self.frame(tx).into_iter().next().flatten();
                bus::read_reply(v.map(|x| x as u32))
            }
            _ => {
                self.frame(tx);
                vec![0xFF; tx.len()]
            }
        }
    }

    pub fn i2c_write(&mut self, _data: &[u8]) -> bool {
        false
    }

    pub fn i2c_read(&mut self, _wr: &[u8]) -> Vec<u8> {
        vec![]
    }

    pub fn pin(&mut self, _name: &str) -> Option<bool> {
        None
    }

    pub fn set_pin(&mut self, _name: &str, _high: bool) {}

    fn actual_rab(&self) -> f64 {
        model::rab_at(self.env.rab_nominal * (1.0 + self.env.sim_rab_dev_pct / 100.0), self.env.temp_c)
    }

    /// Virtual multimeter on the terminals of this part (RAB differs from nominal).
    pub fn meter(&mut self) -> Meter {
        let w = self.words();
        let code = (self.mem[ADDR_WIPER0 as usize] as u32).min(FULL_SCALE);
        let p = model::point(code, self.actual_rab(), &self.env, model::terminals(&w));
        Meter { rab: self.actual_rab(), rwb: p.rwb, rwa: p.rwa, vw: p.vw }
    }

    pub fn take_error(&mut self) -> Option<String> {
        self.last_error.take()
    }

    pub fn poll(&mut self) -> SimStatus {
        let w = self.words();
        let busy = self.ee_busy();
        let t = model::terminals(&w);
        let code = self.mem[ADDR_WIPER0 as usize] as u32;
        let meter = self.meter();
        let (state, text) = if busy {
            ("busy", "Запись EEPROM (EEWA = 1)".to_string())
        } else if t.shutdown {
            ("off", "Отключение: A разомкнут, W → B".to_string())
        } else if self.conflicts > 0 {
            ("warn", format!("Конфликт на SDI/SDO: {}", self.conflicts))
        } else if !(t.a && t.w && t.b) {
            ("warn", "Часть выводов отключена (TCON)".to_string())
        } else {
            ("ok", format!("Движок {} из 256 ({:.0} %)", code.min(FULL_SCALE), code.min(FULL_SCALE) as f64 / 2.56))
        };
        SimStatus {
            state,
            text,
            signals: vec![
                SimSignal { name: "R_WB (мультиметр)", value: meter.rwb, unit: "Ω" },
                SimSignal { name: "V_W", value: meter.vw, unit: "V" },
                SimSignal { name: "Записей EEPROM", value: self.ee_writes as f64, unit: "" },
                SimSignal { name: "Ошибок CMDERR", value: self.cmd_errors as f64, unit: "" },
            ],
            pins: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regs::{self, NUM_REGS};

    fn write(chip: &mut VirtualChip, r: usize, v: u32) {
        let bus::Frame::Spi(tx) = bus::write_frame(r, v) else { unreachable!() };
        chip.spi(&tx);
    }

    fn read(chip: &mut VirtualChip, r: usize) -> Option<u32> {
        let Some(bus::Frame::Spi3 { tx, drive_bits }) = bus::read_frame(r) else { unreachable!() };
        bus::parse_read(&chip.spi3(&tx, drive_bits))
    }

    #[test]
    fn power_on_state() {
        let mut c = VirtualChip::new(Settings::default());
        assert_eq!(read(&mut c, regs::R_WIPER0), Some(0x80));
        assert_eq!(read(&mut c, regs::R_TCON), Some(0x1FF));
        assert_eq!(read(&mut c, regs::R_STATUS), Some(0x1E0));
        assert_eq!(read(&mut c, regs::R_EE0), Some(0x0FF));
    }

    #[test]
    fn volatile_write_and_read() {
        let mut c = VirtualChip::new(Settings::default());
        write(&mut c, regs::R_WIPER0, 0x100);
        assert_eq!(read(&mut c, regs::R_WIPER0), Some(0x100));
        write(&mut c, regs::R_TCON, 0x0F7);
        assert_eq!(read(&mut c, regs::R_TCON), Some(0x1F7), "D8 forced to 1");
    }

    #[test]
    fn status_is_read_only() {
        let mut c = VirtualChip::new(Settings::default());
        write(&mut c, regs::R_STATUS, 0);
        assert_eq!(read(&mut c, regs::R_STATUS), Some(0x1E0));
        assert!(c.take_error().is_some());
    }

    #[test]
    fn increment_decrement_limits() {
        let mut c = VirtualChip::new(Settings::default());
        write(&mut c, regs::R_WIPER0, 0x0FE);
        let bus::Frame::Spi(up) = bus::step_frame(regs::R_WIPER0, true, 5) else { unreachable!() };
        c.spi(&up);
        assert_eq!(read(&mut c, regs::R_WIPER0), Some(0x100), "stops at full scale");
        write(&mut c, regs::R_WIPER0, 1);
        let bus::Frame::Spi(down) = bus::step_frame(regs::R_WIPER0, false, 3) else { unreachable!() };
        c.spi(&down);
        assert_eq!(read(&mut c, regs::R_WIPER0), Some(0));
        write(&mut c, regs::R_WIPER0, 0x150);
        c.spi(&down);
        assert_eq!(read(&mut c, regs::R_WIPER0), Some(0x150), "ignored above 100h");
    }

    #[test]
    fn eeprom_cycle_and_recall() {
        let mut c = VirtualChip::new(Settings::default());
        write(&mut c, regs::R_NV_WIPER0, 0x040);
        // during tWC: EEWA = 1, NV addresses rejected, volatile still OK
        assert_eq!(read(&mut c, regs::R_STATUS).map(|s| s >> 4 & 1), Some(1));
        assert_eq!(read(&mut c, regs::R_EE0), None, "CMDERR while EEWA");
        write(&mut c, regs::R_WIPER0, 0x010);
        std::thread::sleep(Duration::from_millis(12));
        assert_eq!(read(&mut c, regs::R_NV_WIPER0), Some(0x040));
        assert_eq!(read(&mut c, regs::R_WIPER0), Some(0x010));
        c.power_on();
        assert_eq!(read(&mut c, regs::R_WIPER0), Some(0x040), "POR loads the NV wiper");
        assert_eq!(c.ee_writes(), 1);
    }

    #[test]
    fn mikroe_library_frame_writes_eeprom_too() {
        let mut c = VirtualChip::new(Settings::default());
        c.spi(&[0x00, 0x7F, 0x20, 0x7F]);
        assert_eq!(c.ee_writes(), 1);
        c.finish_eeprom(true);
        assert_eq!(c.words()[regs::R_NV_WIPER0], 0x7F);
        // a command after an NV write in the same CS window is an error
        c.spi(&[0x20, 0x10, 0x00, 0x20]);
        assert!(c.take_error().unwrap().contains("§7.3"));
    }

    #[test]
    fn full_duplex_read_is_a_conflict() {
        let mut c = VirtualChip::new(Settings::default());
        let rx = c.spi(&bus::Cmd::Read { addr: 0 }.encode());
        assert_eq!(rx, vec![0xFF, 0xFF], "MISO is not connected on the click");
        assert_eq!(c.poll().state, "warn");
    }

    #[test]
    fn meter_uses_actual_rab() {
        let mut c = VirtualChip::new(Settings { sim_rab_dev_pct: 10.0, ..Settings::default() });
        let m = c.meter();
        assert!((m.rab - 11_000.0).abs() < 1e-6);
        assert!((m.rwb - (5_500.0 + 75.0)).abs() < 1e-6);
        assert_eq!(NUM_REGS, 14);
    }
}
