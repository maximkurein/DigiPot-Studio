//! About: version, author, repositories, terms of use.

use crate::app::{Shared, Tone, View};
use crate::widgets::{self as w, Btn};
use digipot_core::{about, bus};
use eframe::egui::{self, RichText, Stroke, Ui};

/// Only http(s) links are opened, so nothing can launch a local program.
fn open_url(ui: &Ui, sh: &Shared, url: &str) {
    if url.starts_with("http://") || url.starts_with("https://") {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    } else {
        sh.toast(Tone::Err, format!("поддерживаются только ссылки http/https: {}", url));
    }
}

fn repo(ui: &mut Ui, sh: &Shared, icon: &str, title: &str, note: &str, url: &str) {
    let p = sh.pal;
    egui::Frame::new().fill(p.panel2).stroke(Stroke::new(1.0, p.line)).corner_radius(10).inner_margin(10).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(18.0).color(p.accent));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new(title).strong().size(12.5));
                    ui.label(RichText::new(url).monospace().size(11.0).color(p.muted));
                    ui.label(RichText::new(note).size(11.0).color(p.muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if w::button(ui, &p, "↗ Открыть", Btn::Default).clicked() {
                        open_url(ui, sh, url);
                    }
                    if w::icon_button(ui, &p, "📋", false).on_hover_text("Скопировать команду git clone").clicked()
                    {
                        ui.ctx().copy_text(format!("git clone {}.git", url));
                        sh.toast(Tone::Ok, "Команда git clone скопирована");
                    }
                });
            });
        },
    );
}

pub fn show(ui: &mut Ui, sh: &Shared) {
    let p = sh.pal;
    w::page_header(ui, &p, "ℹ", "О программе", "Версия, разработчик, исходный код и условия использования.", |_| {});
    ui.set_max_width(ui.available_width().min(1100.0));
    let half = ((ui.available_width() - 16.0) / 2.0).max(380.0);
    w::two_cols(
        ui,
        half,
        &mut (),
        |ui, _| {
            w::card(ui, &p, "", "", |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 18.0, p.accent);
                    ui.painter().text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        "Ω",
                        egui::FontId::proportional(36.0),
                        p.accent_fg,
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new(about::NAME).size(18.0).strong());
                        ui.label(
                            RichText::new(format!("Версия {} · от {}", about::VERSION, about::RELEASE_DATE))
                                .size(12.5)
                                .color(p.muted),
                        );
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Разработчик").size(12.5).color(p.muted));
                            ui.label(RichText::new(about::AUTHOR).size(12.5).strong());
                        });
                    });
                });
                ui.add_space(10.0);
                let year = about::RELEASE_DATE.rsplit('.').next().unwrap_or("");
                let rows = [
                    ("⚖ Лицензия", format!("{} · © {} {}", about::LICENSE, year, about::AUTHOR)),
                    ("Кристалл", "Microchip MCP4161".into()),
                    ("Плата", "DIGI POT click (MIKROE-923)".into()),
                    ("Шина", bus::describe()),
                    ("Режим запуска", "Приложение (egui, нативное)".into()),
                    (
                        "Подключение",
                        if sh.is_sim() {
                            "Симулятор кристалла".into()
                        } else {
                            sh.st.link.name.clone()
                        },
                    ),
                ];
                for (k, v) in rows {
                    w::row(ui, &p, k, "", |ui| {
                        ui.label(RichText::new(v).size(12.5));
                    });
                }
            });
        },
        |ui, _| {
            w::card(
                ui,
                &p,
                "Исходный код",
                "Проект открыт под лицензией MIT",
                |ui| {
                    repo(
                        ui,
                        sh,
                        "🌐",
                        "GitHub — основной репозиторий",
                        "Релизы, сборки CI, обсуждения и issue.",
                        about::REPOSITORY,
                    );
                    repo(
                        ui,
                        sh,
                        "🖧",
                        "Forgejo — зеркало в локальной сети",
                        "Доступно из сети лаборатории (100.64.0.12:3000).",
                        about::MIRROR,
                    );
                    if w::button(ui, &p, "📖 Документация проекта (README)", Btn::Ghost).clicked()
                    {
                        open_url(ui, sh, &format!("{}#readme", about::REPOSITORY));
                    }
                },
            );
            w::card(ui, &p, "🛡 Условия использования", "", |ui| {
                w::hint_text(
                    ui,
                    &p,
                    p.warn,
                    "Программа поставляется «как есть». Модель — инженерное приближение по даташиту MCP4161 (DS22059B); для точных применений калибруйте по измерениям. Уровни SPI равны VDD кристалла, напряжения на A, W, B — только 0…VDD, ток — не больше 2.5 мА. Если click стоит в отладочной плате MikroE, её микроконтроллер тоже мастер SPI — остановите его.",
                );
                ui.horizontal(|ui| {
                    if w::button(ui, &p, "Правила безопасности платы", Btn::Ghost).clicked() {
                        sh.go(View::Board);
                    }
                    if w::button(ui, &p, "Академия", Btn::Ghost).clicked() {
                        sh.go(View::Learn);
                    }
                });
            });
        },
    );
}
