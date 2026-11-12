# Architecture of the egui front-end

Portable version of DigiPot Studio's `crates/digipot-egui`. Names below match `assets/template/`.

## Layers

```text
crates/
  <core>/        Engine, models, rules, transports — shared by Tauri, server and egui
  <app>-egui/
    main.rs      window options, eframe::run_native
    app.rs       App (eframe::App): refresh → draw → run acts; Shared, Act, ViewState
    worker.rs    engine thread: Job in, State/log/replies out
    theme.rs     Pal (CSS tokens) + theme::apply
    widgets.rs   cards, buttons, fields, tables used by all views
    canvas.rs    viewBox → pixels for hand-drawn diagrams
    content.rs   static texts mirrored from the web data files
    fmt.rs       display formatting only
    shots.rs     screenshot tour
    views/*.rs   one file per screen: pub fn show(ui, sh: &Shared, st: &mut XState)
src-tauri/       unchanged
```

## Frame loop

egui is immediate mode: `update` re-describes the whole UI every frame and each widget call
returns what happened to it this frame. The loop in `App::update`:

```rust
fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
    self.refresh(ctx);          // 1. take what the worker published (state, log, replies)
    self.shortcuts(ctx);        // 2. global keys
    let sh = &self.sh;          // 3. draw; views only get &Shared
    top_bar(ctx, sh);
    sidebar(ctx, sh);
    egui::CentralPanel::default().show(ctx, |ui| match sh.view {
        View::Main => views::main::show(ui, sh, &mut self.vs.main),
        // …
    });
    self.show_toasts(ctx);
    // 4. after the frame: run queued actions (they may mutate self)
    let acts: Vec<Act> = self.sh.acts.borrow_mut().drain(..).collect();
    for a in acts { self.run(ctx, a); }
}
```

Why "after the frame": during drawing `self.sh` is borrowed immutably by every view; running an
action (which changes `self.sh.view`, sends a job, opens a dialog) needs `&mut self`.

## Shared and Act

```rust
pub enum Act { Rpc(&'static str, Value), Go(View), Toast(Tone, String), Export, Import, Theme(bool) }

pub struct Shared {
    pub st: State,              // typed snapshot from the core
    pub pal: Pal,
    pub view: View,
    pub busy: bool,             // jobs queued or running
    acts: RefCell<Vec<Act>>,    // views push through &self
}
impl Shared {
    pub fn act(&self, a: Act) { self.acts.borrow_mut().push(a); }
    pub fn rpc(&self, cmd: &'static str, args: Value) { self.act(Act::Rpc(cmd, args)); }
}
```

`RefCell` is the only interior mutability needed: everything is single-threaded on the UI side.

## Worker (engine thread)

```rust
pub enum Job { Rpc { cmd: &'static str, args: Value, ok: Option<String> }, Export /* … */ }
pub enum Reply { Toast(Tone, String), ExportText(String) /* … */ }
```

Thread loop:

1. `rx.recv_timeout(IDLE_REFRESH)` (DigiPot: 350 ms — same cadence as the web UI polling). On
   timeout still republish `state()` so live status (simulated timers, link state) keeps moving.
2. `jobs.extend(rx.try_iter())` — take everything that piled up during the last I/O.
3. `coalesce(jobs)` — of consecutive `set_field X` jobs keep only the last; a job of another kind
   is a barrier (order of different jobs is never changed).
4. Run each through `engine.handle`; errors become `Reply::Toast(Err, msg)`.
5. Lock the inbox once, store `state`, append new log entries and replies.
6. `pending.fetch_sub(taken)`; `ctx.request_repaint()` wakes egui.

The UI side: `Worker::send(job)` increments `pending` (for the busy indicator, shown only after
~150 ms), `Worker::take()` swaps out the inbox once per frame — never waits.

### Streaming a log

If the core keeps an event/bus log with increasing ids, don't resend it each time:

```rust
// core
pub fn log_head(&self) -> u64;                           // id of the newest entry
pub fn log_since(&self, id: u64) -> impl Iterator<Item = &Txn>;

// worker
let new_log: Vec<Txn> = engine.log_since(head).cloned().collect();
head = engine.log_head();
// a `clear_log` job sets head = 0 and a `log_reset` flag so the UI drops its copy
```

Cap the UI copy (DigiPot keeps the last 2000 entries).

## Typed State in the core

```rust
#[derive(Serialize, Clone)]
pub struct State { pub regs: Words, pub fields: BTreeMap<&'static str, u32>, pub derived: Derived, /* … */ }

impl Engine {
    pub fn state(&mut self) -> State { /* build once */ }
    fn snapshot(&mut self) -> Value { serde_json::to_value(self.state()).unwrap() } // RPC unchanged
}
```

The web frontend keeps receiving the same JSON; egui reads fields directly.

## Per-view state

Anything that must survive a frame and is not in `State`: drafts of text fields, a dragged slider
value until the worker catches up, the selected tab, an armed confirm button with its deadline.
Put it in `ViewState { main: MainState, … }` owned by `App`, pass `&mut` to the view.
