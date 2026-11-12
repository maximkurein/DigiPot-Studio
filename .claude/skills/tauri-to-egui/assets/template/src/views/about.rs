use crate::app::Shared;
use eframe::egui::{RichText, Ui};

pub fn show(ui: &mut Ui, sh: &Shared) {
    ui.heading("О программе");
    ui.label(RichText::new(concat!("__TITLE__ · v", env!("CARGO_PKG_VERSION"))).color(sh.pal.muted));
}
