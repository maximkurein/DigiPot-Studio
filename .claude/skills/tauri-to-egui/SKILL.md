---
name: tauri-to-egui
description: Port a Tauri desktop app (Rust backend + React/Vue/Svelte/TS frontend in a WebView) to a native Rust UI on egui/eframe — a new crate next to the Tauri one, same backend, no WebView and no Node. Covers the inventory of commands and screens, the engine-on-its-own-thread pattern (Job/Reply channel, coalescing slider jobs), views that only read a snapshot and queue actions, theme tokens from CSS variables, plots (egui_plot) and SVG diagrams (Painter with a viewBox helper), file dialogs on the UI thread, a screenshot tour for visual checks, and a hand-made macOS .app/.dmg (Info.plist, icns, ad-hoc codesign, hdiutil). Distilled from DigiPot Studio. Use when the user wants to rewrite, port, mirror or add a native egui/eframe front-end to a Tauri (or other WebView/Electron-with-Rust-core) app, or to bundle an eframe binary as .app/.dmg. Phrases: «переписать с Tauri на egui», «нативный интерфейс на Rust», «перевести фронтенд на egui», «без WebView», «eframe вместо React», «сделай как в DigiPot egui», «собери .app и .dmg для egui».
---

# Tauri → egui

Turn a Tauri app (Rust core + web frontend) into a native **egui/eframe** app that calls the same
Rust core directly. The default is a **new crate next to the Tauri one** — Tauri and the web build
keep working, the egui binary is an alternative front-end that shares the backend, the config file
and the rules.

Distilled from **DigiPot Studio** (`crates/digipot-egui`, ~4.2k lines, 8 screens, MCP4161 digital
pot over SPI): the walkthrough with real code is `EGUI.md` in that repository;
`references/architecture.md` here is the portable version.

## What the user gets

| Piece | Where in the result |
|---|---|
| New workspace member `crates/<app>-egui`, binary `<app>-egui` | `Cargo.toml`, `src/main.rs` |
| Backend on its own thread: `Job` channel in, `State` + replies out, slider jobs coalesced | `src/worker.rs` |
| Shell: navigation, top bar, toasts, `Act` → `Job`, shortcuts, persisted theme | `src/app.rs` |
| Screens mirroring the web views, each `show(ui, &Shared, &mut ViewState)` | `src/views/*.rs` |
| Palette = the web CSS variables, both themes pinned (not following the OS) | `src/theme.rs` |
| SVG-coordinate drawing helper for diagrams | `src/canvas.rs` |
| Display formatting only (SI prefixes, hex) | `src/fmt.rs` |
| `APP_SHOTS=<dir>` walks every screen and saves BMPs | `src/shots.rs` |
| `.app` + `.dmg` without Tauri's bundler | `build_macos.py --egui` |
| Typed snapshot in the core (`Engine::state() -> State`), JSON RPC unchanged | core `api.rs` |

Templates for all of these are in `assets/template/` (placeholders `__APP__`, `__CORE__`,
`__TITLE__`, `__BUNDLE_ID__`).

## Preconditions — check before writing code

1. **The logic must already be in Rust.** Run `python3 scripts/inventory.py <repo>`: it lists
   `#[tauri::command]`s, frontend `invoke(...)`/`rpc(...)` calls, views, Tauri plugins and any math
   that lives in TS. Physics/business logic found in TS is moved into the core *first* (with tests),
   never re-implemented in the egui crate.
2. **One entry point is ideal**: `Engine::handle(cmd, args) -> Result<Value>` (as in DigiPot). If the
   app has many separate `#[tauri::command]` functions, either call them as plain Rust functions from
   the worker or first fold them into one `handle`. Do not make egui depend on `tauri`.
3. **Pick eframe versions that build on the local rustc.** `rustc -V`, then check the eframe
   changelog's MSRV. DigiPot on Rust 1.88–1.90: `eframe 0.33`, `egui_extras 0.33`, `egui_plot 0.34`,
   `rfd 0.15` (0.34+ needs 1.92). Pin the line in a comment in `Cargo.toml`.
4. **Ask only if it is unclear**: new crate next to Tauri (default) vs replacing Tauri entirely.
   Replacing also drops the web/server mode — say so.

## Workflow

1. **Inventory** (`scripts/inventory.py`) → write the port plan as a checklist: screens, commands,
   shortcuts, persisted settings, dialogs, charts, diagrams, toasts, the bus/event log, safety
   confirmations. Map each frontend piece with `references/porting-map.md`.
2. **Core first**: add a typed `State` struct + `Engine::state()` and make the existing
   `get_state`/snapshot RPC serialize it (JSON shape unchanged, so the web UI is untouched). Add
   small typed helpers the UI needs (`log_since(id)`, `log_head()`, `curve(...)`). `cargo test` of
   the core must stay green.
3. **Scaffold the crate** from `assets/template/` (replace placeholders), add it to the workspace,
   `cargo run -p <app>-egui` shows an empty shell with navigation.
4. **Worker thread** from `assets/template/src/worker.rs`: the UI never calls the engine.
   Mutations still go through `handle`, so every rule of the core (confirmations, limits, "never
   auto-write EEPROM/flash") holds in one place. Write the worker tests (off-thread run, error →
   toast, coalescing with barriers).
5. **Theme**: copy the CSS variables into `Pal::LIGHT/DARK`, apply with `theme::apply` (pins the
   theme, sets both `Visuals`, `slider_trailing_fill`). Install fonts with the glyphs the UI needs.
6. **Screens one by one**, same order and texts as the web UI. Each view reads `sh.st`, queues
   `sh.rpc(...)`. Long-lived per-screen state (drafts, selected tab, armed confirm) goes into
   `ViewState`. Texts shared with the web live in `content.rs`, which mirrors the TS data files —
   note in the project's CLAUDE.md/AGENTS.md that they must be kept in sync.
7. **Charts and diagrams**: points come from the core; draw with `egui_plot`. SVG diagrams are
   re-drawn with `Canvas` in the same viewBox coordinates as the web SVG.
8. **Visual check**: `APP_SHOTS=target/shots cargo run -p <app>-egui`, convert with
   `sips -s format png`, look at every screenshot in both themes. Fix, repeat.
   Checklist: `references/verification.md`.
9. **Bundle** (macOS): add a `--egui` flag to the project's build script, or drop in
   `assets/template/build_egui_macos.py` — see `references/bundling.md`.
   Separate bundle id from the Tauri app so both can be installed.
10. **Docs**: README run/build lines, CHANGELOG `[Unreleased]`, CLAUDE.md/AGENTS.md architecture
    paragraph, Makefile targets (`egui`, `egui-shots`, `build-mac-egui`), CI job
    (`cargo test/clippy -p <app>-egui`; on Linux CI install `libxkbcommon-dev` etc.), and an
    `EGUI.md` walkthrough with code examples from the project.

## Rules

- **egui never re-implements backend logic.** Formatting only (`fmt.rs`); everything else is a
  core call. If a view needs a number, add it to `State` or a typed core helper.
- **Nothing blocks `update`.** Serial/USB/network I/O, sleeps, file parsing of big files → worker.
  Only `rfd` dialogs stay on the UI thread (macOS requirement): the worker prepares data
  (`Reply::ExportText`), the UI shows the dialog.
- **Views get `&Shared`, never `&mut Engine`.** They queue `Act`s; the shell runs them after the frame.
- **Destructive/wear-limited actions** keep the web UI's two-step confirmation in egui too, and the
  core still rejects them without `confirm: true`. Add a worker test that proves it.
- **Same config file as Tauri** (Tauri's `app_config_dir()` = `<OS config dir>/<tauri identifier>/`),
  so both front-ends share settings. eframe `persistence` only for UI prefs (theme, panels).
- **Keep the Tauri app building.** `npm run build` and the Tauri tests must still pass; the egui
  crate is additive.
- Pin the theme (`ctx.set_theme`) — otherwise eframe follows the OS and mixes palettes.
- Clippy clean with `-D warnings`; `#[allow(clippy::too_many_arguments)]` only on drawing helpers.
- Do not commit or push without the user asking.

## References

| File | Read when |
|---|---|
| `references/architecture.md` | Before writing the shell/worker: frame loop, `Shared`/`Act`, `Job`/`Reply`, log streaming, coalescing |
| `references/porting-map.md` | Mapping each React/TS construct (store, components, hooks, SVG, CSS, router, toasts, i18n) to egui |
| `references/pitfalls.md` | Something looks wrong: invisible text, jumping sliders, reversed buttons, missing glyphs, clipped columns, frozen window |
| `references/bundling.md` | Building `.app`/`.dmg` (and notes for Windows/Linux) |
| `references/verification.md` | Before saying "done": tests, clippy, screenshot tour, manual click list |

`scripts/inventory.py <repo>` — prints the port inventory (Markdown) to paste into the plan.
