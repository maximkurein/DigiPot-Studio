//! EEPROM: power-up wiper and the ten data cells. Every write is a two-step
//! confirmation and goes through `nv_write {confirm: true}`.

use crate::app::{Act, Shared};
use crate::fmt::hex3;
use crate::widgets::{self as w, Btn};
use digipot_core::regs::{self, R_NV_WIPER0};
use eframe::egui::{RichText, Ui};
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const EE_CELLS: [&str; 10] = ["EE06", "EE07", "EE08", "EE09", "EE0A", "EE0B", "EE0C", "EE0D", "EE0E", "EE0F"];

#[derive(Default)]
pub struct MemoryState {
    nv_draft: Option<u32>,
    drafts: HashMap<&'static str, u32>,
    /// Which confirm button is armed, and until when.
    armed: Option<(&'static str, Instant)>,
}

/// Two-step button: the first click arms it (5 s), the second performs the write.
fn confirm_button(
    ui: &mut Ui,
    sh: &Shared,
    ms: &mut MemoryState,
    key: &'static str,
    label: &str,
    confirm: &str,
    enabled: bool,
) -> bool {
    let p = sh.pal;
    let armed = matches!(ms.armed, Some((k, t)) if k == key && t > Instant::now());
    if !armed {
        if ui.add_enabled(enabled, w::btn(&p, label, Btn::Default)).clicked() {
            ms.armed = Some((key, Instant::now() + Duration::from_secs(5)));
        }
        return false;
    }
    ui.ctx().request_repaint_after(Duration::from_millis(500));
    let mut go = false;
    let mut cancel = false;
    ui.with_layout(eframe::egui::Layout::left_to_right(eframe::egui::Align::Center), |ui| {
        go = w::button(ui, &p, confirm, Btn::Danger).clicked();
        cancel = w::button(ui, &p, "Отмена", Btn::Ghost).clicked();
    });
    if go || cancel {
        ms.armed = None;
    }
    go
}

fn nv_write(sh: &Shared, reg: usize, value: u32) {
    sh.rpc("nv_write", json!({ "index": reg, "value": value, "confirm": true }));
}

pub fn show(ui: &mut Ui, sh: &Shared, ms: &mut MemoryState) {
    let p = sh.pal;
    w::page_header(
        ui,
        &p,
        "💾",
        "Память EEPROM",
        "Положение движка при включении и 10 ячеек пользовательских данных. Каждая запись — один цикл из ~1 млн и до 10 мс ожидания, поэтому здесь она только по подтверждению.",
        |ui| {
            if w::button(ui, &p, "⬇ Прочитать EEPROM", Btn::Default).on_hover_text("Прочитать NV_WIPER0 и ячейки 06h…0Fh").clicked() {
                sh.act(Act::ReadNv);
            }
        },
    );
    w::two_cols(ui, 380.0, ms, |ui, ms| left(ui, sh, ms), |ui, _| right(ui, sh));
}

fn left(ui: &mut Ui, sh: &Shared, ms: &mut MemoryState) {
    let p = sh.pal;
    let wiper_raw = sh.field("WIPER0");
    let nv = sh.field("NV_WIPER0");
    let wiper = wiper_raw.min(0x100);

    w::card(
        ui,
        &p,
        "Движок при включении питания",
        "POR копирует NV_WIPER0 (02h) в WIPER0 (00h)",
        |ui| {
            let same = nv == wiper_raw;
            let tiles = [
                ("Сейчас (WIPER0)", wiper_raw.to_string(), hex3(wiper_raw), p.accent),
                ("В EEPROM (NV_WIPER0)", nv.to_string(), hex3(nv), p.refc),
                (
                    "После выключения",
                    if same { "то же".to_string() } else { format!("вернётся к {}", nv) },
                    String::new(),
                    if same { p.ok } else { p.warn },
                ),
            ];
            w::auto_grid(ui, 170.0, 3, |ui, i| {
                let (l, v, s, t) = &tiles[i];
                w::stat(ui, &p, l, v, Some(*t), s);
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if confirm_button(
                    ui,
                    sh,
                    ms,
                    "nv_cur",
                    "💾 Сохранить текущее положение",
                    &format!("Записать {} в EEPROM", wiper),
                    nv != wiper,
                ) {
                    nv_write(sh, R_NV_WIPER0, wiper);
                }
                ui.label(RichText::new("или значение").size(12.0).color(p.muted));
                let draft = ms.nv_draft.unwrap_or(nv);
                if let Some(v) = w::num_field(ui, draft as i64, 0, 0x1FF, false, true) {
                    ms.nv_draft = Some(v as u32);
                }
                if confirm_button(ui, sh, ms, "nv_val", "Записать", &format!("Записать {}", draft), true)
                {
                    nv_write(sh, R_NV_WIPER0, draft);
                    ms.nv_draft = None;
                }
                ui.add_space(12.0);
                if sh.is_sim() {
                    if w::button(ui, &p, "⏻ Цикл питания", Btn::Default)
                        .on_hover_text("Выключить и включить питание виртуального кристалла")
                        .clicked()
                    {
                        sh.rpc("power_cycle", json!({}));
                    }
                } else {
                    w::badge(ui, &p, "цикл питания — вручную на плате", p.muted);
                }
            });
        },
    );

    w::card(
        ui,
        &p,
        "Ячейки данных 06h…0Fh",
        "9 бит каждая, с завода 0FFh. Удобно хранить калибровку R_AB, серийный номер, настройки",
        |ui| {
            for n in EE_CELLS {
                let Some(fd) = regs::field(n) else { continue };
                let v = sh.field(n);
                let rb = sh.st.readback.words.get(fd.reg).copied().flatten();
                let draft = *ms.drafts.get(n).unwrap_or(&v);
                egui_row(ui, &p, |ui| {
                    ui.label(RichText::new(format!("{}h", &n[2..])).monospace().size(12.0));
                    ui.add_space(14.0);
                    ui.label(RichText::new(hex3(v)).monospace().size(12.5));
                    ui.add_space(14.0);
                    let t = rb.map(|x| format!("прочитано {}", hex3(x))).unwrap_or_else(|| "не читалось".into());
                    ui.label(RichText::new(t).size(11.0).color(p.muted));
                    ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                        // right-to-left layout: the button is added first so it ends up rightmost
                        let key: &'static str = n;
                        let go = confirm_button(ui, sh, ms, key, "Записать", "Подтвердить", draft != v);
                        if let Some(x) = w::num_field(ui, draft as i64, 0, 0x1FF, true, true) {
                            ms.drafts.insert(n, x as u32);
                        }
                        if go {
                            nv_write(sh, fd.reg, draft);
                            ms.drafts.remove(n);
                        }
                    });
                });
            }
        },
    );
}

fn egui_row(ui: &mut Ui, p: &crate::theme::Pal, add: impl FnOnce(&mut Ui)) {
    eframe::egui::Frame::new()
        .fill(p.panel2)
        .stroke(eframe::egui::Stroke::new(1.0, p.line))
        .corner_radius(8)
        .inner_margin(eframe::egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(add);
        });
}

fn right(ui: &mut Ui, sh: &Shared) {
    let p = sh.pal;
    w::card(ui, &p, "Ресурс EEPROM", "Endurance 1 млн циклов, tWC 5 мс (макс. 10 мс)", |ui| {
        let sim_writes = sh
            .st
            .sim
            .as_ref()
            .and_then(|s| s.signals.iter().find(|x| x.name.starts_with("Записей EEPROM")).map(|x| x.value));
        let mut tiles: Vec<(&str, String, &str, Option<_>)> =
            vec![("Записей за сеанс", sh.st.ee_writes.to_string(), "", (sh.st.ee_writes > 0).then_some(p.warn))];
        if let Some(n) = sim_writes {
            tiles.push(("Записей в этот кристалл", format!("{:.0}", n), "симулятор", None));
        }
        tiles.push(("Раз в секунду", "≈ 12 сут".into(), "до исчерпания ресурса", None));
        tiles.push(("Раз в минуту", "≈ 1.9 года".into(), "до исчерпания ресурса", None));
        w::auto_grid(ui, 150.0, tiles.len(), |ui, i| {
            let (l, v, s, t) = &tiles[i];
            w::stat(ui, &p, l, v, *t, s);
        });
        ui.label(RichText::new("Ресурс: 1 000 000 циклов на ячейку.").size(11.0).color(p.muted));
    });
    w::hint_text(
        ui,
        &p,
        p.warn,
        "Библиотека MikroE для DIGI POT click при каждой установке движка шлёт кадр 00 nn 20 nn: пишет и WIPER0, и NV_WIPER0. Если двигать движок в цикле, EEPROM изнашивается. Пишите в EEPROM только итоговое значение.",
    );
    w::hint_text(
        ui,
        &p,
        p.accent,
        "Пока идёт запись (STATUS.EEWA = 1), кристалл принимает команды только к 00h, 01h, 04h, 05h. Любая команда в том же кадре после записи в EEPROM — ошибка (CMDERR). Приложение ждёт 10 мс после каждой записи.",
    );
    w::hint_text(
        ui,
        &p,
        p.accent,
        "🛡 WiperLock и защита записи (WP) включаются командами с высоким напряжением на CS (V_IHH). Click такого напряжения не формирует, поэтому в приложении они только отображаются.",
    );
}
