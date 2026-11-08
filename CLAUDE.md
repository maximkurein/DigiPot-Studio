# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

DigiPot Studio: a desktop (Tauri 2, macOS + Windows) and web app for controlling and studying the Microchip **MCP4161** (8-битный цифровой потенциометр с EEPROM и SPI) and its **DIGI POT click (MIKROE-923)** evaluation board. All engineering logic is Rust (`crates/digipot-core`); the UI is React 19 + Tailwind 4 (`src/`). UI language: Russian; register/field names stay in English as in the datasheet.

## Commands

```bash
npm ci                         # first time
npm run app                    # desktop app, hot reload (Tauri dev; Vite on :1420)
make dev                       # web dev: digipot-server :7878 + Vite :1420 (Vite proxies /api)
npm run web                    # build UI, run release digipot-server on http://127.0.0.1:7878

cargo test -p digipot-core         # model tests (registers, bus framing, model, simulator, RPC)
npm run build                  # tsc --noEmit + vite build
make check                     # version status + build + tests + cargo check
cargo run -p digipot-core --example probe   # CLI sanity probe of the model

python3 build_macos.py [--universal] [--with-server]   # .app + .dmg → release/vX.Y.Z/
.\build_windows.ps1 [-Bundles nsis|msi] [-WithServer]  # NSIS/MSI → release\vX.Y.Z\
```

## Versioning and releases

- Single source of truth: `config/version.json` (`app_info.version`, `release_date`, `last_updated`). Never edit versions by hand. Use `python3 tools/update_version.py status | bump patch|minor|major | sync [X.Y.Z]`. It updates `Cargo.toml` (`[workspace.package]`), `Cargo.lock`, `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, the README heading, `.github/ISSUE_TEMPLATE/bug.yml` and `crates/digipot-core/src/about.rs`.
- `config/version.json` also carries the project identity (`name`, `author` = Kurein M N, `license`, `repository`, `mirror`). The sync script writes it into `about.rs`, which `get_meta` returns as `meta.app` and the About screen (`src/views/AboutView.tsx`) displays. Repos: GitHub <https://github.com/maximkurein/DigiPot-Studio>, Forgejo mirror <http://100.64.0.12:3000/mxm/DigiPot-Studio>.
- `CHANGELOG.md` is hand-written (Keep a Changelog): `## [X.Y.Z] - YYYY-MM-DD`.
- Commit subjects: user-facing description, then `; bump to X.Y.Z` when the version changes.
- Build scripts refuse to build if `status` reports drift. Tag `vX.Y.Z` triggers `.github/workflows/release.yml` (tauri-action, draft release).

## Architecture

**One RPC for both runtimes.** `digipot_core::Engine::handle(cmd, args) -> Result<Value>` (`crates/digipot-core/src/api.rs`) is the whole backend API. Tauri exposes it as the `rpc` command (`src-tauri/src/lib.rs`), `digipot-server` as `POST /api/rpc` → `{ok, data|error}`. The frontend calls `rpc()` from `src/lib/api.ts`, which picks the transport. Add features by adding an RPC command in `api.rs` and mirroring its types in `src/lib/types.ts`.

**digipot-core modules**
- `regs.rs` — `REGS` (address, name, reset) and `FIELDS` (exact bit positions from the datasheet, access rw/ro/sticky/fixed, defaults, RU title/description, options, owning block, unit, signed), `defaults()` (datasheet power-on) vs `starter()` (working initial config), `write_order()`, optional `COMMIT_REG` for double-buffered chips.
- `bus.rs` — MCP4161 command format (`Cmd`: write/read 16-bit, inc/dec 8-bit), `parse()` for continuous commands, `Frame::Spi3` 3-wire read (MISO is NC on the click; SDI/SDO shares MOSI), CMDERR in `parse_read()`.
- `model.rs` — `Settings` (board-level parameters), `calc()` typed results, `derive()` → `{metrics, issues}` (RU messages, `block` key for UI).
- `sim.rs` — `VirtualChip`: POR loads NV wiper, EEPROM write cycle with EEWA, inc/dec limits, CMDERR rules, SDI/SDO contention on full-duplex reads, `meter()` with a per-part RAB deviation.
- `transport.rs` — `Link` trait (sim, `SerialBridge` text protocol `BRIDGE`), port listing, USB scan with vendor hints, `PINS`.
- `api.rs` — `Engine`: auto-apply writes only volatile WIPER0/TCON; EEPROM only via `nv_write {confirm: true}` (waits tWC); `step`, `goto`, `power_cycle` (sim), `curve`; bus log; snapshot.

**Frontend** — `src/lib/store.ts` (zustand): `snap` (full snapshot after every mutating RPC), `status` (polled every 350 ms), log, view routing. `src/components/fields.tsx` renders any register field from metadata. Views live in `src/views/*View.tsx`; the overview diagram is drawn from `src/lib/blocks.ts`, whose ids match `FieldDef.block`. Board facts live in `src/lib/board.ts`, lessons in `src/lib/lessons.ts`. Charts are the in-house `components/Plot.tsx` (SVG, log axes, dual y, hover).

## Conventions

- UI strings and model messages are Russian. Datasheet names stay English.
- Register bit positions come only from the datasheet — never guess; `regs.rs` tests guard overlaps, defaults and field round-trips.
- Keep physics in Rust; the frontend formats and plots. Do not duplicate model math in TS beyond trivial display formulas.
- `tsconfig` is strict with `noUnusedLocals/Parameters`; `npm run build` must pass.
- lucide-react 1.x has no brand icons (e.g. no `Github`).
- Hardware safety notes (SPI levels = chip VDD, one SPI master — stop a MikroE host board, A/W/B within 0…VDD and ≤ 2.5 mA) must stay in docs/UI.
- Never let auto-apply or UI sliders write EEPROM: each write costs one of ~1M cycles.
