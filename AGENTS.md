# AGENTS.md

Инструкции для ИИ-агентов (Codex, Cursor, Claude Code и др.), работающих с этим репозиторием. Подробная техническая справка — в [CLAUDE.md](CLAUDE.md).

## Проект

DigiPot Studio — приложение для управления и изучения Microchip MCP4161 (8-битный цифровой потенциометр с EEPROM и SPI) и платы DIGI POT click (MIKROE-923). Rust-ядро (`crates/digipot-core`), два интерфейса с одинаковыми экранами — React 19 + Tailwind 4 (`src`, в desktop-оболочке Tauri 2 `src-tauri` и в веб-режиме через `crates/digipot-server`) и нативный egui/eframe (`crates/digipot-egui`), прошивка USB-моста (`firmware/bridge`).

## Обязательные правила

1. **Версия** меняется только через `python3 tools/update_version.py` (`status`, `bump patch|minor|major`, `sync X.Y.Z`). Источник — `config/version.json`. Никаких ручных правок версий в `Cargo.toml`, `package.json`, `tauri.conf.json`, `crates/digipot-core/src/about.rs`.
   Там же живут сведения о проекте (`author` — Kurein M N, `license`, `repository`, `mirror`); их показывает экран «О программе» через `meta.app`.
2. **CHANGELOG.md** обновляется вручную в разделе `## [Unreleased]` при каждом заметном изменении; при релизе раздел переименовывается в `## [X.Y.Z] - YYYY-MM-DD`.
3. **Перед завершением работы**: `make check` (или по отдельности: `python3 tools/update_version.py status`, `npm run build`, `cargo test -p digipot-core -p digipot-egui`, `cargo check -p digipot-server -p digipot-studio -p digipot-egui`).
4. **Позиции битов регистров** — только из даташита MCP4161. Не угадывать. При изменении `regs.rs` обновить тесты.
5. **Физика и расчёты** — в Rust (`digipot-core`). Оба интерфейса только отображают.
6. **Новая функция бэкенда** = команда в `Engine::handle` (`api.rs`) + типы в `src/lib/types.ts` + вызов через `useStore().call()` + то же в egui (`sh.rpc(...)` в `crates/digipot-egui/src/views/`). Тексты экранов: `crates/digipot-egui/src/content.rs` держать синхронно с `src/lib/blocks.ts`, `board.ts`, `lessons.ts`.
7. **Язык интерфейса** — русский; имена регистров и полей — как в даташите (английские).
8. **Безопасность железа**: предупреждения об уровнях логики и изоляции штатного USB-контроллера платы не удалять из документации и интерфейса.

## Стиль коммитов

`<описание для пользователя>` и, если менялась версия, `; bump to X.Y.Z`. Пример: `Экран измерения показывает гистограмму шума; bump to 0.1.1`.

## Структура документации

| Файл | Назначение |
|---|---|
| `README.md` | обзор, быстрый старт, возможности |
| `BUILD.md` | сборка macOS / Windows / веб |
| `VERSION_MANAGEMENT.md` | версия и релизный чек-лист |
| `CHANGELOG.md` | история изменений |
| `SECURITY.md` | безопасность ПО и железа |
| `docs/INDEX_ru.md` | индекс руководств |
| `docs/FIRST_RUN_ru.md` | первый запуск с платой |
| `docs/BRIDGE_ru.md` | прошивка, схема и протокол USB-моста |
| `docs/ARCHITECTURE_ru.md` | архитектура кода |
| `EGUI.md` | нативный интерфейс egui с примерами кода |
| `PROMPT_TAURI_TO_EGUI.md` | промт для ИИ: перенос Tauri-проекта на egui скилом `tauri-to-egui` |
| `docs/release_guide_ru.md` | выпуск релиза |

## Что не коммитить

`target/`, `node_modules/`, `dist/`, `release/`, `src-tauri/gen/`, `.env`, `.claude/settings.local.json`, даташиты производителя (PDF) — ссылки на них давать в документации.
