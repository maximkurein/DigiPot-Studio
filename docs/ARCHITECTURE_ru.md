# Архитектура кода

<p align="center">
  <img src="images/software-architecture.svg" alt="Архитектура" width="100%">
</p>

## Один RPC

Весь бэкенд — `Engine::handle(cmd, args) -> json` в `crates/digipot-core/src/api.rs`. Desktop-оболочка вызывает его командой Tauri `rpc`, веб-сервер `digipot-server` — через `POST /api/rpc`. Интерфейс вызывает `rpc()` из `src/lib/api.ts` и не знает, какой транспорт работает.

Новая функция = новая ветка `match cmd` в `api.rs` + типы в `src/lib/types.ts` + вызов `useStore().call()`.

## Ядро (`crates/digipot-core`)

| Модуль | Ответственность |
|---|---|
| `regs.rs` | память MCP4161 (16 × 9 бит, Table 4-1): WIPER0, NV_WIPER0, TCON, STATUS, ячейки 06h…0Fh; поля по даташиту; `nv()` отмечает EEPROM, которую автозапись не трогает |
| `bus.rs` | команды MCP4161 (адрес 4 + операция 2 + данные 10; инкремент/декремент — 1 байт), разбор кадра, CMDERR, 3-проводное чтение (`Frame::Spi3`) |
| `model.rs` | R_WB/R_WA, V_W с нагрузкой (узловой анализ), шаги, ток, полоса, температура; кривые по всем кодам; предупреждения по пределам даташита |
| `sim.rs` | виртуальный MCP4161: POR из EEPROM, цикл записи EEPROM (EEWA), пределы инкремента, CMDERR, конфликт на SDI/SDO при полнодуплексном чтении, «мультиметр» с R_AB экземпляра |
| `transport.rs` | симулятор, USB-мост (команды `S` и `T`), список портов, сканер USB |
| `about.rs` | сведения о программе (имя, версия, дата выпуска, разработчик, лицензия, ссылки на репозитории) — синхронизируется из `config/version.json`, отдаётся в `get_meta` как `meta.app` |
| `api.rs` | движок: автозапись WIPER0/TCON, `step`, `goto`, `nv_write` (только с `confirm`), `power_cycle`, `curve`, журнал шины, сохранение настроек |

## Интерфейс (`src`)

- `lib/store.ts` — состояние (zustand): снимок после каждой команды, опрос статуса раз в 350 мс, журнал.
- `components/fields.tsx` — редактор любого поля по метаданным из Rust.
- `lib/blocks.ts` — блоки схемы на экране «Обзор»; `id` совпадает с `FieldDef.block`.
- `lib/board.ts` — факты о плате; `lib/lessons.ts` — уроки «Академии».
- `views/*View.tsx` — экраны; `components/Plot.tsx` — графики (SVG, лог-оси, две оси Y).
- `views/AboutView.tsx` — «О программе»: всё берётся из `meta.app`; внешние ссылки открываются через `openUrl()` (в Tauri — команда `open_url`, пропускает только http/https; в браузере — `window.open`).

## Тесты

`cargo test -p digipot-core`: поля не перекрываются, значения сброса совпадают с даташитом, битовые строки команд — с Table 7-2, симулятор соблюдает POR, EEWA, пределы инкремента и CMDERR, формулы модели — с Equation 5-2, RPC и экспорт/импорт работают.
