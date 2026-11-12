# Verification before "done"

## Automatic

```bash
cargo fmt --all -- --check
cargo clippy -p <app>-egui -- -D warnings
cargo test -p <core> -p <app>-egui
npm run build                     # Tauri/web side still builds
```

Worker tests to have (template: `assets/template/src/worker.rs`):

- a job runs off the UI thread and the new `State` is published;
- a forbidden action (e.g. writing wear-limited memory without confirm) comes back as an error toast;
- `clear_log` resets the streamed log;
- coalescing keeps the last of a run and treats other jobs as barriers.

## Screenshot tour

```bash
APP_SHOTS=target/shots cargo run -p <app>-egui
for f in target/shots/*.bmp; do sips -s format png "$f" --out "${f%.bmp}.png" >/dev/null; done
```

Open every PNG (Read tool shows images). Check in both themes: text contrast, plot backgrounds,
slider rails, icons rendered (no tofu squares), nothing clipped on the right, no overlapping labels,
same texts as the web UI.

## Manual / interactive (say explicitly if not done)

- drag sliders fast; the handle must not jump back;
- every destructive action asks twice and is rejected by the core without confirm;
- import / export dialogs open and write files;
- keyboard shortcuts;
- with real hardware or the real backend (not only the simulator), if it exists.

UI automation tools usually need the bundled `.app` (a bundle id) to target the window.
