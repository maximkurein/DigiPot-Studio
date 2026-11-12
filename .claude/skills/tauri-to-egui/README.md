# Скил tauri-to-egui

Снимок скила на 12 ноября 2026. Источник — DigiPot Studio v0.1.1: нативный интерфейс
`crates/digipot-egui` (8 экранов, движок в отдельном потоке, сборка `.app`/`.dmg`),
ветки `feature/egui-ui`, `feature/egui-engine-thread`, `feature/egui-macos-bundle`.
Подробный разбор с кодом — `EGUI.md` в корне DigiPot Studio.

## Что делает

Переводит приложение на Tauri (Rust-ядро + веб-фронтенд в WebView) на нативный интерфейс
**egui/eframe**. По умолчанию — новый крейт рядом с Tauri: Tauri и веб-режим продолжают
работать, egui-версия зовёт то же ядро напрямую.

- **Инвентаризация**: `scripts/inventory.py <repo>` — команды Tauri, диспетчер RPC, вызовы из
  фронтенда, экраны, общие компоненты, файлы с текстами, математика в TS, плагины Tauri,
  identifier, CSS-переменные цветов.
- **Ядро**: типизированный снимок `Engine::state() -> State`; RPC `get_state` отдаёт тот же JSON.
- **Поток движка**: интерфейс шлёт задания (`Job`) в канал, раз в кадр забирает состояние и
  ответы; повторные записи одного поля (ползунок через медленный порт) сливаются. Окно не
  замирает при обмене с железом.
- **Экраны** читают `&Shared` и ставят действия (`Act`) в очередь — все изменения идут через
  `Engine::handle`, поэтому правила ядра (подтверждение записи EEPROM и т. п.) не обойти.
- **Тема** из CSS-переменных веб-версии, тёмная и светлая, не зависит от темы ОС.
- **Графики** — `egui_plot`, **схемы** — `Painter` в координатах SVG viewBox (`canvas.rs`).
- **Диалоги файлов** — только в UI-потоке (требование macOS).
- **Снимки всех экранов** — `APP_SHOTS=<папка>`: визуальная проверка без кликов.
- **Сборка macOS** — `.app` с иконкой и ad-hoc подписью, `.dmg` с ярлыком «Программы»,
  `.app.zip`, `checksum.txt`; отдельный bundle id, чтобы стоять рядом с Tauri-версией.
- **Документация**: README, CHANGELOG, CLAUDE.md, Makefile, CI и `EGUI.md` с примерами кода.

## Состав

```text
tauri-to-egui/
  SKILL.md                    инструкция для агента (workflow, правила)
  README.md                   этот файл
  references/
    architecture.md           цикл кадра, Shared/Act, Job/Reply, поток журнала, State в ядре
    porting-map.md            React/TS → egui: что во что превращается
    pitfalls.md               грабли из DigiPot и их решения
    bundling.md               .app/.dmg, подпись, нотаризация; заметки для Windows/Linux
    verification.md           тесты, clippy, снимки, ручная проверка
  assets/template/            заготовка крейта (__APP__, __CORE__, __TITLE__, __TAURI_ID__,
                              __CONFIG_FILE__, __BUNDLE_ID__, __SLUG__ …)
    Cargo.toml  rustfmt.toml  build_egui_macos.py
    src/ main.rs app.rs worker.rs theme.rs canvas.rs fmt.rs shots.rs views/{mod,main,about}.rs
  scripts/inventory.py        отчёт по проекту перед переносом
  evals/                      три проверочных сценария
```

Заготовка проверена: с заглушкой ядра собирается, `cargo test` (3 теста), `clippy -D warnings`
и `cargo fmt --check` проходят.

## Установка

Глобально (все проекты):

```bash
cp -R tauri-to-egui ~/.claude/skills/
```

Только в одном проекте:

```bash
mkdir -p <проект>/.claude/skills && cp -R tauri-to-egui <проект>/.claude/skills/
```

Скил появится в следующей сессии агента.

## Как вызвать

Скил срабатывает сам по фразам вроде:

- «переведи этот Tauri-проект на egui»;
- «сделай нативный интерфейс на Rust рядом с Tauri, как в DigiPot»;
- «хочу без WebView, на eframe»;
- «собери .app и .dmg для egui-версии».

Явно: `/tauri-to-egui` (Claude), `$tauri-to-egui` (Codex), `/skill:tauri-to-egui` (Pi).
Готовый промт с порядком работы — `PROMPT_TAURI_TO_EGUI.md` в корне DigiPot Studio.

## Требования к исходному проекту

- логика уже в Rust (если расчёты во фронтенде — сначала перенести их в ядро с тестами);
- лучше всего — один вход `Engine::handle(cmd, args)`; иначе воркер вызывает команды как
  обычные функции;
- Rust stable; версия eframe подбирается под локальный `rustc` (на 1.88–1.90 — 0.33/0.34).
