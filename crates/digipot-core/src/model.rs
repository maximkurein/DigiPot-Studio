//! Engineering model of the MCP4161 on the DIGI POT click: everything that can be
//! computed from the register words plus board settings. The UI never
//! re-implements these formulas. Sources: DS22059B §5 (resistor network,
//! Equations 5-1 and 5-2), electrical characteristics tables (RAB ±20 %, RW,
//! IW max, bandwidth, tempco, INL/DNL), Table 6-1 (SPI clock).

use crate::regs::{self, Words, FULL_SCALE};
use serde::{Deserialize, Serialize};

/// Board-level parameters that are not in the chip's registers.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// Nominal RAB of the fitted part, Ω: -502 = 5 k, -103 = 10 k, -503 = 50 k, -104 = 100 k.
    pub rab_nominal: f64,
    /// Supply of the click (3.3 V or 5 V from mikroBUS), V.
    pub vdd: f64,
    /// Voltage applied to terminal A, V (divider mode).
    pub va: f64,
    /// Voltage applied to terminal B, V (divider mode).
    pub vb: f64,
    /// Load from W to ground, Ω; 0 = no load (high-impedance input).
    pub load_ohm: f64,
    /// Wiper resistance RW, Ω (75 Ω typical).
    pub rw_ohm: f64,
    /// Ambient temperature, °C.
    pub temp_c: f64,
    /// Simulator: how far the RAB of "this" part is from nominal, % (datasheet allows ±20 %).
    pub sim_rab_dev_pct: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            rab_nominal: 10_000.0,
            vdd: 3.3,
            va: 3.3,
            vb: 0.0,
            load_ohm: 0.0,
            rw_ohm: 75.0,
            temp_c: 25.0,
            sim_rab_dev_pct: 4.0,
        }
    }
}

/// RAB options (part number suffix, Ω).
pub const RAB_OPTIONS: [(&str, f64); 4] =
    [("-502", 5_000.0), ("-103", 10_000.0), ("-503", 50_000.0), ("-104", 100_000.0)];
/// Maximum current through A, W or B (electrical characteristics, Note 6).
pub const IW_MAX: f64 = 2.5e-3;
/// RAB tolerance (± 20 %).
pub const RAB_TOL: f64 = 0.20;
/// EEPROM write cycle time, typ / max (tWC).
pub const TWC_TYP_S: f64 = 5e-3;
pub const TWC_MAX_S: f64 = 10e-3;
/// EEPROM endurance, cycles (typ).
pub const EE_ENDURANCE: f64 = 1e6;

/// -3 dB bandwidth at code 80h, 30 pF load (electrical characteristics).
pub fn bandwidth_hz(rab_nominal: f64) -> f64 {
    match rab_nominal as u32 {
        0..=7_499 => 2e6,
        7_500..=29_999 => 1e6,
        30_000..=74_999 => 200e3,
        _ => 100e3,
    }
}

/// Typical RAB tempco, ppm/°C: 50 (−20…70 °C), 100 (−40…85 °C), 150 (−40…125 °C).
pub fn tempco_ppm(t: f64) -> f64 {
    if (-20.0..=70.0).contains(&t) {
        50.0
    } else if (-40.0..=85.0).contains(&t) {
        100.0
    } else {
        150.0
    }
}

/// RAB at temperature `t` for a part whose 25 °C value is `rab25`.
pub fn rab_at(rab25: f64, t: f64) -> f64 {
    rab25 * (1.0 + tempco_ppm(t) * 1e-6 * (t - 25.0))
}

/// Terminal connections after TCON and shutdown (§5.4).
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Terminals {
    pub a: bool,
    pub w: bool,
    pub b: bool,
    /// R0HW = 0: "shutdown" configuration — A open, wiper forced to B.
    pub shutdown: bool,
}

pub fn terminals(w: &Words) -> Terminals {
    let shutdown = regs::get(w, "R0HW") == 0;
    Terminals {
        a: !shutdown && regs::get(w, "R0A") == 1,
        w: regs::get(w, "R0W") == 1,
        b: regs::get(w, "R0B") == 1,
        shutdown,
    }
}

/// Electrical state of the potentiometer for a given code and RAB.
#[derive(Serialize, Clone, Copy, Debug, Default)]
pub struct Point {
    /// R between W and B incl. wiper resistance (Equation 5-2), Ω.
    pub rwb: f64,
    /// R between W and A incl. wiper resistance, Ω.
    pub rwa: f64,
    /// Open-circuit voltage at W, V (NaN if W floats).
    pub vw_open: f64,
    /// Voltage at W with the load, V (NaN if W floats).
    pub vw: f64,
    /// Output (Thevenin) resistance seen from W, Ω.
    pub rout: f64,
    /// Current through the load / wiper, A.
    pub iw: f64,
}

/// Node analysis of A —(RAW')— tap —(RWB')— B, tap —(RW)— W —(RL)— GND.
pub fn point(code: u32, rab: f64, s: &Settings, t: Terminals) -> Point {
    let n = code.min(FULL_SCALE) as f64;
    let r_wb_ladder = rab * n / 256.0;
    let r_wa_ladder = rab * (256.0 - n) / 256.0;
    let rwb = r_wb_ladder + s.rw_ohm;
    let rwa = r_wa_ladder + s.rw_ohm;
    // branches from the tap to the terminals: (resistance, terminal voltage)
    let mut br: Vec<(f64, f64)> = vec![];
    if t.shutdown {
        // wiper forced to terminal B through the switch
        if t.b {
            br.push((0.0, s.vb));
        }
    } else {
        if t.a {
            br.push((r_wa_ladder, s.va));
        }
        if t.b {
            br.push((r_wb_ladder, s.vb));
        }
    }
    if br.is_empty() || !t.w {
        let vw = if s.load_ohm > 0.0 { 0.0 } else { f64::NAN };
        return Point { rwb, rwa, vw_open: f64::NAN, vw, rout: f64::INFINITY, iw: 0.0 };
    }
    // Thevenin equivalent at the tap; a 0 Ω branch pins the tap exactly
    let shorts: Vec<f64> = br.iter().filter(|b| b.0 <= 0.0).map(|b| b.1).collect();
    let (vt, rth) = if !shorts.is_empty() {
        (shorts.iter().sum::<f64>() / shorts.len() as f64, 0.0)
    } else {
        let gsum: f64 = br.iter().map(|b| 1.0 / b.0).sum();
        (br.iter().map(|b| b.1 / b.0).sum::<f64>() / gsum, 1.0 / gsum)
    };
    let rout = rth + s.rw_ohm;
    let (vw, iw) = if s.load_ohm > 0.0 {
        let v = vt * s.load_ohm / (rout + s.load_ohm);
        (v, v / s.load_ohm)
    } else {
        (vt, 0.0)
    };
    Point { rwb, rwa, vw_open: vt, vw, rout, iw }
}

/// Typed results used by the simulator and other Rust code.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Calc {
    /// Wiper code as written (0…1FFh).
    pub code_raw: u32,
    /// Effective code 0…256 (codes above 100h act as full scale).
    pub code: u32,
    pub ratio: f64,
    pub rab: f64,
    pub rab_min: f64,
    pub rab_max: f64,
    pub rs: f64,
    pub p: Point,
    pub term: Terminals,
    /// Current through the ladder A→B, A.
    pub i_ab: f64,
    /// Worst-case wiper current at zero/full scale with the load, A.
    pub iw_worst: f64,
    /// Voltage of one step (VA − VB) / 256, V.
    pub lsb_v: f64,
    pub bw_hz: f64,
    pub tempco_ppm: f64,
    /// Code loaded into WIPER0 at the next power-up.
    pub nv_code: u32,
}

pub fn calc(w: &Words, s: &Settings) -> Calc {
    let code_raw = regs::get(w, "WIPER0");
    let code = code_raw.min(FULL_SCALE);
    // Equation 5-1: RS = RAB / 256 (8-bit device)
    let rab = rab_at(s.rab_nominal, s.temp_c);
    let term = terminals(w);
    let p = point(code, rab, s, term);
    let i_ab = if term.a && term.b && !term.shutdown { (s.va - s.vb).abs() / rab } else { 0.0 };
    let iw_worst = if s.load_ohm > 0.0 { s.va.abs().max(s.vb.abs()) / (s.rw_ohm + s.load_ohm) } else { 0.0 };
    Calc {
        code_raw,
        code,
        ratio: code as f64 / 256.0,
        rab,
        rab_min: s.rab_nominal * (1.0 - RAB_TOL),
        rab_max: s.rab_nominal * (1.0 + RAB_TOL),
        rs: rab / 256.0,
        p,
        term,
        i_ab,
        iw_worst,
        lsb_v: (s.va - s.vb).abs() / 256.0,
        bw_hz: bandwidth_hz(s.rab_nominal),
        tempco_ppm: tempco_ppm(s.temp_c),
        nv_code: regs::get(w, "NV_WIPER0"),
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Metric {
    pub key: &'static str,
    pub label: &'static str,
    pub value: f64,
    /// Base SI unit ("V", "Hz", "A", "Ω", "%", "") — the UI adds prefixes.
    pub unit: &'static str,
    pub block: &'static str,
    /// Show in the top bar.
    pub top: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct Issue {
    /// "error" | "warn" | "info"
    pub level: &'static str,
    pub block: &'static str,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Derived {
    pub metrics: Vec<Metric>,
    pub issues: Vec<Issue>,
    pub calc: Calc,
}

fn m(key: &'static str, label: &'static str, value: f64, unit: &'static str, block: &'static str, top: bool) -> Metric {
    Metric { key, label, value, unit, block, top }
}

fn kohm(r: f64) -> String {
    if r >= 1000.0 {
        format!("{:.3} кОм", r / 1000.0)
    } else {
        format!("{:.0} Ом", r)
    }
}

pub fn derive(w: &Words, s: &Settings) -> Derived {
    let c = calc(w, s);
    let mut metrics = vec![
        m("code", "Код N", c.code as f64, "", "wiper", true),
        m("rwb", "R_WB", c.p.rwb, "Ω", "ladder", true),
        m("vw", "V_W", c.p.vw, "V", "terminals", true),
        m("rs", "Шаг R_S", c.rs, "Ω", "ladder", true),
        m("rab", "R_AB", c.rab, "Ω", "ladder", true),
        m("rwa", "R_WA", c.p.rwa, "Ω", "ladder", false),
        m("ratio", "Положение", c.ratio * 100.0, "%", "wiper", false),
        m("lsb", "Шаг по напряжению", c.lsb_v, "V", "terminals", false),
        m("rout", "R_out (с W)", c.p.rout, "Ω", "terminals", false),
        m("iab", "Ток A→B", c.i_ab, "A", "terminals", false),
        m("bw", "Полоса −3 дБ", c.bw_hz, "Hz", "ladder", false),
        m("nv", "Код при включении", c.nv_code as f64, "", "eeprom", false),
    ];
    if s.load_ohm > 0.0 {
        metrics.push(m("iw", "Ток движка", c.p.iw, "A", "terminals", false));
    }

    let mut issues = vec![];
    let mut issue = |level, block, text: String| issues.push(Issue { level, block, text });

    // supply (electrical characteristics: 2.7…5.5 V specified, 1.8…2.7 V serial interface only)
    if s.vdd > 5.5 {
        issue(
            "error",
            "por",
            format!("VDD {:.2} В выше 5.5 В — вне рабочего диапазона (абсолютный максимум 7.0 В)", s.vdd),
        );
    } else if s.vdd < 1.8 {
        issue("error", "por", format!("VDD {:.2} В ниже 1.8 В — кристалл не работает", s.vdd));
    } else if s.vdd < 2.7 {
        issue("warn", "por", format!("VDD {:.2} В: при 1.8…2.7 В работает только последовательный интерфейс, характеристики резистора не гарантированы", s.vdd));
    }
    // terminal voltages must stay within VSS…VDD (Resistor Terminal Input Voltage Range)
    for (name, v) in [("A", s.va), ("B", s.vb)] {
        if v < 0.0 || v > s.vdd {
            issue("error", "terminals", format!("Напряжение на выводе {} {:.2} В вне диапазона VSS…VDD (0…{:.2} В). Абсолютный максимум −0.3…VDD + 0.3 В", name, v, s.vdd));
        }
    }
    if c.i_ab > IW_MAX {
        issue("error", "terminals", format!("Ток через цепочку A→B {:.2} мА больше допустимых 2.5 мА: уменьшите напряжение или возьмите вариант с большим R_AB", c.i_ab * 1e3));
    }
    if c.iw_worst > IW_MAX {
        issue(
            "error",
            "terminals",
            format!("Нагрузка {} на W: в крайних положениях ток движка дойдёт до {:.2} мА (предел 2.5 мА). Увеличьте нагрузку или поставьте буфер", kohm(s.load_ohm), c.iw_worst * 1e3),
        );
    }
    if s.load_ohm > 0.0 && c.p.vw_open.is_finite() && c.p.vw_open.abs() > 1e-9 {
        let err = 1.0 - c.p.vw / c.p.vw_open;
        if err > 0.01 {
            issue(
                "warn",
                "terminals",
                format!("Нагрузка {} проседает выход на {:.1} % (R_out = {}). Для делителя нужна нагрузка ≥ 100 × R_out или повторитель на ОУ", kohm(s.load_ohm), err * 100.0, kohm(c.p.rout)),
            );
        }
    }
    if c.code_raw > FULL_SCALE {
        issue("warn", "wiper", format!("Код {:03X}h больше 100h: это тоже полная шкала (W = A), но инкремент и декремент здесь игнорируются (табл. 5-1)", c.code_raw));
    }
    if c.term.shutdown {
        issue(
            "info",
            "tcon",
            "R0HW = 0: конфигурация отключения — вывод A разомкнут, движок соединён с B, через цепочку ток не течёт"
                .into(),
        );
    } else if !(c.term.a && c.term.w && c.term.b) {
        let off: Vec<&str> =
            [("A", c.term.a), ("W", c.term.w), ("B", c.term.b)].iter().filter(|x| !x.1).map(|x| x.0).collect();
        issue(
            "info",
            "tcon",
            format!(
                "TCON: отключены выводы {} — это режим пониженного потребления, делитель не работает",
                off.join(", ")
            ),
        );
    }
    if c.nv_code != c.code_raw {
        issue(
            "info",
            "eeprom",
            format!("После выключения питания движок вернётся к коду {} (NV_WIPER0), а сейчас стоит {}. Чтобы сохранить — «Записать в EEPROM»", c.nv_code, c.code_raw),
        );
    }
    issue(
        "info",
        "ladder",
        format!(
            "R_AB задан как {} ({}): сверьте суффикс на корпусе (502/103/503/104) или измерьте между A и B; допуск ±20 % ({} … {})",
            kohm(s.rab_nominal),
            RAB_OPTIONS.iter().find(|o| (o.1 - s.rab_nominal).abs() < 1.0).map(|o| o.0).unwrap_or("свой номинал"),
            kohm(c.rab_min),
            kohm(c.rab_max)
        ),
    );
    Derived { metrics, issues, calc: c }
}

/// Transfer characteristic over all 257 codes (for plots).
#[derive(Serialize, Clone, Debug)]
pub struct Curve {
    pub code: Vec<f64>,
    pub rwb: Vec<f64>,
    pub rwa: Vec<f64>,
    /// Ideal ratiometric divider VB + (VA − VB)·N/256, V.
    pub vw_ideal: Vec<f64>,
    /// With RW, terminal connections and the load, V.
    pub vw: Vec<f64>,
    /// ±1 LSB INL band around the ideal line (3.0 V ≤ VDD ≤ 5.5 V), V.
    pub inl_lo: Vec<f64>,
    pub inl_hi: Vec<f64>,
}

pub fn curve(w: &Words, s: &Settings) -> Curve {
    let rab = rab_at(s.rab_nominal, s.temp_c);
    let t = terminals(w);
    let lsb = (s.va - s.vb) / 256.0;
    let codes: Vec<u32> = (0..=FULL_SCALE).collect();
    let pts: Vec<Point> = codes.iter().map(|&n| point(n, rab, s, t)).collect();
    let ideal: Vec<f64> = codes.iter().map(|&n| s.vb + (s.va - s.vb) * n as f64 / 256.0).collect();
    Curve {
        code: codes.iter().map(|&n| n as f64).collect(),
        rwb: pts.iter().map(|p| p.rwb).collect(),
        rwa: pts.iter().map(|p| p.rwa).collect(),
        vw: pts.iter().map(|p| p.vw).collect(),
        inl_lo: ideal.iter().map(|v| v - lsb.abs()).collect(),
        inl_hi: ideal.iter().map(|v| v + lsb.abs()).collect(),
        vw_ideal: ideal,
    }
}

/// Code that gives the resistance `r` between W and B (rheostat), rounded.
pub fn code_for_rwb(r: f64, s: &Settings) -> u32 {
    let rab = rab_at(s.rab_nominal, s.temp_c);
    (((r - s.rw_ohm) / rab * 256.0).round().clamp(0.0, 256.0)) as u32
}

/// Code that gives the open-circuit voltage `v` at W (divider), rounded.
pub fn code_for_vw(v: f64, s: &Settings) -> u32 {
    if (s.va - s.vb).abs() < 1e-12 {
        return 0;
    }
    (((v - s.vb) / (s.va - s.vb) * 256.0).round().clamp(0.0, 256.0)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_code(n: u32) -> Words {
        let mut w = regs::starter();
        regs::set(&mut w, "WIPER0", n);
        w
    }

    #[test]
    fn starter_has_no_errors() {
        let d = derive(&regs::starter(), &Settings::default());
        assert!(d.issues.iter().all(|i| i.level != "error"), "{:?}", d.issues);
    }

    #[test]
    fn equation_5_2() {
        // RWB = RAB·N/256 + RW: 10 kΩ, N = 128 → 5000 + 75 Ω
        let s = Settings::default();
        let c = calc(&with_code(128), &s);
        assert!((c.p.rwb - 5075.0).abs() < 1e-9);
        assert!((c.p.rwa - 5075.0).abs() < 1e-9);
        assert!((c.rs - 10_000.0 / 256.0).abs() < 1e-9);
        // zero scale: only the wiper resistance remains between W and B
        assert!((calc(&with_code(0), &s).p.rwb - 75.0).abs() < 1e-9);
        // codes above 100h act as full scale
        assert_eq!(calc(&with_code(0x1FF), &s).code, 256);
    }

    #[test]
    fn unloaded_divider_is_ratiometric() {
        let s = Settings::default();
        for n in [0, 1, 64, 128, 200, 256] {
            let c = calc(&with_code(n), &s);
            assert!((c.p.vw - 3.3 * n as f64 / 256.0).abs() < 1e-9, "n={}", n);
        }
    }

    #[test]
    fn load_sags_output_and_warns() {
        let s = Settings { load_ohm: 10_000.0, ..Settings::default() };
        let d = derive(&with_code(128), &s);
        // Thevenin: 1.65 V behind 2.5 kΩ + 75 Ω into 10 kΩ
        let expect = 1.65 * 10_000.0 / (2_575.0 + 10_000.0);
        assert!((d.calc.p.vw - expect).abs() < 1e-9);
        assert!(d.issues.iter().any(|i| i.level == "warn" && i.text.contains("проседает")));
    }

    #[test]
    fn limits_raise_errors() {
        let over = Settings { va: 4.0, ..Settings::default() };
        assert!(derive(&regs::starter(), &over).issues.iter().any(|i| i.level == "error" && i.block == "terminals"));
        let heavy = Settings { load_ohm: 1_000.0, ..Settings::default() };
        assert!(derive(&regs::starter(), &heavy)
            .issues
            .iter()
            .any(|i| i.level == "error" && i.text.contains("2.5 мА")));
        let low = Settings { rab_nominal: 5_000.0, vdd: 5.0, va: 5.0, ..Settings::default() };
        assert!(derive(&regs::starter(), &low).issues.iter().all(|i| i.level != "error"), "5 V on 5 kΩ = 1 mA is fine");
    }

    #[test]
    fn shutdown_forces_w_to_b() {
        let mut w = with_code(200);
        regs::set(&mut w, "R0HW", 0);
        let c = calc(&w, &Settings { vb: 0.5, ..Settings::default() });
        assert!((c.p.vw - 0.5).abs() < 1e-6);
        assert_eq!(c.i_ab, 0.0);
    }

    #[test]
    fn inverse_helpers() {
        let s = Settings::default();
        assert_eq!(code_for_vw(1.65, &s), 128);
        assert_eq!(code_for_rwb(5_075.0, &s), 128);
    }

    #[test]
    fn tempco() {
        assert!((rab_at(10_000.0, 75.0) - 10_000.0 * (1.0 + 100e-6 * 50.0)).abs() < 1e-9);
        assert_eq!(bandwidth_hz(100_000.0), 100e3);
    }
}
