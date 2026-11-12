//! Academy: short lessons with a "try it" jump.

use crate::app::Shared;
use crate::content::LESSONS;
use crate::widgets::{self as w, Btn};
use eframe::egui::{self, RichText, Sense, Stroke, Ui};

pub fn show(ui: &mut Ui, sh: &Shared, cur: &mut usize) {
    let p = sh.pal;
    w::page_header(
        ui,
        &p,
        "🎓",
        "Академия",
        "Как устроен кристалл — от принципа работы до настройки регистров. Каждый урок можно сразу попробовать в приложении.",
        |_| {},
    );
    let total = ui.available_width().min(1200.0);
    let list_w = 260.0;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(list_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(list_w);
            ui.spacing_mut().item_spacing.y = 2.0;
            for (i, l) in LESSONS.iter().enumerate() {
                let on = i == *cur;
                let galley_w = list_w - 16.0;
                let r = egui::Frame::new()
                    .fill(if on { p.panel } else { egui::Color32::TRANSPARENT })
                    .stroke(if on { Stroke::new(1.0, p.line) } else { Stroke::NONE })
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.set_width(galley_w - 4.0);
                        ui.horizontal_top(|ui| {
                            ui.label(RichText::new(format!("{}.", i + 1)).size(12.5).color(p.muted));
                            w::para(ui, RichText::new(l.title).size(12.5).color(if on { p.fg } else { p.fg2 }));
                        });
                    })
                    .response
                    .interact(Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if r.clicked() {
                    *cur = i;
                }
            }
        });
        ui.add_space(16.0);
        let body_w = (total - list_w - 16.0).max(320.0);
        ui.allocate_ui_with_layout(egui::vec2(body_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(body_w);
            let l = &LESSONS[(*cur).min(LESSONS.len() - 1)];
            w::card(ui, &p, l.title, "", |ui| {
                for para in l.body {
                    if let Some(h) = para.strip_prefix("§ ") {
                        ui.add_space(6.0);
                        ui.label(RichText::new(h).strong().size(14.0));
                    } else if let Some(f) = para.strip_prefix("$ ") {
                        egui::Frame::new()
                            .fill(p.panel3)
                            .corner_radius(8)
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    egui::Label::new(RichText::new(f).monospace().size(13.0).color(p.accent)).wrap(),
                                );
                            });
                    } else if let Some(b) = para.strip_prefix("- ") {
                        ui.horizontal_top(|ui| {
                            ui.label(RichText::new("•").color(p.accent));
                            w::para(ui, RichText::new(b).size(13.0).color(p.fg2));
                        });
                    } else {
                        w::para(ui, RichText::new(*para).size(13.0).color(p.fg2));
                    }
                    ui.add_space(2.0);
                }
                if let Some((label, view, block)) = l.try_it {
                    ui.add_space(8.0);
                    if w::button(ui, &p, &format!("→ {}", label), Btn::Primary).clicked() {
                        match block {
                            Some(b) => sh.go_block(view, Some(b)),
                            None => sh.go(view),
                        }
                    }
                }
            });
        });
    });
}
