# Pitfalls seen in the DigiPot port

| Symptom | Cause | Fix |
|---|---|---|
| Window freezes while talking to hardware | engine called inside `update` | all engine calls in the worker thread |
| Bold/strong text invisible, plot background white in dark theme | eframe follows the OS theme; only one `Visuals` was customised | `ctx.set_theme(...)` + `ctx.set_visuals_of(Dark, v.clone())` + `set_visuals_of(Light, v)` + `all_styles_mut` |
| Slider rail invisible on dark panels | default inactive fill ≈ panel colour | `visuals.slider_trailing_fill = true`, inactive `bg_fill = panel3` |
| Slider handle jumps back after release | the worker has not applied the value yet, the view redraws from old `State` | keep `draft` while `r.dragged()` or while `sh.busy && draft != state` |
| Slider over a slow serial link lags behind | one write per frame queued | coalesce consecutive `set_field` of the same field in the worker |
| Icons show as empty squares | glyph not in egui default fonts (e.g. 🎚, ⏚) | pick glyphs that render (📈, Ω, ●, ⏫), check in screenshots |
| Buttons / segments in reversed order | parent layout is right-to-left | wrap the group in `ui.with_layout(Layout::left_to_right(Align::Center), …)` |
| Right column clipped | `ui.set_max_width(1600.0)` larger than the panel | `ui.set_max_width(ui.available_width().min(1600.0))` |
| Segmented control overflows its row | too many options on one line with a label | put it on its own line or use `horizontal_wrapped` |
| Labels overlap in a diagram | coordinates copied from SVG with different text metrics | adjust y in viewBox units; check screenshot |
| File dialog crashes / does nothing on macOS | called from the worker thread | `rfd` only on the UI thread |
| Clippy `too_many_arguments` on drawing helpers | painter helpers take many params | `#[allow(clippy::too_many_arguments)]` on those functions only |
| New eframe won't compile | MSRV jump (0.34+ needs Rust 1.92) | pin to the newest line that builds on the local rustc, comment why |
| Linux CI fails to link | missing system libs | `apt-get install libxkbcommon-dev libgl1-mesa-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libwayland-dev` |
| computer-use / UI automation cannot find the app | a bare binary has no bundle id | test clicks against the bundled `.app` |
| `checksum.txt` loses the other build's lines | build script overwrites it | merge: replace lines of produced files, keep the rest |
| `__pycache__` committed after `py_compile` of the build script | no ignore rule | add `__pycache__/` to `.gitignore` |
