//! Example screen: read from `sh.st`, queue actions with `sh.rpc` / `sh.set_field`.
//! Replace `value` with a real field of the core's `State`.

use crate::app::{Act, Shared};
use eframe::egui::{self, RichText, Ui};

#[derive(Default)]
pub struct MainState {
    /// Slider value while dragging: the worker may not have applied it yet,
    /// without this the handle jumps back for a frame after release.
    draft: Option<u32>,
}

pub fn show(ui: &mut Ui, sh: &Shared, ms: &mut MainState) {
    let p = sh.pal;
    ui.heading("Главная");
    ui.add_space(8.0);
    egui::Frame::new().fill(p.panel).stroke(egui::Stroke::new(1.0, p.line)).corner_radius(12).inner_margin(14).show(
        ui,
        |ui| {
            let current = sh.st.value; // ← adapt
            let mut draft = ms.draft.unwrap_or(current);
            ui.label(RichText::new(draft.to_string()).size(30.0).strong());
            ui.spacing_mut().slider_width = (ui.available_width() - 10.0).max(200.0);
            let r = ui.add(egui::Slider::new(&mut draft, 0..=255).show_value(false));
            if r.dragged() {
                ms.draft = Some(draft);
            } else if !sh.busy || ms.draft == Some(current) {
                ms.draft = None;
            }
            if r.changed() && draft != current {
                sh.set_field("value", draft);
            }
            ui.horizontal(|ui| {
                if ui.button("Экспорт…").clicked() {
                    sh.act(Act::Export);
                }
                if ui.button("Импорт…").clicked() {
                    sh.act(Act::Import);
                }
            });
        },
    );
}
