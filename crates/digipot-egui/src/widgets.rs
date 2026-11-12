//! Reusable widgets in the style of the web UI (cards, stats, hints, badges,
//! segmented controls, engineering-unit fields) and the metadata-driven
//! register-field editor.

use crate::app::{Act, Shared, View};
use crate::fmt::{eng, parse_eng};
use crate::theme::{mix, Pal};
use digipot_core::regs::{self, Access, FieldDef, REGS};
use eframe::egui::{self, Color32, Response, RichText, Sense, Stroke, Ui, Widget};

pub fn card_frame(p: &Pal) -> egui::Frame {
    egui::Frame::new().fill(p.panel).stroke(Stroke::new(1.0, p.line)).corner_radius(12).inner_margin(14)
}

/// Card with an optional title and subtitle.
pub fn card<R>(ui: &mut Ui, p: &Pal, title: &str, subtitle: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    card_frame(p)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.label(RichText::new(title).strong().size(13.5));
            }
            if !subtitle.is_empty() {
                ui.add(egui::Label::new(RichText::new(subtitle).size(11.5).color(p.muted)).wrap());
            }
            if !title.is_empty() || !subtitle.is_empty() {
                ui.add_space(6.0);
            }
            body(ui)
        })
        .inner
}

/// Page title with an icon, a subtitle and right-aligned actions.
pub fn page_header(ui: &mut Ui, p: &Pal, icon: &str, title: &str, subtitle: &str, actions: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(38.0, 38.0), Sense::hover());
        ui.painter().rect(r, 10.0, p.panel, Stroke::new(1.0, p.line), egui::StrokeKind::Inside);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(18.0), p.accent);
        ui.vertical(|ui| {
            ui.set_max_width((ui.available_width() - 360.0).max(320.0));
            ui.label(RichText::new(title).size(20.0).strong());
            ui.add(egui::Label::new(RichText::new(subtitle).size(12.5).color(p.muted)).wrap());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min).with_main_wrap(true), actions);
    });
    ui.add_space(14.0);
}

/// Main column plus a fixed-width right column; stacks on narrow windows.
pub fn two_cols<S>(
    ui: &mut Ui,
    right_w: f32,
    s: &mut S,
    left: impl FnOnce(&mut Ui, &mut S),
    right: impl FnOnce(&mut Ui, &mut S),
) {
    let w = ui.available_width();
    if w < right_w + 520.0 {
        left(ui, s);
        ui.add_space(12.0);
        right(ui, s);
        return;
    }
    let gap = 16.0;
    let lw = w - right_w - gap;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.allocate_ui_with_layout(egui::vec2(lw, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(lw);
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 12.0);
            left(ui, s)
        });
        ui.add_space(gap);
        ui.allocate_ui_with_layout(egui::vec2(right_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(right_w);
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 12.0);
            right(ui, s)
        });
    });
}

/// Auto-fill grid: as many columns of at least `min_w` as fit.
pub fn auto_grid(ui: &mut Ui, min_w: f32, n: usize, mut cell: impl FnMut(&mut Ui, usize)) {
    if n == 0 {
        return;
    }
    let gap = 8.0;
    let cols = (((ui.available_width() + gap) / (min_w + gap)).floor() as usize).clamp(1, n);
    let cw = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
    for row in 0..n.div_ceil(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for c in 0..cols {
                let i = row * cols + c;
                if i >= n {
                    break;
                }
                ui.allocate_ui_with_layout(egui::vec2(cw, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(cw);
                    cell(ui, i)
                });
            }
        });
    }
}

/// Number tile: label, big value, optional sub-line.
pub fn stat(ui: &mut Ui, p: &Pal, label: &str, value: &str, tone: Option<Color32>, sub: &str) -> Response {
    egui::Frame::new()
        .fill(p.panel2)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add(egui::Label::new(RichText::new(label).size(11.0).color(p.muted)).truncate());
            ui.add(egui::Label::new(RichText::new(value).size(16.0).strong().color(tone.unwrap_or(p.fg))).truncate());
            if !sub.is_empty() {
                ui.add(egui::Label::new(RichText::new(sub).size(11.0).color(p.muted)).truncate());
            }
        })
        .response
}

/// Tinted note box.
pub fn hint(ui: &mut Ui, p: &Pal, tone: Color32, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(mix(tone, 8.0, p.panel))
        .stroke(Stroke::new(1.0, mix(tone, 25.0, p.panel)))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(11, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui)
        });
}

pub fn hint_text(ui: &mut Ui, p: &Pal, tone: Color32, text: &str) {
    hint(ui, p, tone, |ui| {
        ui.add(egui::Label::new(RichText::new(text).size(12.0).color(p.fg2)).wrap());
    });
}

/// Small rounded tag.
pub fn badge(ui: &mut Ui, p: &Pal, text: &str, tone: Color32) -> Response {
    egui::Frame::new()
        .fill(mix(tone, 14.0, p.panel))
        .stroke(Stroke::new(1.0, mix(tone, 35.0, p.panel)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(if tone == p.muted { p.fg2 } else { tone }));
        })
        .response
}

/// Clickable rounded pill with a dot.
pub fn pill(ui: &mut Ui, p: &Pal, text: &str, tone: Color32, dot_only: bool) -> Response {
    let fill = if dot_only { p.panel } else { mix(tone, 12.0, p.panel) };
    let r = egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, if dot_only { p.line } else { mix(tone, 35.0, p.panel) }))
        .corner_radius(14)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                led(ui, tone);
                ui.label(RichText::new(text).size(12.0).color(if dot_only { p.fg } else { tone }));
            });
        })
        .response
        .interact(Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    r
}

pub fn led(ui: &mut Ui, c: Color32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 4.5, c);
    ui.painter().circle_stroke(r.center(), 6.0, Stroke::new(2.0, c.gamma_multiply(0.25)));
}

/// "label ……… value" line.
pub fn kv(ui: &mut Ui, p: &Pal, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(RichText::new(k).size(11.5).color(p.muted)).truncate());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(v).size(11.5).strong());
        });
    });
}

/// Form row: fixed-width label (+hint) on the left, controls on the right.
pub fn row(ui: &mut Ui, p: &Pal, label: &str, hint: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(150.0, 28.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(150.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new(label).size(12.5).color(p.fg2));
            if !hint.is_empty() {
                ui.label(RichText::new(hint).size(10.5).color(p.muted));
            }
        });
        ui.horizontal_wrapped(add);
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    Default,
    Primary,
    Subtle,
    Ghost,
    Danger,
}

pub fn btn(p: &Pal, text: &str, kind: Btn) -> egui::Button<'static> {
    let (fill, fg, stroke) = match kind {
        Btn::Primary => (p.accent, p.accent_fg, p.accent),
        Btn::Danger => (p.err, Color32::WHITE, p.err),
        Btn::Subtle => (p.panel3, p.fg2, p.line),
        Btn::Ghost => (Color32::TRANSPARENT, p.fg2, Color32::TRANSPARENT),
        Btn::Default => (p.panel2, p.fg, p.line),
    };
    egui::Button::new(RichText::new(text.to_string()).color(fg).size(12.5))
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(7)
        .min_size(egui::vec2(0.0, 26.0))
}

pub fn button(ui: &mut Ui, p: &Pal, text: &str, kind: Btn) -> Response {
    ui.add(btn(p, text, kind))
}

pub fn icon_button(ui: &mut Ui, p: &Pal, icon: &str, active: bool) -> Response {
    let b = egui::Button::new(RichText::new(icon).size(14.0).color(if active { p.accent } else { p.fg2 }))
        .fill(if active { p.panel3 } else { Color32::TRANSPARENT })
        .stroke(Stroke::NONE)
        .corner_radius(7)
        .min_size(egui::vec2(28.0, 26.0));
    ui.add(b)
}

/// Sidebar entry.
pub fn nav_item(ui: &mut Ui, p: &Pal, icon: &str, label: &str, hint: &str, n: usize, on: bool) -> Response {
    let w = ui.available_width();
    let (rect, r) = ui.allocate_exact_size(egui::vec2(w, 38.0), Sense::click());
    let hovered = r.hovered();
    if on {
        ui.painter().rect(rect, 8.0, p.panel, Stroke::new(1.0, p.line), egui::StrokeKind::Inside);
    } else if hovered {
        ui.painter().rect_filled(rect, 8.0, p.panel3);
    }
    let pt = ui.painter();
    pt.text(
        rect.left_center() + egui::vec2(16.0, 0.0),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(15.0),
        if on { p.accent } else { p.muted },
    );
    pt.text(
        rect.left_top() + egui::vec2(32.0, 5.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(12.5),
        if on { p.fg } else { p.fg2 },
    );
    pt.text(
        rect.left_top() + egui::vec2(32.0, 21.0),
        egui::Align2::LEFT_TOP,
        hint,
        egui::FontId::proportional(10.5),
        p.muted,
    );
    if hovered {
        pt.text(
            rect.right_center() - egui::vec2(8.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            n.to_string(),
            egui::FontId::proportional(10.0),
            p.muted,
        );
    }
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// iOS-style switch. Sets `changed()` when toggled.
pub fn toggle(ui: &mut Ui, p: &Pal, on: &mut bool) -> Response {
    let size = egui::vec2(34.0, 19.0);
    let (rect, mut r) = ui.allocate_exact_size(size, Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(r.id, *on);
    let bg = mix(p.accent, t * 100.0, p.line2);
    let rad = rect.height() / 2.0;
    ui.painter().rect_filled(rect, rad, bg);
    let cx = egui::lerp((rect.left() + rad)..=(rect.right() - rad), t);
    ui.painter().circle_filled(egui::pos2(cx, rect.center().y), rad - 2.5, Color32::WHITE);
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Segmented control; returns the newly picked value.
pub fn segmented<T: PartialEq + Copy, S: AsRef<str>>(ui: &mut Ui, p: &Pal, value: T, options: &[(T, S)]) -> Option<T> {
    let mut out = None;
    egui::Frame::new().fill(p.panel3).corner_radius(8).inner_margin(2).show(ui, |ui| {
        // explicit LTR: inside a right-to-left parent the options would be reversed
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (v, label) in options {
                let on = *v == value;
                let b =
                    egui::Button::new(RichText::new(label.as_ref()).size(12.0).color(if on { p.fg } else { p.muted }))
                        .fill(if on { p.panel } else { Color32::TRANSPARENT })
                        .stroke(if on { Stroke::new(1.0, p.line) } else { Stroke::NONE })
                        .corner_radius(6)
                        .min_size(egui::vec2(0.0, 22.0));
                if ui.add(b).clicked() && !on {
                    out = Some(*v);
                }
            }
        });
    });
    out
}

/// Text field with SI prefixes ("10k", "2.5 мВ"). Shows the formatted value
/// while not focused; commits on Enter / focus loss. Returns the new value.
#[allow(clippy::too_many_arguments)]
pub fn eng_field(
    ui: &mut Ui,
    p: &Pal,
    id: impl std::hash::Hash,
    value: f64,
    unit: &str,
    min: f64,
    max: f64,
    width: f32,
) -> Option<f64> {
    let id = ui.id().with(("eng", id));
    let shown = eng(value, unit, 6);
    let mut buf: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.clone());
    let bad: bool = ui.data_mut(|d| d.get_temp::<bool>(id.with("bad"))).unwrap_or(false);
    let r = ui.add(egui::TextEdit::singleline(&mut buf).id(id.with("te")).desired_width(width).text_color(if bad {
        p.err
    } else {
        p.fg
    }));
    let mut out = None;
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    }
    if r.lost_focus() {
        match parse_eng(&buf) {
            Some(v) if v >= min && v <= max => {
                if v != value {
                    out = Some(v);
                }
                ui.data_mut(|d| d.insert_temp(id.with("bad"), false));
            }
            _ => ui.data_mut(|d| d.insert_temp(id.with("bad"), true)),
        }
        ui.data_mut(|d| d.remove::<String>(id));
    } else if !r.has_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    if bad {
        r.on_hover_text(format!("Ожидается число от {} до {}", eng(min, unit, 4), eng(max, unit, 4)));
    }
    out
}

/// Integer field (decimal or hex) that commits on change.
pub fn num_field(ui: &mut Ui, value: i64, min: i64, max: i64, hex: bool, enabled: bool) -> Option<i64> {
    let mut v = value;
    let mut d = egui::DragValue::new(&mut v).range(min..=max).speed(0.2).update_while_editing(false);
    if hex {
        d = d.hexadecimal(3, false, true).prefix("0x");
    }
    let r = ui.add_enabled(enabled, d);
    (r.changed() && v != value).then_some(v)
}

/// Float field that commits on change.
pub fn float_field(ui: &mut Ui, value: f64, min: f64, max: f64, step: f64, suffix: &str) -> Option<f64> {
    let mut v = value;
    let r =
        ui.add(egui::DragValue::new(&mut v).range(min..=max).speed(step).suffix(suffix).update_while_editing(false));
    (r.changed() && v != value).then_some(v)
}

/// Combo box over (value, label) options.
pub fn select<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    value: T,
    options: &[(T, String)],
    width: f32,
) -> Option<T> {
    let mut out = None;
    let cur = options.iter().find(|o| o.0 == value).map(|o| o.1.clone()).unwrap_or_default();
    egui::ComboBox::from_id_salt(id).selected_text(cur).width(width).show_ui(ui, |ui| {
        for (v, label) in options {
            if ui.selectable_label(*v == value, label).clicked() && *v != value {
                out = Some(*v);
            }
        }
    });
    out
}

/// `WIPER0[8:0]`.
pub fn bits_label(f: &FieldDef) -> String {
    let reg = REGS.get(f.reg).map(|r| r.name).unwrap_or("R?");
    if f.msb == f.lsb {
        format!("{}[{}]", reg, f.msb)
    } else {
        format!("{}[{}:{}]", reg, f.msb, f.lsb)
    }
}

fn signed(v: u32, bits: u32) -> i64 {
    if bits < 64 && v as i64 >= 1i64 << (bits - 1) {
        v as i64 - (1i64 << bits)
    } else {
        v as i64
    }
}

/// Auto-generated editor for any register field (from the Rust metadata).
pub fn field_control(ui: &mut Ui, sh: &Shared, f: &'static FieldDef, compact: bool, highlight: bool) {
    let p = &sh.pal;
    let width = f.width();
    let raw = sh.field(f.name);
    let ro = f.access != Access::Rw;
    let nv = regs::nv(f.reg);
    let v = if f.signed { signed(raw, width) } else { raw as i64 };
    let opt_label = f.options.iter().find(|o| o.0 == raw).map(|o| o.1);
    let unit = if f.unit.is_empty() { String::new() } else { format!(" {}", f.unit) };

    let frame = egui::Frame::new()
        .fill(p.panel2)
        .stroke(Stroke::new(if highlight { 2.0 } else { 1.0 }, if highlight { p.accent } else { p.line }))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(11, if compact { 5 } else { 8 }));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.set_max_width((ui.available_width() - 150.0).max(150.0));
                ui.add(egui::Label::new(RichText::new(f.title).size(12.5).strong()).wrap());
                let link = ui
                    .add(
                        egui::Label::new(
                            RichText::new(format!("{} · {}", f.name, bits_label(f)))
                                .monospace()
                                .size(10.5)
                                .color(p.muted),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_text("Открыть в карте регистров")
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if link.clicked() {
                    sh.act(Act::Focus(Some(f.name)));
                    sh.go(View::Registers);
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if nv && !ro {
                    // EEPROM: written only from the Memory view, with confirmation
                    if badge(ui, p, &format!("{}{} · EEPROM →", v, unit), p.refc)
                        .interact(Sense::click())
                        .on_hover_text("Запись в EEPROM — на экране «EEPROM»")
                        .clicked()
                    {
                        sh.go(View::Memory);
                    }
                } else if ro {
                    let what = match f.access {
                        Access::Ro => "только чтение",
                        Access::Sticky => "sticky",
                        _ => "фиксировано",
                    };
                    let shown = opt_label.map(str::to_string).unwrap_or_else(|| v.to_string());
                    let tone = if f.access == Access::Sticky { p.warn } else { p.muted };
                    badge(ui, p, &format!("{}{} · {}", shown, unit, what), tone);
                } else if !f.options.is_empty()
                    && f.options.len() <= 4
                    && f.options.iter().all(|o| o.1.chars().count() < 26)
                {
                    if let Some(x) = segmented(ui, p, raw, f.options) {
                        sh.set_field(f.name, x);
                    }
                } else if !f.options.is_empty() {
                    let opts: Vec<(u32, String)> =
                        f.options.iter().map(|(v, l)| (*v, format!("{}  ({})", l, v))).collect();
                    if let Some(x) = select(ui, f.name, raw, &opts, 220.0) {
                        sh.set_field(f.name, x);
                    }
                } else if width == 1 {
                    let mut on = raw == 1;
                    if toggle(ui, p, &mut on).changed() {
                        sh.set_field(f.name, on as u32);
                    }
                } else if f.signed {
                    let lo = -(1i64 << (width - 1));
                    let hi = (1i64 << (width - 1)) - 1;
                    if let Some(x) = num_field(ui, v, lo, hi, false, true) {
                        let w = if x < 0 { x + (1i64 << width) } else { x };
                        sh.set_field(f.name, w as u32);
                    }
                } else if let Some(x) = num_field(ui, raw as i64, 0, f.max() as i64, false, true) {
                    sh.set_field(f.name, x as u32);
                }
            });
        });
        if !compact && !f.desc.is_empty() {
            ui.add(egui::Label::new(RichText::new(f.desc).size(11.5).color(p.muted)).wrap());
        }
    });
}

/// Paragraph label that wraps.
pub fn para(ui: &mut Ui, text: impl Into<RichText>) {
    egui::Label::new(text.into()).wrap().ui(ui);
}
