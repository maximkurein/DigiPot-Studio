# Web frontend → egui: what becomes what

| Web (React/TS + Tauri) | egui | Notes |
|---|---|---|
| `invoke('cmd', args)` / `rpc()` | `sh.rpc("cmd", json!({…}))` → `Job::Rpc` → `Engine::handle` | Same command names and args; the core stays the single API |
| zustand/redux store `snap` | `Shared.st: State` (typed) | Filled by the worker, replaced each publish |
| polling `setInterval(get_status, 350)` | worker `recv_timeout(350 ms)` republish | No timers in the UI |
| React component | `fn foo(ui: &mut Ui, sh: &Shared, …)` | Plain functions; return `Response` or `Option<new value>` |
| `useState` | field in `ViewState`/`XState` | Only for state that outlives a frame |
| `useEffect` on change | compare old/new in the frame, or do it in the core | Usually not needed |
| router / view switch | `enum View` + `match sh.view` | Keyboard `⌘/Ctrl + N` via `ctx.input_mut(consume_key)` |
| CSS variables (`--bg`, `--accent`) | `Pal { bg, accent, … }` LIGHT/DARK consts | Copy hex values 1:1 |
| Tailwind layout | `ui.horizontal`, `ui.vertical`, `ui.columns`, `Grid`, `StripBuilder` | Width: `ui.available_width().min(MAX)` |
| card / panel div | `egui::Frame::new().fill().stroke().corner_radius().inner_margin()` | Wrap in a `widgets::card` helper |
| `<input type=range>` | `egui::Slider::new(&mut v, 0..=N)` | Keep a draft while dragging (see pitfalls) |
| number input with units | `TextEdit` + `fmt::parse_eng` on Enter/lost focus | Validate, show error toast |
| `<select>` / segmented | `ComboBox` or row of `selectable_label` | Force left-to-right layout |
| checkbox / toggle | `ui.checkbox` / custom toggle | |
| table | `egui_extras::TableBuilder` | Virtualized rows for long logs |
| tooltip `title=` | `.on_hover_text()` | |
| toast library | `Vec<Toast>` + `egui::Area` anchored bottom-right | Errors longer (6.5 s) than info (3.2 s) |
| modal / confirm() | two-step button with deadline, or `egui::Modal` | Never block |
| SVG diagram | `Canvas::new(ui, viewBox, max_h)` + painter calls in viewBox coords | Click areas: `ui.interact(rect, id, Sense::click())` |
| chart lib / custom `Plot.tsx` | `egui_plot::Plot` with `Line`, `Points`, `VLine` | Log axes: plot `log10(x)` and format ticks |
| file input / download | `rfd::FileDialog` on the UI thread | Data prep in the worker |
| `localStorage` prefs | eframe `Storage` (`persistence` feature) in `App::save` | Domain settings stay in the core's config file |
| i18n strings / data files (`blocks.ts`, `lessons.ts`) | `content.rs` consts | Keep in sync; say so in CLAUDE.md |
| `window.open(url)` | `ctx.open_url(OpenUrl::new_tab(url))` | |
| clipboard | `ctx.copy_text(s)` | |
| lucide icons | Unicode glyphs present in egui default fonts, or own painter shapes | Test every glyph in a screenshot |
| Tauri `app_config_dir()` | same path computed by hand (`<config>/<identifier>/file.json`) | Shared settings between apps |
| Tauri plugins (shell, dialog, fs) | `std::process`, `rfd`, `std::fs` | Check the inventory for each plugin |
