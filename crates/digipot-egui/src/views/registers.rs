//! Register map: bit grids, per-register write/read, field editor.

use crate::app::{Act, Shared, View};
use crate::fmt::hex_w;
use crate::theme::mix;
use crate::widgets::{self as w, Btn};
use digipot_core::bus;
use digipot_core::regs::{self, Access, FieldDef, DATA_BITS, FIELDS, REGS};
use eframe::egui::{self, Color32, RichText, Sense, Stroke, Ui};
use serde_json::json;

const PALETTE: [u32; 12] = [
    0x6366f1, 0x06b6d4, 0xf59e0b, 0x10b981, 0xec4899, 0x8b5cf6, 0xef4444, 0x14b8a6, 0xeab308, 0x3b82f6, 0xf97316,
    0x84cc16,
];

#[derive(Default)]
pub struct RegState {
    reg: usize,
    /// Last focus we followed (to jump to its register once).
    seen_focus: Option<&'static str>,
}

fn fields_of(r: usize) -> Vec<&'static FieldDef> {
    FIELDS.iter().filter(|f| f.reg == r).collect()
}

fn writable(r: usize) -> bool {
    !regs::nv(r) && FIELDS.iter().any(|f| f.reg == r && f.access == Access::Rw)
}

fn color_of(i: usize) -> Color32 {
    let v = PALETTE[i % PALETTE.len()];
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub fn show(ui: &mut Ui, sh: &Shared, rs: &mut RegState) {
    let p = sh.pal;
    if sh.focus != rs.seen_focus {
        rs.seen_focus = sh.focus;
        if let Some(f) = sh.focus.and_then(regs::field) {
            rs.reg = f.reg;
        }
    }
    let can_read = sh.st.link.readback && bus::can_read();
    w::page_header(
        ui,
        &p,
        "🔢",
        "Карта регистров",
        &format!(
            "{} регистров × {} бит · {}. Щелчок по биту инвертирует его, по имени поля — открывает редактор.",
            REGS.len(),
            DATA_BITS,
            bus::describe()
        ),
        |ui| {
            if w::button(ui, &p, "Импорт", Btn::Ghost).clicked() {
                sh.act(Act::Import);
            }
            if w::button(ui, &p, "Экспорт", Btn::Ghost).clicked() {
                sh.act(Act::Export);
            }
            if w::button(ui, &p, "По даташиту", Btn::Ghost).on_hover_text("Значения после сброса по даташиту").clicked()
            {
                sh.rpc("reset_defaults", json!({ "starter": false }));
            }
            if w::button(ui, &p, "⟲ Стартовая", Btn::Default).on_hover_text("Рабочая стартовая конфигурация").clicked()
            {
                sh.rpc("reset_defaults", json!({ "starter": true }));
            }
            if ui
                .add_enabled(can_read, w::btn(&p, "⬇ Прочитать всё", Btn::Default))
                .on_hover_text("Прочитать все регистры")
                .clicked()
            {
                sh.rpc("read_all", json!({}));
            }
            if w::button(ui, &p, "➤ Записать всё", Btn::Default).on_hover_text("Записать всю конфигурацию").clicked()
            {
                sh.rpc("write_all", json!({}));
            }
        },
    );
    w::two_cols(
        ui,
        420.0,
        rs,
        |ui, rs| {
            for r in 0..REGS.len() {
                reg_card(ui, sh, rs, r, can_read);
            }
        },
        |ui, rs| {
            let rd = &REGS[rs.reg];
            w::card(ui, &p, &format!("Поля {}", rd.name), rd.title, |ui| {
                for f in fields_of(rs.reg) {
                    let hl = sh.focus == Some(f.name);
                    w::field_control(ui, sh, f, false, hl);
                }
            });
        },
    );
}

fn reg_card(ui: &mut Ui, sh: &Shared, rs: &mut RegState, r: usize, can_read: bool) {
    let p = sh.pal;
    let rd = &REGS[r];
    let st = &sh.st;
    let rw = writable(r);
    let sel = r == rs.reg;
    let resp = egui::Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, if sel { p.accent } else { p.line }))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(rd.name).strong().size(13.0));
                ui.label(RichText::new(hex_w(rd.addr as u32, 8)).monospace().size(11.0).color(p.muted));
                if let Some(v) = w::num_field(ui, st.regs[r] as i64, 0, regs::data_mask() as i64, true, rw) {
                    sh.rpc("set_register", json!({ "index": r, "value": v as u32 }));
                }
                if regs::nv(r) {
                    if w::badge(ui, &p, "EEPROM →", p.refc)
                        .interact(Sense::click())
                        .on_hover_text("Запись в EEPROM — на экране «EEPROM», с подтверждением")
                        .clicked()
                    {
                        sh.go(View::Memory);
                    }
                } else if rw {
                    if st.dirty[r] {
                        w::badge(ui, &p, "не записан", p.warn);
                    } else {
                        w::badge(ui, &p, "записан", p.ok);
                    }
                } else {
                    w::badge(ui, &p, "статус", p.muted);
                }
                if let (Some(m), Some(word)) = (st.readback.matches[r], st.readback.words[r]) {
                    if m {
                        w::badge(ui, &p, &format!("чтение ✓ {}", hex_w(word, DATA_BITS)), p.ok);
                    } else {
                        w::badge(ui, &p, &format!("чтение ≠ {}", hex_w(word, DATA_BITS)), p.err);
                    }
                }
                if let Some(dev) = st.dev_regs.map(|d| d[r]) {
                    if rw && dev != st.desired[r] {
                        ui.label(
                            RichText::new(format!("в кристалле {}", hex_w(dev, DATA_BITS)))
                                .monospace()
                                .size(11.0)
                                .color(p.muted),
                        );
                    }
                }
                ui.label(RichText::new(rd.title).size(11.5).color(p.muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(can_read, w::btn(&p, "R", Btn::Ghost))
                        .on_hover_text(format!("Прочитать {}", rd.name))
                        .clicked()
                    {
                        sh.rpc("read_register", json!({ "index": r }));
                    }
                    if ui
                        .add_enabled(rw, w::btn(&p, "W", Btn::Ghost))
                        .on_hover_text(format!("Записать {}", rd.name))
                        .clicked()
                    {
                        sh.rpc("write_register", json!({ "index": r }));
                    }
                });
            });
            ui.add_space(4.0);
            bit_grid(ui, sh, rs, r, rw);
        })
        .response
        .interact(Sense::click());
    if resp.clicked() {
        rs.reg = r;
    }
}

fn bit_grid(ui: &mut Ui, sh: &Shared, rs: &mut RegState, r: usize, rw: bool) {
    let p = sh.pal;
    let bits = DATA_BITS as usize;
    let word = sh.st.regs[r];
    let fields = fields_of(r);
    let owner = |b: usize| fields.iter().position(|f| (f.lsb as usize..=f.msb as usize).contains(&b));
    let gap = 2.0;
    let w = ui.available_width();
    let cw = (w - gap * (bits as f32 - 1.0)) / bits as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 28.0 + 12.0 + 20.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let col_x = |i: usize| rect.left() + i as f32 * (cw + gap);

    for i in 0..bits {
        let b = bits - 1 - i;
        let cell = egui::Rect::from_min_size(egui::pos2(col_x(i), rect.top()), egui::vec2(cw, 28.0));
        let v = (word >> b) & 1;
        let o = owner(b);
        let resp = ui.interact(cell, ui.id().with(("bit", r, b)), Sense::click());
        let (fill, stroke) = match o {
            Some(k) => {
                let c = color_of(k);
                let pct = if v == 1 { 55.0 } else { 14.0 } + if resp.hovered() { 12.0 } else { 0.0 };
                (mix(c, pct, p.panel), mix(c, 50.0, p.panel))
            }
            None => (p.panel2, p.line),
        };
        let focus = o.map(|k| sh.focus == Some(fields[k].name)).unwrap_or(false);
        painter.rect(
            cell,
            5.0,
            fill,
            Stroke::new(if focus { 2.0 } else { 1.0 }, if focus { p.accent } else { stroke }),
            egui::StrokeKind::Inside,
        );
        painter.text(
            cell.center(),
            egui::Align2::CENTER_CENTER,
            v.to_string(),
            egui::FontId::monospace(11.0),
            if v == 1 { p.fg } else { p.muted },
        );
        if b % 4 == 3 || b == 0 {
            painter.text(
                egui::pos2(cell.center().x, cell.bottom() + 6.0),
                egui::Align2::CENTER_CENTER,
                b.to_string(),
                egui::FontId::monospace(9.0),
                p.muted,
            );
        }
        if let Some(k) = o {
            let f = fields[k];
            let resp = resp.on_hover_text(format!("{}: {} — {}", b, f.name, f.title));
            if resp.clicked() {
                rs.reg = r;
                if f.access != Access::Rw || !rw {
                    sh.act(Act::Focus(Some(f.name)));
                } else {
                    sh.rpc("set_register", json!({ "index": r, "value": word ^ (1 << b) }));
                }
            }
        } else {
            resp.on_hover_text(format!("{}: не используется", b));
        }
    }
    // field name bars
    let y = rect.top() + 28.0 + 14.0;
    for (k, f) in fields.iter().enumerate() {
        let i0 = bits - 1 - f.msb as usize;
        let i1 = bits - 1 - f.lsb as usize;
        let bar = egui::Rect::from_min_max(egui::pos2(col_x(i0), y), egui::pos2(col_x(i1) + cw, y + 18.0));
        let c = color_of(k);
        let resp = ui.interact(bar, ui.id().with(("fbar", r, k)), Sense::click()).on_hover_text(format!(
            "{} {}",
            f.name,
            w::bits_label(f)
        ));
        painter.rect_filled(bar, 4.0, mix(c, if resp.hovered() { 30.0 } else { 18.0 }, p.panel));
        let galley = painter.layout_no_wrap(f.name.to_string(), egui::FontId::monospace(9.5), c);
        let pos = bar.center() - galley.size() / 2.0;
        if galley.size().x <= bar.width() {
            painter.galley(pos, galley, c);
        }
        if resp.clicked() {
            rs.reg = r;
            sh.act(Act::Focus(Some(f.name)));
        }
    }
}
