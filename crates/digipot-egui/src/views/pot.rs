//! Potentiometer: wiper position, schematic with TCON switches, transfer curves.

use crate::app::Shared;
use crate::canvas::Canvas;
use crate::fmt::{eng, eng_opt};
use crate::theme::{mix, Pal};
use crate::widgets::{self as w, Btn};
use digipot_core::model::{self, RAB_OPTIONS};
use eframe::egui::{self, Align2, RichText, Stroke, Ui};
use egui_plot::{Legend, Line, LineStyle, Plot, PlotPoints, Points, VLine};
use serde_json::json;

const FS: u32 = 256;

#[derive(Default)]
pub struct PotState {
    /// Slider value while dragging (follows the chip otherwise).
    draft: Option<u32>,
}

pub fn show(ui: &mut Ui, sh: &Shared, ps: &mut PotState) {
    let p = sh.pal;
    w::page_header(
        ui,
        &p,
        "📈",
        "Потенциометр",
        "Положение движка, схема включения и передаточная характеристика. Значения считает модель по даташиту; в симуляторе рядом — «мультиметр» на конкретном экземпляре.",
        |_| {},
    );
    w::two_cols(ui, 400.0, ps, |ui, ps| left(ui, sh, ps), |ui, _| right(ui, sh));
}

fn left(ui: &mut Ui, sh: &Shared, ps: &mut PotState) {
    let p = sh.pal;
    let code = sh.field("WIPER0");
    let eff = code.min(FS);
    let k_ohm = |v: Option<f64>| eng(v.unwrap_or(0.0), "Ω", 3);

    w::card(ui, &p, "Движок", "WIPER0 · 257 положений (0…256)", |ui| {
        let mut draft = ps.draft.unwrap_or(eff);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("Код N").size(11.0).color(p.muted));
                ui.label(RichText::new(draft.to_string()).size(34.0).strong());
                ui.label(
                    RichText::new(format!("{:03X}h · {:.1} %", code, eff as f64 / FS as f64 * 100.0))
                        .size(11.0)
                        .color(p.muted),
                );
            });
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.spacing_mut().slider_width = (ui.available_width() - 10.0).max(200.0);
                let r = ui.add(egui::Slider::new(&mut draft, 0..=FS).show_value(false));
                // keep the dragged value until the engine thread has caught up,
                // otherwise the handle jumps back for a frame after release
                if r.dragged() {
                    ps.draft = Some(draft);
                } else if !sh.busy || ps.draft == Some(eff) {
                    ps.draft = None;
                }
                if r.changed() && draft != eff {
                    sh.set_field("WIPER0", draft);
                }
                ui.horizontal(|ui| {
                    let t = |s: &str| RichText::new(s).size(10.5).color(p.muted);
                    ui.label(t("0 · W = B"));
                    let wd = ui.available_width();
                    ui.add_space(wd / 2.0 - 30.0);
                    ui.label(t("128"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(t("256 · W = A")));
                });
            });
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for (label, up, n, tip) in [
                ("⏬ −10", false, 10, "10 команд декремента в одном кадре"),
                ("− −1", false, 1, "Команда декремента (1 байт)"),
                ("+ +1", true, 1, "Команда инкремента (1 байт)"),
                ("⏫ +10", true, 10, "10 команд инкремента в одном кадре"),
            ] {
                if w::button(ui, &p, label, Btn::Default).on_hover_text(tip).clicked() {
                    sh.rpc("step", json!({ "up": up, "n": n }));
                }
            }
            ui.add_space(10.0);
            for v in [0u32, 64, 128, 192, 256] {
                let kind = if eff == v { Btn::Primary } else { Btn::Ghost };
                if w::button(ui, &p, &v.to_string(), kind).clicked() {
                    sh.set_field("WIPER0", v);
                }
            }
            ui.add_space(10.0);
            if let Some(v) = w::num_field(ui, code as i64, 0, 0x1FF, false, true) {
                sh.set_field("WIPER0", v as u32);
            }
        });
        ui.add_space(8.0);
        let stats = [
            ("R_WB", k_ohm(sh.metric("rwb")), Some(p.sig)),
            ("R_WA", k_ohm(sh.metric("rwa")), Some(p.clk)),
            ("V_W", eng_opt(sh.metric("vw"), "V", 4), Some(p.accent)),
            ("Шаг R_S", k_ohm(sh.metric("rs")), None),
            ("Шаг по напряжению", eng_opt(sh.metric("lsb"), "V", 3), None),
            ("Ток A→B", eng_opt(sh.metric("iab"), "A", 3), None),
        ];
        w::auto_grid(ui, 140.0, stats.len(), |ui, i| {
            let (l, v, t) = &stats[i];
            w::stat(ui, &p, l, v, *t, "");
        });
    });

    let curve = model::curve(&sh.st.regs, &sh.st.settings);
    let meter = if sh.is_sim() { sh.st.meter.clone() } else { None };
    let pts = |ys: &[f64]| -> PlotPoints<'static> {
        curve.code.iter().zip(ys).filter(|(_, y)| y.is_finite()).map(|(x, y)| [*x, *y]).collect()
    };

    w::card(
        ui,
        &p,
        "Сопротивление",
        "R_WB и R_WA от кода (номинал R_AB, R_W, температура)",
        |ui| {
            base_plot("plot_r", &p, "сопротивление", "Ω").show(ui, |pu| {
                pu.line(Line::new("R_WB", pts(&curve.rwb)).color(p.sig).width(2.0));
                pu.line(Line::new("R_WA", pts(&curve.rwa)).color(p.clk).width(2.0));
                pu.vline(VLine::new("", eff as f64).color(p.accent).style(LineStyle::Dashed { length: 6.0 }));
                if let Some(m) = &meter {
                    pu.points(
                        Points::new("мультиметр", vec![[eff as f64, m.rwb]]).color(p.warn).radius(5.0).filled(true),
                    );
                }
            });
        },
    );

    w::card(
        ui,
        &p,
        "Напряжение на W",
        "Идеальный делитель и с нагрузкой R_L (выход W имеет внутреннее сопротивление)",
        |ui| {
            base_plot("plot_v", &p, "V_W", "V").show(ui, |pu| {
                pu.line(
                    Line::new("идеально", pts(&curve.vw_ideal)).color(p.muted).style(LineStyle::Dashed { length: 6.0 }),
                );
                pu.line(Line::new("+1 LSB", pts(&curve.inl_hi)).color(p.line2).width(1.0));
                pu.line(Line::new("−1 LSB", pts(&curve.inl_lo)).color(p.line2).width(1.0));
                pu.line(Line::new("V_W", pts(&curve.vw)).color(p.accent).width(2.0));
                pu.vline(VLine::new("", eff as f64).color(p.accent).style(LineStyle::Dashed { length: 6.0 }));
                if let Some(m) = meter.as_ref().filter(|m| m.vw.is_finite()) {
                    pu.points(
                        Points::new("мультиметр", vec![[eff as f64, m.vw]]).color(p.warn).radius(5.0).filled(true),
                    );
                }
            });
        },
    );
}

fn base_plot(id: &'static str, p: &Pal, y_label: &'static str, unit: &'static str) -> Plot<'static> {
    let _ = p;
    Plot::new(id)
        .height(240.0)
        .legend(Legend::default().position(egui_plot::Corner::LeftTop))
        .include_x(0.0)
        .include_x(FS as f64)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .x_axis_label("код N")
        .y_axis_label(y_label)
        .y_axis_min_width(56.0)
        .y_axis_formatter(move |m, _| eng(m.value, unit, 3))
        .label_formatter(move |name, pt| {
            let head = if name.is_empty() { String::new() } else { format!("{}\n", name) };
            format!("{}N = {:.0}\n{}", head, pt.x, eng(pt.y, unit, 4))
        })
}

fn right(ui: &mut Ui, sh: &Shared) {
    let p = sh.pal;
    let s = &sh.st.settings;
    w::card(ui, &p, "", "", |ui| schematic(ui, sh));

    w::card(
        ui,
        &p,
        "Схема включения",
        "Параметры платы, которых нет в регистрах",
        |ui| {
            // four options do not fit next to the label: put them on their own line
            ui.label(RichText::new("Вариант R_AB").size(12.5).color(p.fg2));
            ui.horizontal(|ui| {
                let opts: Vec<(u32, String)> =
                    RAB_OPTIONS.iter().map(|(_, ohm)| (*ohm as u32, eng(*ohm, "Ω", 3))).collect();
                if let Some(v) = w::segmented(ui, &p, s.rab_nominal as u32, &opts) {
                    sh.set_settings(json!({ "rab_nominal": v as f64 }));
                }
                ui.label(RichText::new("суффикс в маркировке U1").size(10.5).color(p.muted));
            });
            w::row(ui, &p, "VDD кристалла", "", |ui| {
                let cur = if (s.vdd - 5.0).abs() < 1e-9 {
                    50
                } else if (s.vdd - 3.3).abs() < 1e-9 {
                    33
                } else {
                    0
                };
                if let Some(v) = w::segmented(ui, &p, cur, &[(33, "3.3 В"), (50, "5 В")]) {
                    sh.set_settings(json!({ "vdd": v as f64 / 10.0 }));
                }
            });
            let eng_row =
                |ui: &mut Ui, label: &str, hint: &str, key: &'static str, v: f64, unit: &str, lo: f64, hi: f64| {
                    w::row(ui, &p, label, hint, |ui| {
                        if let Some(x) = w::eng_field(ui, &p, key, v, unit, lo, hi, 130.0) {
                            sh.set_settings(json!({ key: x }));
                        }
                    });
                };
            eng_row(ui, "Напряжение на A", "", "va", s.va, "V", -1.0, 7.0);
            eng_row(ui, "Напряжение на B", "", "vb", s.vb, "V", -1.0, 7.0);
            eng_row(ui, "Нагрузка W → GND", "0 = без нагрузки", "load_ohm", s.load_ohm, "Ω", 0.0, 1e9);
            eng_row(ui, "R_W (движок)", "75 Ом типично", "rw_ohm", s.rw_ohm, "Ω", 0.0, 1000.0);
            eng_row(ui, "Температура", "", "temp_c", s.temp_c, "°C", -40.0, 125.0);
        },
    );

    w::card(
        ui,
        &p,
        "Задать по значению",
        "Код подбирается моделью и округляется до ближайшего шага",
        |ui| {
            w::row(ui, &p, "R_WB (реостат)", "", |ui| {
                if let Some(v) = w::eng_field(ui, &p, "goto_rwb", sh.metric("rwb").unwrap_or(0.0), "Ω", 0.0, 1e6, 130.0)
                {
                    sh.rpc("goto", json!({ "target": "rwb", "value": v }));
                }
            });
            w::row(ui, &p, "V_W (делитель)", "", |ui| {
                if let Some(v) = w::eng_field(ui, &p, "goto_vw", sh.metric("vw").unwrap_or(0.0), "V", -1.0, 7.0, 130.0)
                {
                    sh.rpc("goto", json!({ "target": "vw", "value": v }));
                }
            });
        },
    );

    match (&sh.st.meter, sh.is_sim()) {
        (Some(m), true) => {
            w::card(ui, &p, "Мультиметр (симулятор)", "Этот экземпляр отличается от номинала — как настоящий кристалл", |ui| {
                let k = |v: f64| eng(v, "Ω", 3);
                let tiles = [
                    ("R_AB экземпляра", k(m.rab), format!("номинал {}", k(s.rab_nominal))),
                    ("R_WB измерено", k(m.rwb), format!("модель {}", k(sh.metric("rwb").unwrap_or(0.0)))),
                    ("V_W измерено", eng(m.vw, "V", 4), format!("модель {}", eng_opt(sh.metric("vw"), "V", 4))),
                    ("R_WA измерено", k(m.rwa), String::new()),
                ];
                w::auto_grid(ui, 150.0, 4, |ui, i| {
                    let (l, v, sub) = &tiles[i];
                    w::stat(ui, &p, l, v, Some(p.warn), sub);
                });
                ui.add_space(4.0);
                w::row(ui, &p, "Отклонение R_AB", "допуск по даташиту ±20 %", |ui| {
                    if let Some(v) = w::float_field(ui, s.sim_rab_dev_pct, -20.0, 20.0, 0.1, " %") {
                        sh.set_settings(json!({ "sim_rab_dev_pct": v }));
                    }
                });
                w::hint_text(ui, &p, p.accent, "В режиме делителя V_W совпадает с моделью при любом отклонении R_AB, а R_WB — нет. Поэтому реостат калибруют, а делитель — нет.");
            });
        }
        _ => w::hint_text(
            ui,
            &p,
            p.accent,
            "Подключена реальная плата: измерьте R_WB или V_W мультиметром и сравните с моделью — так находится R_AB вашего экземпляра.",
        ),
    }
}

/// Schematic of the pot with clickable TCON switches (440×380 view box).
fn schematic(ui: &mut Ui, sh: &Shared) {
    let p = sh.pal;
    let s = &sh.st.settings;
    let code = sh.field("WIPER0").min(FS);
    let shdn = sh.field("R0HW") == 0;
    let a = sh.field("R0A") == 1 && !shdn;
    let b = sh.field("R0B") == 1;
    let wc = sh.field("R0W") == 1;
    let vw = sh.metric("vw");
    let (y_top, y_bot) = (80.0f32, 300.0f32);
    let y_w = y_bot - (code as f32 / FS as f32) * (y_bot - y_top);
    let x = 150.0f32;
    let (c, _) = Canvas::new(ui, [0.0, 0.0, 440.0, 380.0], 420.0);
    let wire = |pts: &[(f32, f32)]| c.line(pts, p.fg2, 2.0);
    let term = |cx: f32, cy: f32, col| c.circle(cx, cy, 9.0, p.panel, Stroke::new(2.0, col));

    let switch = |ui: &mut Ui, x1: f32, y1: f32, x2: f32, y2: f32, closed: bool, field: &'static str, label: &str| {
        let len = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        let rot = (y2 - y1).atan2(x2 - x1) + if closed { 0.0 } else { (-28.0f32).to_radians() };
        c.circle(x1, y1, 3.5, p.fg2, Stroke::NONE);
        c.circle(x2, y2, 3.5, p.fg2, Stroke::NONE);
        c.line(&[(x1, y1), (x1 + len * rot.cos(), y1 + len * rot.sin())], if closed { p.ok } else { p.warn }, 3.0);
        if !label.is_empty() {
            c.text(x1.max(x2) + 12.0, (y1 + y2) / 2.0, Align2::LEFT_CENTER, label, 11.0, p.muted);
        }
        let r = c.hit(ui, field, x1.min(x2) - 14.0, y1.min(y2) - 14.0, (x2 - x1).abs() + 28.0, (y2 - y1).abs() + 28.0);
        if r.on_hover_text(format!("{} — нажмите, чтобы переключить", field)).clicked() {
            sh.set_field(field, if sh.field(field) == 1 { 0 } else { 1 });
        }
    };

    // terminal A
    term(x, 22.0, p.sig);
    c.text_bold(x - 24.0, 22.0, Align2::RIGHT_CENTER, "A", 14.0, p.fg);
    c.text(x + 18.0, 22.0, Align2::LEFT_CENTER, eng(s.va, "V", 3), 12.0, p.sig);
    wire(&[(x, 31.0), (x, 44.0)]);
    switch(ui, x, 44.0, x, 70.0, a, "R0A", "R0A");
    wire(&[(x, 70.0), (x, y_top)]);
    // ladder
    c.rect(x - 13.0, y_top, 26.0, y_bot - y_top, 4.0, mix(p.sig, 10.0, p.panel), Stroke::new(1.6, p.sig));
    for i in 0..16 {
        let y = y_top + (i as f32 + 1.0) * (y_bot - y_top) / 17.0;
        c.line(&[(x - 13.0, y), (x + 13.0, y)], p.sig.gamma_multiply(0.25), 1.0);
    }
    c.text(
        x - 24.0,
        (y_top + y_w) / 2.0,
        Align2::RIGHT_CENTER,
        format!("R_WA {}", eng_opt(sh.metric("rwa"), "Ω", 3)),
        11.0,
        p.muted,
    );
    c.text(
        x - 24.0,
        (y_w + y_bot) / 2.0,
        Align2::RIGHT_CENTER,
        format!("R_WB {}", eng_opt(sh.metric("rwb"), "Ω", 3)),
        11.0,
        p.muted,
    );
    // wiper arrow
    c.painter.add(egui::Shape::convex_polygon(
        vec![c.p(x + 16.0, y_w), c.p(x + 26.0, y_w - 6.0), c.p(x + 26.0, y_w + 6.0)],
        p.accent,
        Stroke::NONE,
    ));
    c.line(&[(x + 26.0, y_w), (250.0, y_w)], p.accent, 2.0);
    if shdn {
        // W forced to B: dashed bypass
        let pts: Vec<(f32, f32)> = (0..=16)
            .map(|i| {
                let t = i as f32 / 16.0;
                let (p0, p1, p2, p3) = ((x + 20.0, y_w), (x + 60.0, y_w), (x + 60.0, y_bot + 22.0), (x, y_bot + 22.0));
                let u = 1.0 - t;
                (
                    u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0,
                    u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1,
                )
            })
            .collect();
        c.dashed(&pts, p.warn, 1.6, 5.0, 4.0, 0.0);
    }
    switch(ui, 250.0, y_w, 290.0, y_w, wc, "R0W", "");
    c.text(270.0, y_w - 14.0, Align2::CENTER_CENTER, "R0W", 11.0, p.muted);
    wire(&[(290.0, y_w), (322.0, y_w)]);
    term(331.0, y_w, p.accent);
    c.text_bold(331.0, y_w - 20.0, Align2::CENTER_CENTER, "W", 14.0, p.fg);
    let vw_txt = vw.filter(|v| v.is_finite()).map(|v| eng(v, "V", 4)).unwrap_or_else(|| "—".into());
    c.text(346.0, y_w, Align2::LEFT_CENTER, vw_txt, 12.0, p.accent);
    if s.load_ohm > 0.0 {
        wire(&[(331.0, y_w + 9.0), (331.0, y_w + 30.0)]);
        c.rect(323.0, y_w + 30.0, 16.0, 34.0, 3.0, p.panel, Stroke::new(1.0, p.fg2));
        c.text(346.0, y_w + 48.0, Align2::LEFT_CENTER, format!("R_L {}", eng(s.load_ohm, "Ω", 3)), 11.0, p.muted);
        wire(&[(331.0, y_w + 64.0), (331.0, y_w + 76.0)]);
        for (dx, dy) in [(12.0, 76.0), (8.0, 81.0), (4.0, 86.0)] {
            c.line(&[(331.0 - dx, y_w + dy), (331.0 + dx, y_w + dy)], p.fg2, 1.6);
        }
    }
    // terminal B
    wire(&[(x, y_bot), (x, y_bot + 10.0)]);
    switch(ui, x, y_bot + 10.0, x, y_bot + 36.0, b, "R0B", "R0B");
    wire(&[(x, y_bot + 36.0), (x, y_bot + 44.0)]);
    term(x, y_bot + 53.0, p.sig);
    c.text_bold(x - 24.0, y_bot + 53.0, Align2::RIGHT_CENTER, "B", 14.0, p.fg);
    c.text(x + 18.0, y_bot + 53.0, Align2::LEFT_CENTER, eng(s.vb, "V", 3), 12.0, p.sig);
    c.text(8.0, 372.0, Align2::LEFT_CENTER, "Ключи кликабельны (TCON)", 11.0, p.muted);
    if shdn {
        c.text(250.0, y_bot + 53.0, Align2::LEFT_CENTER, "Отключение: W → B", 11.5, p.warn);
    }
}
