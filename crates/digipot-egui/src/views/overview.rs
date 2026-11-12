//! Overview: live functional diagram of the chip and the selected block's fields.

use crate::app::{Act, Shared};
use crate::canvas::Canvas;
use crate::content::{block, BlockDef, BLOCKS, WIRES};
use crate::fmt::eng;
use crate::theme::mix;
use crate::widgets as w;
use digipot_core::regs::{Access, FIELDS};
use eframe::egui::{Align2, RichText, Stroke, Ui};

fn anchor(b: &BlockDef, side: char) -> (f32, f32) {
    match side {
        'l' => (b.x, b.y + b.h / 2.0),
        'r' => (b.x + b.w, b.y + b.h / 2.0),
        't' => (b.x + b.w / 2.0, b.y),
        _ => (b.x + b.w / 2.0, b.y + b.h),
    }
}

/// Orthogonal wire between two blocks (left→right, or vertical when stacked).
fn wire_path(a: &BlockDef, b: &BlockDef) -> Vec<(f32, f32)> {
    let vertical = ((a.x + a.w / 2.0) - (b.x + b.w / 2.0)).abs() < 40.0;
    if vertical {
        let (x1, y1) = anchor(a, if a.y < b.y { 'b' } else { 't' });
        let (_, y2) = anchor(b, if a.y < b.y { 't' } else { 'b' });
        return vec![(x1, y1), (x1, y2)];
    }
    let (x1, y1) = anchor(a, 'r');
    let (x2, y2) = anchor(b, 'l');
    if x2 < x1 {
        let (tx, ty) = anchor(a, if b.y < a.y { 't' } else { 'b' });
        let (ux, uy) = anchor(b, if b.y < a.y { 'b' } else { 't' });
        let my = (ty + uy) / 2.0;
        return vec![(tx, ty), (tx, my), (ux, my), (ux, uy)];
    }
    let mx = (x1 + x2) / 2.0;
    vec![(x1, y1), (mx, y1), (mx, y2), (x2, y2)]
}

fn diagram(ui: &mut Ui, sh: &Shared) {
    let p = sh.pal;
    let active = sh.st.sim.as_ref().map(|s| s.state != "off").unwrap_or(true);
    let (c, _) = Canvas::new(ui, [60.0, 118.0, 1120.0, 490.0], 560.0);
    c.rect(80.0, 170.0, 1080.0, 420.0, 28.0, p.panel2, Stroke::new(2.0, p.line2));
    c.text_bold(104.0, 198.0, Align2::LEFT_CENTER, "Microchip MCP4161", 15.0, p.muted);
    c.text(80.0, 136.0, Align2::LEFT_CENTER, "Нажмите на блок, чтобы увидеть его параметры и регистры", 13.0, p.muted);

    // animated "data flow" along the wires
    let offset = (sh.time as f32 * 24.0) % 14.0;
    for (from, to, tone) in WIRES {
        let (Some(a), Some(b)) = (block(from), block(to)) else { continue };
        let path = wire_path(a, b);
        let col = p.tone(tone);
        c.line(&path, col.gamma_multiply(0.35), 5.0);
        if active {
            c.dashed(&path, col, 2.5, 8.0, 6.0, -offset);
        }
    }
    if active {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(40));
    }

    for b in BLOCKS {
        let sel = sh.block == Some(b.id);
        let iss = sh.st.derived.issues.iter().find(|i| i.block == b.id && i.level != "info");
        let metric = sh.st.derived.metrics.iter().find(|m| m.block == b.id);
        let tone = p.tone(b.tone);
        let stroke_c = match iss {
            Some(i) if i.level == "error" => p.err,
            Some(_) => p.warn,
            None => tone,
        };
        let hit = c.hit(ui, b.id, b.x, b.y, b.w, b.h);
        let fill = mix(tone, if sel || hit.hovered() { 22.0 } else { 10.0 }, p.panel);
        c.rect(b.x, b.y, b.w, b.h, 16.0, fill, Stroke::new(if sel { 3.0 } else { 1.6 }, stroke_c));
        let cy = b.y + b.h / 2.0;
        c.text_bold(
            b.x + b.w / 2.0,
            if metric.is_some() { cy - 12.0 } else { cy },
            Align2::CENTER_CENTER,
            b.title,
            16.0,
            p.fg,
        );
        if let Some(m) = metric {
            c.text(b.x + b.w / 2.0, cy + 14.0, Align2::CENTER_CENTER, eng(m.value, m.unit, 4), 13.0, tone);
        }
        if hit.on_hover_text(b.desc).clicked() {
            sh.act(Act::SelectBlock(if sel { None } else { Some(b.id) }));
        }
    }
}

pub fn show(ui: &mut Ui, sh: &Shared) {
    let p = sh.pal;
    w::page_header(
        ui,
        &p,
        "🔲",
        "Обзор кристалла",
        "Microchip MCP4161 на плате DIGI POT click (MIKROE-923). Блок-схема живая: значения пересчитываются моделью при каждом изменении регистров.",
        |_| {},
    );
    let b = sh.block.and_then(block);
    w::two_cols(
        ui,
        420.0,
        &mut (),
        |ui, _| {
            w::card(ui, &p, "", "", |ui| diagram(ui, sh));
            let ms = &sh.st.derived.metrics;
            w::auto_grid(ui, 160.0, ms.len(), |ui, i| {
                let m = &ms[i];
                let tone = block(m.block).map(|b| p.tone(b.tone));
                w::stat(ui, &p, m.label, &eng(m.value, m.unit, 4), tone, "");
            });
        },
        |ui, _| {
            let (title, sub) = b.map(|b| (b.title, b.desc)).unwrap_or(("Все блоки", "Выберите блок на схеме"));
            w::card(ui, &p, title, sub, |ui| {
                let in_block = |id: &str| b.map(|b| b.id == id).unwrap_or(true);
                for i in sh.st.derived.issues.iter().filter(|i| in_block(i.block)) {
                    let (icon, c) = match i.level {
                        "error" => ("⛔", p.err),
                        "warn" => ("⚠", p.warn),
                        _ => ("ℹ", p.accent),
                    };
                    w::hint(ui, &p, c, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.label(RichText::new(icon).color(c));
                            w::para(ui, RichText::new(&i.text).size(12.0).color(p.fg2));
                        });
                    });
                }
                if let Some(b) = b {
                    let ms: Vec<_> = sh.st.derived.metrics.iter().filter(|m| m.block == b.id).collect();
                    w::auto_grid(ui, 150.0, ms.len(), |ui, i| {
                        w::stat(ui, &p, ms[i].label, &eng(ms[i].value, ms[i].unit, 4), Some(p.tone(b.tone)), "");
                    });
                }
                let fields = FIELDS.iter().filter(|f| in_block(f.block));
                for f in fields.clone().filter(|f| f.access == Access::Rw) {
                    w::field_control(ui, sh, f, false, false);
                }
                for f in fields.filter(|f| f.access != Access::Rw) {
                    w::field_control(ui, sh, f, true, false);
                }
            });
        },
    );
}
