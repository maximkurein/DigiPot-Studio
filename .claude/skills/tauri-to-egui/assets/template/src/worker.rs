//! Engine thread. The UI never blocks on I/O: it sends `Job`s over a channel,
//! the worker runs them against the `Engine` it owns and publishes the new
//! `State` and replies into an inbox the UI drains once per frame.
//!
//! Template from DigiPot Studio (`crates/digipot-egui/src/worker.rs`).
//! Assumes the core has `Engine::handle(&mut self, &str, Value) -> Result<Value, E: Display>`
//! and `Engine::state(&mut self) -> State`. Add a log stream (`log_since`/`log_head`)
//! if the core keeps an event log — see references/architecture.md.

use crate::app::Tone;
use __CORE__::{Engine, State};
use eframe::egui;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// Republish while idle so live status keeps moving (match the web UI polling).
const IDLE_REFRESH: Duration = Duration::from_millis(350);

pub enum Job {
    /// `Engine::handle(cmd, args)`; `ok` is toasted on success.
    Rpc { cmd: &'static str, args: Value, ok: Option<String> },
    /// Text to save (the UI shows the save dialog — dialogs stay on the UI thread).
    Export,
}

pub enum Reply {
    Toast(Tone, String),
    ExportText(String),
}

/// What the worker published since the UI last looked.
pub struct Update {
    pub state: Option<State>,
    pub replies: Vec<Reply>,
}

#[derive(Default)]
struct Inbox {
    state: Option<State>,
    replies: Vec<Reply>,
}

pub struct Worker {
    tx: Sender<Job>,
    inbox: Arc<Mutex<Inbox>>,
    pending: Arc<AtomicUsize>,
}

fn lock(m: &Mutex<Inbox>) -> MutexGuard<'_, Inbox> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Worker {
    /// Move the engine to its own thread. Returns the worker and the initial state.
    pub fn spawn(mut engine: Engine, ctx: egui::Context) -> (Worker, State) {
        let state = engine.state();
        let (tx, rx) = mpsc::channel::<Job>();
        let inbox = Arc::new(Mutex::new(Inbox::default()));
        let pending = Arc::new(AtomicUsize::new(0));
        let (inbox2, pending2) = (inbox.clone(), pending.clone());
        std::thread::Builder::new()
            .name("engine".into())
            .spawn(move || loop {
                let first = match rx.recv_timeout(IDLE_REFRESH) {
                    Ok(j) => Some(j),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                };
                let mut jobs: Vec<Job> = first.into_iter().collect();
                jobs.extend(rx.try_iter());
                let taken = jobs.len();
                let mut replies = vec![];
                for job in coalesce(jobs) {
                    run(&mut engine, job, &mut replies);
                }
                let state = engine.state();
                {
                    let mut ib = lock(&inbox2);
                    ib.state = Some(state);
                    ib.replies.extend(replies);
                }
                pending2.fetch_sub(taken, Ordering::SeqCst);
                ctx.request_repaint();
            })
            .expect("failed to start the engine thread");
        (Worker { tx, inbox, pending }, state)
    }

    /// Queue a job; `false` if the engine thread is gone.
    pub fn send(&self, job: Job) -> bool {
        self.pending.fetch_add(1, Ordering::SeqCst);
        if self.tx.send(job).is_err() {
            self.pending.fetch_sub(1, Ordering::SeqCst);
            return false;
        }
        true
    }

    /// Jobs queued or running.
    pub fn busy(&self) -> bool {
        self.pending.load(Ordering::SeqCst) > 0
    }

    /// Never waits: takes whatever was published.
    pub fn take(&self) -> Update {
        let mut ib = lock(&self.inbox);
        Update { state: ib.state.take(), replies: std::mem::take(&mut ib.replies) }
    }
}

/// Key under which a job may be replaced by a later one of the same kind.
/// Adapt to the core's command names (here: `set_field {name, value}`).
fn coalesce_key(j: &Job) -> Option<String> {
    match j {
        Job::Rpc { cmd: "set_field", args, ok: None } => args["name"].as_str().map(|n| format!("set_field:{}", n)),
        _ => None,
    }
}

/// Keep only the last of each coalescible run, preserving order otherwise.
/// A job of another kind in between is a barrier.
fn coalesce(jobs: Vec<Job>) -> Vec<Job> {
    let keys: Vec<Option<String>> = jobs.iter().map(coalesce_key).collect();
    let mut out = Vec::with_capacity(jobs.len());
    for (i, job) in jobs.into_iter().enumerate() {
        let superseded = keys[i].as_ref().is_some_and(|k| {
            keys[i + 1..].iter().take_while(|n| n.is_some()).any(|n| n.as_deref() == Some(k.as_str()))
        });
        if !superseded {
            out.push(job);
        }
    }
    out
}

fn run(engine: &mut Engine, job: Job, replies: &mut Vec<Reply>) {
    let mut call = |cmd: &str, args: Value, replies: &mut Vec<Reply>| match engine.handle(cmd, args) {
        Ok(v) => Some(v),
        Err(e) => {
            replies.push(Reply::Toast(Tone::Err, e.to_string()));
            None
        }
    };
    match job {
        Job::Rpc { cmd, args, ok } => {
            if call(cmd, args, replies).is_some() {
                if let Some(msg) = ok {
                    replies.push(Reply::Toast(Tone::Ok, msg));
                }
            }
        }
        Job::Export => {
            if let Some(r) = call("export_config", json!({}), replies) {
                replies.push(Reply::ExportText(r["text"].as_str().unwrap_or_default().to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(name: &str, v: u32) -> Job {
        Job::Rpc { cmd: "set_field", args: json!({ "name": name, "value": v }), ok: None }
    }

    fn describe(jobs: &[Job]) -> Vec<String> {
        jobs.iter()
            .map(|j| match j {
                Job::Rpc { cmd, args, .. } => format!("{}:{}", cmd, args.get("value").cloned().unwrap_or(Value::Null)),
                _ => "other".into(),
            })
            .collect()
    }

    #[test]
    fn coalesces_runs_but_keeps_barriers() {
        let jobs = vec![
            set("A", 1),
            set("A", 2),
            set("B", 0),
            set("A", 3),
            Job::Rpc { cmd: "step", args: json!({}), ok: None },
            set("A", 4),
            set("A", 5),
        ];
        assert_eq!(describe(&coalesce(jobs)), ["set_field:0", "set_field:3", "step:null", "set_field:5"]);
    }

    // Add for the real core (see DigiPot's worker.rs):
    // - runs_jobs_off_thread_and_publishes: send a job, poll `take()` until !busy, assert State changed;
    // - a forbidden action comes back as Reply::Toast(Tone::Err, _).
}
