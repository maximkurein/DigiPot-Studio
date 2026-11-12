//! Board: top view of DIGI POT click with clickable hotspots, checklist, safety.

use crate::app::{Shared, View};
use crate::canvas::Canvas;
use crate::content::{
    HotKind, BOARD_NAME, BOARD_SIZE, BOARD_SUPPLY, CHECKLIST, HOTSPOTS, MIKROBUS_LEFT, MIKROBUS_RIGHT, SAFETY,
    USED_PINS,
};
use crate::theme::mix;
use crate::widgets::{self as w, Btn};
use eframe::egui::{Align2, RichText, Stroke, Ui};

pub struct BoardState {
    sel: &'static str,
    done: Vec<bool>,
}

impl Default for BoardState {
    fn default() -> Self {
        BoardState { sel: HOTSPOTS[0].id, done: vec![false; CHECKLIST.len()] }
    }
}

pub fn show(ui: &mut Ui, sh: &Shared, bs: &mut BoardState) {
    let p = sh.pal;
    w::page_header(
        ui,
        &p,
        "📟",
        &format!("Плата {}", BOARD_NAME),
        &format!("{} · питание: {}. Нажмите на элемент платы, чтобы узнать его назначение.", BOARD_SIZE, BOARD_SUPPLY),
        |_| {},
    );
    w::two_cols(
        ui,
        400.0,
        bs,
        |ui, bs| {
            w::card(ui, &p, "", "", |ui| drawing(ui, sh, bs));
        },
        |ui, bs| {
            if let Some(h) = HOTSPOTS.iter().find(|h| h.id == bs.sel) {
                w::card(ui, &p, &format!("{} — {}", h.refdes, h.title), h.kind.label(), |ui| {
                    w::para(ui, RichText::new(h.desc).size(12.5).color(p.fg2));
                    if let Some(b) = h.block {
                        ui.add_space(6.0);
                        if w::button(ui, &p, "Показать на блок-схеме", Btn::Default).clicked() {
                            sh.go_block(View::Overview, Some(b));
                        }
                    }
                });
            }
            w::card(
                ui,
                &p,
                "Первое включение",
                "Отмечайте шаги по мере выполнения",
                |ui| {
                    for (i, c) in CHECKLIST.iter().enumerate() {
                        let text = if bs.done[i] {
                            RichText::new(*c).size(12.5).color(p.muted).strikethrough()
                        } else {
                            RichText::new(*c).size(12.5).color(p.fg2)
                        };
                        ui.horizontal_top(|ui| {
                            ui.checkbox(&mut bs.done[i], "");
                            w::para(ui, text);
                        });
                    }
                },
            );
            w::card(ui, &p, "🛡 Безопасность", "", |ui| {
                for s in SAFETY {
                    w::hint_text(ui, &p, p.warn, s);
                }
            });
            ui.horizontal_wrapped(|ui| {
                for k in HotKind::ALL {
                    w::badge(ui, &p, &format!("● {}", k.label()), p.tone(k.tone()));
                }
            });
        },
    );
}

fn drawing(ui: &mut Ui, sh: &Shared, bs: &mut BoardState) {
    let p = sh.pal;
    let (c, _) = Canvas::new(ui, [0.0, 0.0, 1000.0, 600.0], 640.0);
    c.rect(20.0, 20.0, 960.0, 560.0, 26.0, mix(p.ok, 12.0, p.panel2), Stroke::new(2.0, p.line2));
    for x in [60.0, 940.0] {
        for y in [60.0, 540.0] {
            c.circle(x, y, 12.0, p.panel, Stroke::new(1.0, p.line2));
        }
    }
    for h in HOTSPOTS {
        let col = p.tone(h.kind.tone());
        let on = h.id == bs.sel;
        let hit = c.hit(ui, h.id, h.x, h.y, h.w, h.h);
        let pct = if on {
            35.0
        } else if hit.hovered() {
            24.0
        } else {
            16.0
        };
        c.rect(h.x, h.y, h.w, h.h, 8.0, mix(col, pct, p.panel), Stroke::new(if on { 3.0 } else { 1.5 }, col));
        if h.id.starts_with("mb-") {
            let left = h.id == "mb-l";
            let pins = if left { MIKROBUS_LEFT } else { MIKROBUS_RIGHT };
            for (i, pin) in pins.iter().enumerate() {
                let used = USED_PINS.contains(pin);
                let cy = h.y + 38.0 + i as f32 * 50.0;
                let px = if left { h.x + 12.0 } else { h.x + h.w - 36.0 };
                c.rect(px, cy - 12.0, 24.0, 24.0, 4.0, if used { p.warn } else { p.panel2 }, Stroke::new(1.0, p.line2));
                let tx = if left { h.x + 44.0 } else { h.x + h.w - 44.0 };
                let label = if *pin == "MISO" { "MISO·NC" } else { pin };
                let align = if left { Align2::LEFT_CENTER } else { Align2::RIGHT_CENTER };
                if used {
                    c.text_bold(tx, cy, align, label, 13.0, p.fg);
                } else {
                    c.text(tx, cy, align, label, 13.0, p.muted);
                }
            }
        } else {
            c.text_bold(h.x + h.w / 2.0, h.y + h.h / 2.0, Align2::CENTER_CENTER, h.refdes, 14.0, p.fg);
        }
        if hit.on_hover_text(h.title).clicked() {
            bs.sel = h.id;
        }
    }
}
