# Промт: перевести Tauri-приложение на egui

Инструкция для ИИ-агента (Claude Code, Codex, Cursor, Pi). Описывает, как по образцу
DigiPot Studio сделать нативный интерфейс на Rust (egui/eframe) для любого похожего проекта на
Tauri и как при этом пользоваться скилом **tauri-to-egui**.

- Скил в этом репозитории: [`.claude/skills/tauri-to-egui/`](.claude/skills/tauri-to-egui/SKILL.md)
- Копия в каталоге скилов: `~/Project/AI/skills/tauri-to-egui/`
- Живой пример с кодом: [EGUI.md](EGUI.md) и `crates/digipot-egui/`

## 1. Подключить скил

| Агент | Куда положить | Как вызвать явно |
|---|---|---|
| Claude Code (все проекты) | `cp -R ~/Project/AI/skills/tauri-to-egui ~/.claude/skills/` | `/tauri-to-egui` или «используй скил tauri-to-egui» |
| Claude Code (один проект) | `<проект>/.claude/skills/tauri-to-egui/` | то же |
| Cursor | `~/.cursor/skills/` или `<проект>/.cursor/skills/` | «используй скил tauri-to-egui» |
| Codex | `~/.codex/skills/` или основная папка проекта | `$tauri-to-egui` |
| Pi | `~/.pi/agent/skills/` или `<проект>/.agents/skills/` | `/skill:tauri-to-egui` |

После копирования начните новую сессию: список скилов читается при старте.
Скил подключается и сам — по фразам «переписать с Tauri на egui», «нативный интерфейс на
Rust», «без WebView», «собери .app и .dmg для egui».

## 2. Готовый промт (скопировать и подставить свои значения)

```text
Используй скил tauri-to-egui.

Проект: ~/Project/<Имя>  (Tauri 2 + <React/Vue/Svelte>, ядро на Rust в crates/<core>)
Задача: сделать нативный интерфейс на egui/eframe новым крейтом crates/<app>-egui рядом с Tauri.
Tauri и веб-режим не ломать. Образец — ~/Project/DigiPot-Studio (EGUI.md, crates/digipot-egui).

Порядок:
1. Запусти scripts/inventory.py скила на проекте и покажи план переноса: экраны, команды,
   графики, схемы, диалоги, опасные действия с подтверждением.
2. Добавь в ядро типизированный State и Engine::state(); JSON get_state не меняй. Тесты ядра зелёные.
3. Создай крейт из assets/template, подставь плейсхолдеры, добавь в workspace.
4. Движок — в отдельном потоке (worker.rs), экраны только читают &Shared и ставят Act.
5. Тема — из CSS-переменных веб-версии, обе темы.
6. Перенеси экраны по одному, тексты как в веб-версии (content.rs синхронно с TS-файлами).
7. Проверка: cargo test, clippy -D warnings, fmt, npm run build, снимки всех экранов
   (APP_SHOTS) в обеих темах — посмотри их и исправь вёрстку.
8. Сборка macOS: .app + .dmg (отдельный bundle id), проверь dmg и запусти приложение.
9. Обнови README, CHANGELOG, CLAUDE.md/AGENTS.md, Makefile, CI; напиши EGUI.md с примерами кода.
Не коммить и не пушь без моей команды. В конце — что проверено и что нет.
```

Короткие варианты:

- «Используй tauri-to-egui: в ~/Project/PLL-Studio сделай оболочку egui и экраны Planner и Registers.»
- «Используй tauri-to-egui: у меня уже есть eframe-крейт, сделай для него .app и .dmg.»
- «Используй tauri-to-egui: вынеси работу с движком в отдельный поток.»

## 3. Что агент должен сделать (критерии готовности)

1. **Инвентаризация до кода.** Отчёт `inventory.py` и план. Если расчёты живут во фронтенде
   (много `Math.*` в TS) — сначала перенос в Rust-ядро с тестами, отдельным шагом и с вопросом
   пользователю.
2. **Ядро — единственный источник логики.** В egui-крейте только отображение и форматирование.
   Все изменения — через `Engine::handle(cmd, args)` (тот же RPC, что у Tauri).
3. **Ничего не блокирует кадр.** Обмен с железом/сетью — в потоке движка. Диалоги `rfd` — только
   в UI-потоке.
4. **Опасные действия** (запись EEPROM/flash, удаление) — двухшаговое подтверждение в интерфейсе
   *и* отказ ядра без `confirm: true`; тест на это в `worker.rs`.
5. **Тот же файл настроек**, что у Tauri (`<config dir>/<identifier>/…`).
6. **Версия eframe** — подходящая к локальному `rustc` (на 1.88–1.90: eframe 0.33, egui_plot 0.34).
7. **Проверено**: `cargo fmt --check`, `cargo clippy -p <app>-egui -- -D warnings`,
   `cargo test -p <core> -p <app>-egui`, `npm run build`, снимки экранов просмотрены.
8. **Сборка**: `.app` + `.dmg` + `.app.zip`, `codesign --verify --strict`, `hdiutil verify`,
   приложение запускается из смонтированного образа.
9. **Документация** обновлена, `EGUI.md` написан с примерами кода именно из этого проекта.
10. **Честный отчёт**: что не проверено (клики вручную, реальное железо, Windows/Linux).

## 4. Как это было сделано в DigiPot Studio (для ориентира)

| Шаг | Результат |
|---|---|
| Новый крейт рядом | `crates/digipot-egui`: 8 экранов, журнал шины, темы, ⌘1…8 |
| Типизированное состояние | `api::State`, `Engine::state()`, `log_since`, `log_head`, `curve` |
| Поток движка | `worker.rs`: `Job`/`Reply`, публикация каждые 350 мс, слияние `set_field` |
| Проверка вида | `make egui-shots` → BMP → `sips` → просмотр |
| Сборка | `python3 build_macos.py --egui [--universal]` → `DigiPot-Studio-egui-v0.1.1-macos-*.dmg` |
| Описание | [EGUI.md](EGUI.md) |

## 5. Частые ошибки (коротко)

- Тема «плывёт» вслед за системой → `ctx.set_theme` + `set_visuals_of` для обеих тем.
- Ползунок прыгает назад → черновик значения, пока поток движка не применил запись.
- Значки-квадраты → использовать символы, которые есть в шрифтах egui; проверять по снимкам.
- Кнопки в обратном порядке → `Layout::left_to_right` для группы.
- Обрезана правая колонка → `ui.available_width().min(1600.0)`.

Полный список — `references/pitfalls.md` скила.
