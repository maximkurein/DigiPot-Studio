//! Connection: simulator or a real board through the USB SPI bridge.

use crate::app::{Act, HwState, Shared};
use crate::content::{SAFETY, WIRING};
use crate::theme::mix;
use crate::widgets::{self as w, Btn};
use digipot_core::bus;
use eframe::egui::{self, RichText, Stroke, Ui};
use serde_json::json;

const BAUDS: [u32; 6] = [9600, 57600, 115_200, 230_400, 460_800, 921_600];

pub fn show(ui: &mut Ui, sh: &Shared, hw: &mut HwState) {
    let p = sh.pal;
    if !hw.loaded {
        hw.loaded = true;
        sh.act(Act::RefreshPorts);
    }
    w::page_header(
        ui,
        &p,
        "🔌",
        "Подключение",
        &format!(
            "Симулятор или реальная плата через USB-мост ({}). Протокол моста и прошивка — firmware/bridge.",
            bus::describe()
        ),
        |_| {},
    );
    let half = ((ui.available_width() - 16.0) / 2.0).max(420.0);
    w::two_cols(
        ui,
        if ui.available_width() > 1000.0 { half } else { 420.0 },
        hw,
        |ui, hw| {
            current(ui, sh);
            wiring(ui, sh);
            let _ = hw;
        },
        |ui, hw| {
            bridge(ui, sh, hw);
            usb_list(ui, sh, hw);
        },
    );
}

fn current(ui: &mut Ui, sh: &Shared) {
    let p = sh.pal;
    w::card(ui, &p, "🔌 Текущее подключение", "", |ui| {
        ui.horizontal(|ui| {
            w::led(ui, if sh.st.link_error.is_some() { p.err } else { p.ok });
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new(&sh.st.link.name).strong().size(13.0));
                let d = sh.st.link_error.clone().unwrap_or_else(|| sh.st.link.detail.clone());
                ui.add(egui::Label::new(RichText::new(d).size(11.5).color(p.muted)).truncate());
            });
            if !sh.is_sim() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if w::button(ui, &p, "На симулятор", Btn::Default).clicked() {
                        sh.rpc("connect", json!({ "kind": "sim" }));
                    }
                });
            }
        });
        ui.add_space(4.0);
        w::hint_text(
            ui,
            &p,
            p.accent,
            "Чтение по 3-проводной схеме. MISO на DIGI POT click не разведён: при чтении мост выдаёт 6 бит команды по MOSI, отпускает линию и принимает ответ кристалла по ней же (команда моста T, ≤ 100 кГц). Запись работает с любым мастером SPI.",
        );
    });
}

fn bridge(ui: &mut Ui, sh: &Shared, hw: &mut HwState) {
    let p = sh.pal;
    w::card(ui, &p, "🔗 USB-мост", "", |ui| {
        ui.horizontal_wrapped(|ui| {
            let opts: Vec<(usize, String)> = if hw.ports.is_empty() {
                vec![(0, "порты не найдены".into())]
            } else {
                hw.ports
                    .iter()
                    .enumerate()
                    .map(|(i, x)| {
                        (
                            i,
                            format!(
                                "{}{}",
                                x.path,
                                x.product.as_ref().map(|s| format!(" — {}", s)).unwrap_or_default()
                            ),
                        )
                    })
                    .collect()
            };
            let cur = hw.ports.iter().position(|x| x.path == hw.port).unwrap_or(0);
            if let Some(i) = w::select(ui, "port", cur, &opts, 260.0) {
                if let Some(x) = hw.ports.get(i) {
                    hw.port = x.path.clone();
                }
            }
            let bauds: Vec<(u32, String)> = BAUDS.iter().map(|b| (*b, format!("{} бод", b))).collect();
            if let Some(b) = w::select(ui, "baud", hw.baud, &bauds, 110.0) {
                hw.baud = b;
            }
            if ui.add_enabled(!hw.port.is_empty(), w::btn(&p, "Подключить", Btn::Primary)).clicked() {
                sh.rpc("connect", json!({ "kind": "serial", "path": hw.port, "baud": hw.baud }));
            }
            if w::icon_button(ui, &p, "⟳", false).on_hover_text("Обновить список").clicked() {
                sh.act(Act::RefreshPorts);
            }
        });
        ui.add_space(4.0);
        w::hint_text(
            ui,
            &p,
            p.accent,
            "Прошейте firmware/bridge/bridge.ino в плату с логикой 3.3 В (RP2040, STM32 Nucleo, ESP32, Arduino Zero/Due) — в ней есть команда T для чтения по одной линии. При подключении приложение спрашивает ? и ждёт ответ BRIDGE ….",
        );
    });
}

fn wiring(ui: &mut Ui, sh: &Shared) {
    let p = sh.pal;
    w::card(ui, &p, "🔧 Схема соединений", "", |ui| {
        egui::Grid::new("wiring").num_columns(5).spacing([14.0, 5.0]).striped(false).show(ui, |ui| {
            for h in ["Мост", "", "Плата", "Сигнал", "Примечание"] {
                ui.label(RichText::new(h).size(11.0).color(p.muted));
            }
            ui.end_row();
            for (mcu, dir, board, signal, note) in WIRING {
                ui.label(RichText::new(*mcu).monospace().size(12.0));
                ui.label(RichText::new(*dir).color(p.muted));
                ui.label(RichText::new(*board).monospace().size(12.0));
                ui.label(RichText::new(*signal).monospace().size(12.0).color(p.accent));
                ui.label(RichText::new(*note).size(11.5).color(p.muted));
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        for s in SAFETY {
            w::hint_text(ui, &p, p.warn, &format!("🛡 {}", s));
        }
    });
}

fn usb_list(ui: &mut Ui, sh: &Shared, hw: &HwState) {
    let p = sh.pal;
    w::card(
        ui,
        &p,
        "USB-устройства",
        "Подсказки по производителю помогают найти интерфейс платы или мост",
        |ui| match &hw.usb {
            None => {
                ui.label(RichText::new("Сканирование…").size(12.0).color(p.muted));
            }
            Some(list) if list.is_empty() => {
                ui.label(RichText::new("Устройства не найдены (или нет доступа к USB).").size(12.0).color(p.muted));
            }
            Some(list) => {
                egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                    for d in list {
                        let (fill, stroke) = if d.likely_bridge {
                            (mix(p.accent, 6.0, p.panel), mix(p.accent, 40.0, p.panel))
                        } else {
                            (p.panel2, p.line)
                        };
                        egui::Frame::new()
                            .fill(fill)
                            .stroke(Stroke::new(1.0, stroke))
                            .corner_radius(8)
                            .inner_margin(egui::Margin::symmetric(10, 5))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{:04X}:{:04X}", d.vid, d.pid))
                                            .monospace()
                                            .size(12.0)
                                            .color(p.muted),
                                    );
                                    ui.label(RichText::new(d.product.as_deref().unwrap_or("—")).strong().size(12.0));
                                    ui.label(
                                        RichText::new(d.manufacturer.as_deref().unwrap_or(""))
                                            .size(12.0)
                                            .color(p.muted),
                                    );
                                });
                                if let Some(h) = d.hint {
                                    ui.label(RichText::new(h).size(11.0).color(p.muted));
                                }
                            });
                    }
                });
            }
        },
    );
}
