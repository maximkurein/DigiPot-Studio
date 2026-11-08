//! Memory map of the Microchip MCP4161 (MCP41X1 family, single potentiometer,
//! 8-bit, 257 taps) — datasheet DS22059B, Table 4-1, Register 4-1 (STATUS),
//! Register 4-2 (TCON), Table 4-2 (factory defaults).
//!
//! The device memory is 16 locations × 9 bits. Addresses 01h (Volatile Wiper 1)
//! and 03h (NV Wiper 1) belong to Pot 1, which a single-pot MCP4161 does not
//! have, so they are not listed. Locations 02h and 06h…0Fh are EEPROM: writing
//! them costs an endurance cycle (1M typ) and 5…10 ms (tWC), so they are never
//! written by auto-apply — only by an explicit, confirmed `nv_write`.
//!
//! Conventions
//! - `REGS[i]` is one register; `FieldDef.reg` is an index into `REGS`.
//! - `defaults()` = power-on values of a factory-fresh part: the volatile wiper is
//!   loaded from the NV wiper (80h), TCON = 1FFh, EEPROM data = 0FFh (§4.2.1.3).
//! - `Access::Fixed` fields are always written with their default; `sanitize()`
//!   enforces it. `Access::Ro` bits are excluded from readback comparison.

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    /// read/write configuration
    Rw,
    /// read only (status, measured data, ID)
    Ro,
    /// status flag that stays set until cleared (write-1-to-clear here)
    Sticky,
    /// reserved bits that must be written with a fixed pattern
    Fixed,
}

#[derive(Serialize, Clone, Copy, Debug)]
pub struct RegDef {
    pub addr: u16,
    pub name: &'static str,
    /// Short Russian description shown next to the register.
    pub title: &'static str,
    /// Datasheet power-on value.
    pub reset: u32,
}

#[derive(Serialize, Clone, Copy, Debug)]
pub struct FieldDef {
    pub name: &'static str,
    /// Index into `REGS`.
    pub reg: usize,
    pub msb: u8,
    pub lsb: u8,
    pub access: Access,
    pub default: u32,
    pub title: &'static str,
    pub desc: &'static str,
    pub options: &'static [(u32, &'static str)],
    /// Block id on the overview diagram (`src/lib/blocks.ts`).
    pub block: &'static str,
    pub unit: &'static str,
    /// Value is two's complement (shown signed in the UI).
    pub signed: bool,
}

impl FieldDef {
    pub const fn new(name: &'static str, reg: usize, msb: u8, lsb: u8) -> Self {
        FieldDef { name, reg, msb, lsb, access: Access::Rw, default: 0, title: "", desc: "", options: &[], block: "", unit: "", signed: false }
    }
    pub const fn access(self, access: Access) -> Self {
        FieldDef { access, ..self }
    }
    pub const fn def(self, default: u32) -> Self {
        FieldDef { default, ..self }
    }
    pub const fn title(self, title: &'static str) -> Self {
        FieldDef { title, ..self }
    }
    pub const fn desc(self, desc: &'static str) -> Self {
        FieldDef { desc, ..self }
    }
    pub const fn opts(self, options: &'static [(u32, &'static str)]) -> Self {
        FieldDef { options, ..self }
    }
    pub const fn block(self, block: &'static str) -> Self {
        FieldDef { block, ..self }
    }
    pub const fn unit(self, unit: &'static str) -> Self {
        FieldDef { unit, ..self }
    }
    pub const fn signed(self) -> Self {
        FieldDef { signed: true, ..self }
    }

    pub fn width(&self) -> u32 {
        (self.msb - self.lsb + 1) as u32
    }
    pub fn mask(&self) -> u32 {
        let ones = if self.width() >= 32 { u32::MAX } else { (1u32 << self.width()) - 1 };
        ones << self.lsb
    }
    pub fn max(&self) -> u32 {
        self.mask() >> self.lsb
    }
    pub fn get(&self, w: &Words) -> u32 {
        (w[self.reg] & self.mask()) >> self.lsb
    }
    pub fn put(&self, w: &mut Words, v: u32) {
        w[self.reg] = (w[self.reg] & !self.mask()) | ((v << self.lsb) & self.mask());
    }
}

// ───────────────────────────── the map ─────────────────────────────

/// Register data width in bits: memory is 9 bits wide (D8:D0); the SPI data
/// field has a 10th bit (D9) that the device ignores (§7.2).
pub const DATA_BITS: u32 = 9;

pub const R_WIPER0: usize = 0;
pub const R_NV_WIPER0: usize = 1;
pub const R_TCON: usize = 2;
pub const R_STATUS: usize = 3;
/// First general-purpose EEPROM location (06h); ten of them, 06h…0Fh.
pub const R_EE0: usize = 4;
pub const EE_COUNT: usize = 10;

/// Full-scale wiper code (W = A). 257 taps: 000h…100h (§5.1, Table 5-1).
pub const FULL_SCALE: u32 = 0x100;

pub const REGS: &[RegDef] = &[
    RegDef { addr: 0x00, name: "WIPER0", title: "Положение движка (оперативное, RAM)", reset: 0x080 },
    RegDef { addr: 0x02, name: "NV_WIPER0", title: "Положение движка после включения (EEPROM)", reset: 0x080 },
    RegDef { addr: 0x04, name: "TCON", title: "Подключение выводов A, W, B (RAM)", reset: 0x1FF },
    RegDef { addr: 0x05, name: "STATUS", title: "Состояние: запись EEPROM, WiperLock, защита (только чтение)", reset: 0x1E0 },
    RegDef { addr: 0x06, name: "EE06", title: "Пользовательская ячейка EEPROM 06h", reset: 0x0FF },
    RegDef { addr: 0x07, name: "EE07", title: "Пользовательская ячейка EEPROM 07h", reset: 0x0FF },
    RegDef { addr: 0x08, name: "EE08", title: "Пользовательская ячейка EEPROM 08h", reset: 0x0FF },
    RegDef { addr: 0x09, name: "EE09", title: "Пользовательская ячейка EEPROM 09h", reset: 0x0FF },
    RegDef { addr: 0x0A, name: "EE0A", title: "Пользовательская ячейка EEPROM 0Ah", reset: 0x0FF },
    RegDef { addr: 0x0B, name: "EE0B", title: "Пользовательская ячейка EEPROM 0Bh", reset: 0x0FF },
    RegDef { addr: 0x0C, name: "EE0C", title: "Пользовательская ячейка EEPROM 0Ch", reset: 0x0FF },
    RegDef { addr: 0x0D, name: "EE0D", title: "Пользовательская ячейка EEPROM 0Dh", reset: 0x0FF },
    RegDef { addr: 0x0E, name: "EE0E", title: "Пользовательская ячейка EEPROM 0Eh", reset: 0x0FF },
    RegDef { addr: 0x0F, name: "EE0F", title: "Пользовательская ячейка EEPROM 0Fh", reset: 0x0FF },
];

pub const NUM_REGS: usize = REGS.len();
pub type Words = [u32; NUM_REGS];

use Access::*;

const WIPER_DESC: &str = "Код N = 0…256 (000h…100h). 0 — движок W соединён с выводом B, 256 — с выводом A, 128 (80h) — середина. \
Коды 101h…1FFh тоже дают полную шкалу, но команды инкремента/декремента в этой зоне игнорируются (табл. 5-1).";
const EE_DESC: &str = "9-битная ячейка общего назначения в EEPROM (§4.2.1.1). С завода — 0FFh. Запись занимает 5…10 мс (tWC) и расходует ресурс (1 млн циклов).";

pub const FIELDS: &[FieldDef] = &[
    // 00h Volatile Wiper 0
    FieldDef::new("WIPER0", R_WIPER0, 8, 0).def(0x080).title("Движок (RAM)").desc(WIPER_DESC).block("wiper").unit("код"),
    // 02h Non-Volatile Wiper 0
    FieldDef::new("NV_WIPER0", R_NV_WIPER0, 8, 0)
        .def(0x080)
        .title("Движок при включении (EEPROM)")
        .desc("При включении питания (POR/BOR) это значение копируется в WIPER0 (§4.1.1). С завода — 80h, середина. Записывается только отдельной командой «Записать в EEPROM».")
        .block("eeprom")
        .unit("код"),
    // 04h TCON (Register 4-2)
    FieldDef::new("TCON_D8", R_TCON, 8, 8).access(Ro).def(1).title("Зарезервировано (D8)").desc("Всегда читается как 1 (регистр 4-2).").block("tcon"),
    FieldDef::new("R1_BITS", R_TCON, 7, 4)
        .access(Fixed)
        .def(0b1111)
        .title("Биты Pot 1 (R1HW, R1A, R1W, R1B)")
        .desc("Управляют вторым потенциометром, которого у MCP4161 нет. Записываются единицами, как после включения питания.")
        .block("tcon"),
    FieldDef::new("R0HW", R_TCON, 3, 3)
        .def(1)
        .title("Не в режиме отключения")
        .desc("0 — потенциометр принудительно в конфигурации «shutdown»: вывод A отключён, W соединён с B (рис. 5-2). 1 — обычная работа.")
        .opts(&[(0, "Отключение (A разомкнут, W→B)"), (1, "Обычная работа")])
        .block("tcon"),
    FieldDef::new("R0A", R_TCON, 2, 2).def(1).title("Вывод A подключён").desc("0 — вывод P0A отсоединён от резистивной цепочки.").opts(&[(0, "Отключён"), (1, "Подключён")]).block("tcon"),
    FieldDef::new("R0W", R_TCON, 1, 1).def(1).title("Вывод W подключён").desc("0 — вывод движка P0W отсоединён от цепочки.").opts(&[(0, "Отключён"), (1, "Подключён")]).block("tcon"),
    FieldDef::new("R0B", R_TCON, 0, 0).def(1).title("Вывод B подключён").desc("0 — вывод P0B отсоединён от резистивной цепочки.").opts(&[(0, "Отключён"), (1, "Подключён")]).block("tcon"),
    // 05h STATUS (Register 4-1)
    FieldDef::new("STATUS_D8_5", R_STATUS, 8, 5).access(Ro).def(0b1111).title("Зарезервировано (D8:D5)").desc("Всегда читаются как 1.").block("spi"),
    FieldDef::new("EEWA", R_STATUS, 4, 4)
        .access(Ro)
        .title("Идёт запись EEPROM")
        .desc("1 — идёт цикл записи EEPROM (до 10 мс): разрешены только команды к оперативным адресам 00h, 01h, 04h, 05h.")
        .block("eeprom"),
    FieldDef::new("WL1", R_STATUS, 3, 3).access(Ro).title("WiperLock Pot 1").desc("У одиночного MCP4161 не используется.").block("eeprom"),
    FieldDef::new("WL0", R_STATUS, 2, 2)
        .access(Ro)
        .title("WiperLock Pot 0")
        .desc("1 — движок и биты TCON Pot 0 защищены от записи. Включается только высоковольтной командой (CS ≈ 8.5…12.5 В), на плате click это недоступно.")
        .block("eeprom"),
    FieldDef::new("SHDN", R_STATUS, 1, 1).access(Ro).title("Вывод SHDN активен").desc("Отражает аппаратный вывод SHDN; у 8-выводного MCP4161 его нет — бит читается 0.").block("tcon"),
    FieldDef::new("WP", R_STATUS, 0, 0)
        .access(Ro)
        .title("Защита EEPROM от записи")
        .desc("1 — запись в EEPROM (включая NV_WIPER0) запрещена. Внутренний бит WP меняется только высоковольтной командой.")
        .block("eeprom"),
    // 06h…0Fh general purpose EEPROM
    FieldDef::new("EE06", R_EE0, 8, 0).def(0x0FF).title("Ячейка 06h").desc(EE_DESC).block("eeprom"),
    FieldDef::new("EE07", R_EE0 + 1, 8, 0).def(0x0FF).title("Ячейка 07h").desc(EE_DESC).block("eeprom"),
    FieldDef::new("EE08", R_EE0 + 2, 8, 0).def(0x0FF).title("Ячейка 08h").desc(EE_DESC).block("eeprom"),
    FieldDef::new("EE09", R_EE0 + 3, 8, 0).def(0x0FF).title("Ячейка 09h").desc(EE_DESC).block("eeprom"),
    FieldDef::new("EE0A", R_EE0 + 4, 8, 0).def(0x0FF).title("Ячейка 0Ah").desc(EE_DESC).block("eeprom"),
    FieldDef::new("EE0B", R_EE0 + 5, 8, 0).def(0x0FF).title("Ячейка 0Bh").desc(EE_DESC).block("eeprom"),
    FieldDef::new("EE0C", R_EE0 + 6, 8, 0).def(0x0FF).title("Ячейка 0Ch").desc(EE_DESC).block("eeprom"),
    FieldDef::new("EE0D", R_EE0 + 7, 8, 0).def(0x0FF).title("Ячейка 0Dh").desc(EE_DESC).block("eeprom"),
    FieldDef::new("EE0E", R_EE0 + 8, 8, 0).def(0x0FF).title("Ячейка 0Eh").desc(EE_DESC).block("eeprom"),
    FieldDef::new("EE0F", R_EE0 + 9, 8, 0).def(0x0FF).title("Ячейка 0Fh").desc(EE_DESC).block("eeprom"),
];

/// No double buffering: every write takes effect when CS returns high.
pub const COMMIT_REG: Option<usize> = None;

/// EEPROM-backed locations (02h, 06h…0Fh).
pub fn nv(r: usize) -> bool {
    r == R_NV_WIPER0 || (R_EE0..R_EE0 + EE_COUNT).contains(&r)
}

/// Volatile configuration written by auto-apply: WIPER0 then TCON.
pub fn write_order() -> Vec<usize> {
    (0..NUM_REGS).filter(|&r| writable(r)).collect()
}

// ───────────────────────────── helpers ─────────────────────────────

pub fn field(name: &str) -> Option<&'static FieldDef> {
    FIELDS.iter().find(|f| f.name == name)
}

pub fn fields_of(r: usize) -> impl Iterator<Item = &'static FieldDef> {
    FIELDS.iter().filter(move |f| f.reg == r)
}

pub fn data_mask() -> u32 {
    if DATA_BITS >= 32 {
        u32::MAX
    } else {
        (1u32 << DATA_BITS) - 1
    }
}

/// Volatile register holding configuration that auto-apply writes.
pub fn writable(r: usize) -> bool {
    !nv(r) && fields_of(r).any(|f| f.access == Rw)
}

/// EEPROM location the user may write explicitly.
pub fn nv_writable(r: usize) -> bool {
    nv(r) && fields_of(r).any(|f| f.access == Rw)
}

pub fn defined_mask(r: usize) -> u32 {
    fields_of(r).fold(0, |m, f| m | f.mask())
}

/// Bits that change on their own (excluded from readback comparison).
pub fn status_mask(r: usize) -> u32 {
    fields_of(r).filter(|f| matches!(f.access, Ro | Sticky)).fold(0, |m, f| m | f.mask())
}

pub fn defaults() -> Words {
    std::array::from_fn(|r| REGS[r].reset)
}

/// Working initial configuration. The factory state already works on the
/// DIGI POT click (mid-scale wiper, all terminals connected), so the starter is
/// the power-on state; it exists to keep one place for board-specific tweaks.
pub fn starter() -> Words {
    defaults()
}

/// Keep only defined data bits and force fixed fields to their pattern.
pub fn sanitize(r: usize, w: u32) -> u32 {
    let mut words = [0u32; NUM_REGS];
    words[r] = w & data_mask();
    for f in fields_of(r).filter(|f| f.access == Fixed) {
        f.put(&mut words, f.default);
    }
    words[r]
}

pub fn get(w: &Words, name: &str) -> u32 {
    field(name).map(|f| f.get(w)).unwrap_or(0)
}

/// Field value as the UI sees it (sign-extended for two's complement fields).
pub fn get_signed(w: &Words, name: &str) -> i64 {
    match field(name) {
        Some(f) if f.signed => {
            let v = f.get(w) as i64;
            let bits = f.width();
            if v >> (bits - 1) & 1 == 1 {
                v - (1i64 << bits)
            } else {
                v
            }
        }
        Some(f) => f.get(w) as i64,
        None => 0,
    }
}

/// Returns false for unknown fields, read-only fields and out-of-range values.
pub fn set(w: &mut Words, name: &str, v: u32) -> bool {
    match field(name) {
        Some(f) if f.access == Rw && v <= f.max() => {
            f.put(w, v);
            true
        }
        _ => false,
    }
}

pub fn decode(w: &Words) -> BTreeMap<&'static str, u32> {
    FIELDS.iter().map(|f| (f.name, f.get(w))).collect()
}

pub fn reg_by_addr(addr: u16) -> Option<usize> {
    REGS.iter().position(|r| r.addr == addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_fit_and_do_not_overlap() {
        for r in 0..NUM_REGS {
            let mut seen = 0u32;
            for f in fields_of(r) {
                assert!(f.msb >= f.lsb, "{}: msb < lsb", f.name);
                assert!((f.msb as u32) < DATA_BITS, "{}: outside the register", f.name);
                assert_eq!(seen & f.mask(), 0, "{} overlaps another field", f.name);
                seen |= f.mask();
                assert!(f.default <= f.max(), "{}: default does not fit", f.name);
                for (v, _) in f.options {
                    assert!(*v <= f.max(), "{}: option {} does not fit", f.name, v);
                }
            }
        }
    }

    #[test]
    fn unique_names_and_addresses() {
        for (i, a) in FIELDS.iter().enumerate() {
            assert!(FIELDS[i + 1..].iter().all(|b| b.name != a.name), "duplicate field {}", a.name);
        }
        for (i, a) in REGS.iter().enumerate() {
            assert!(REGS[i + 1..].iter().all(|b| b.addr != a.addr), "duplicate address {}", a.addr);
        }
    }

    #[test]
    fn reset_values_match_field_defaults() {
        // Keeps REGS[].reset and FieldDef.default consistent (both come from the datasheet).
        let d = defaults();
        for f in FIELDS {
            assert_eq!(f.get(&d), f.default, "{}: reset value of {} disagrees with field default", f.name, REGS[f.reg].name);
        }
    }

    #[test]
    fn round_trip_every_rw_field() {
        for f in FIELDS.iter().filter(|f| f.access == Rw) {
            let mut w = starter();
            let before = w;
            assert!(set(&mut w, f.name, f.max()));
            assert_eq!(get(&w, f.name), f.max());
            for g in FIELDS.iter().filter(|g| g.name != f.name) {
                assert_eq!(g.get(&w), g.get(&before), "setting {} changed {}", f.name, g.name);
            }
        }
    }

    #[test]
    fn sanitize_forces_fixed_bits() {
        for f in FIELDS.iter().filter(|f| f.access == Fixed) {
            let mut w = [0u32; NUM_REGS];
            w[f.reg] = sanitize(f.reg, 0);
            assert_eq!(f.get(&w), f.default, "{}", f.name);
        }
    }

    #[test]
    fn datasheet_values() {
        // Table 4-1 addresses, Table 4-2 factory wiper 80h, TCON POR 1FFh, STATUS D8:D5 = 1.
        let d = defaults();
        assert_eq!(REGS.iter().map(|r| r.addr).collect::<Vec<_>>(), vec![0x00, 0x02, 0x04, 0x05, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        assert_eq!(get(&d, "WIPER0"), 0x80);
        assert_eq!(get(&d, "NV_WIPER0"), 0x80);
        assert_eq!(d[R_TCON], 0x1FF);
        assert_eq!(d[R_STATUS] >> 5, 0b1111);
        assert!(writable(R_WIPER0) && writable(R_TCON) && !writable(R_STATUS));
        assert!(!writable(R_NV_WIPER0) && nv_writable(R_NV_WIPER0) && nv_writable(R_EE0 + 9));
        assert_eq!(write_order(), vec![R_WIPER0, R_TCON]);
    }

    #[test]
    fn tcon_pot1_bits_stay_set() {
        assert_eq!(sanitize(R_TCON, 0x00F) & 0xF0, 0xF0);
    }
}
