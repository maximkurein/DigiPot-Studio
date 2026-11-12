//! Engine thread. The UI never blocks on hardware: it sends `Job`s over a
//! channel, the worker runs them against the `Engine` it owns and publishes
//! the new `State`, fresh bus-log entries and replies into an inbox the UI
//! drains once per frame.
//!
//! - Serial I/O, the 10 ms EEPROM wait (`nv_write`) and the USB scan run here.
//! - While idle the worker republishes every 350 ms, so simulator status
//!   (EEPROM write cycle) stays live — same cadence as the web UI's polling.
//! - Jobs that piled up while the bus was busy are coalesced: of several
//!   `set_field` for the same field (a slider drag over a slow bridge) only the
//!   last is sent. Order of everything else is kept.

use crate::app::Tone;
use digipot_core::api::{State, Txn};
use digipot_core::regs;
use digipot_core::transport::{self, PortInfo, UsbDev};
use digipot_core::Engine;
use eframe::egui;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const IDLE_REFRESH: Duration = Duration::from_millis(350);

pub enum Job {
    /// `Engine::handle(cmd, args)`; `ok` is toasted on success.
    Rpc { cmd: &'static str, args: Value, ok: Option<String> },
    /// Raw SPI frame; the reply is toasted.
    Raw { hex: String, drive: Option<u32> },
    /// Read NV_WIPER0 and the EEPROM cells, stop at the first error.
    ReadNv,
    /// Serial ports + USB scan.
    Ports,
    /// Exported configuration text (the UI shows the save dialog).
    Export,
}

pub enum Reply {
    Toast(Tone, String),
    ExportText(String),
    Ports(Vec<PortInfo>, Vec<UsbDev>),
}

/// What the worker published since the UI last looked.
pub struct Update {
    pub state: Option<State>,
    /// The bus log was cleared: drop what the UI holds before appending `log`.
    pub log_reset: bool,
    pub log: Vec<Txn>,
    pub replies: Vec<Reply>,
}

#[derive(Default)]
struct Inbox {
    state: Option<State>,
    log_reset: bool,
    log: Vec<Txn>,
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
    /// Move the engine to its own thread. Returns the worker plus the initial
    /// state and log so the first frame has something to draw.
    pub fn spawn(mut engine: Engine, ctx: egui::Context) -> (Worker, State, Vec<Txn>) {
        let state = engine.state();
        let log: Vec<Txn> = engine.log_since(0).cloned().collect();
        let (tx, rx) = mpsc::channel::<Job>();
        let inbox = Arc::new(Mutex::new(Inbox::default()));
        let pending = Arc::new(AtomicUsize::new(0));
        let (inbox2, pending2) = (inbox.clone(), pending.clone());
        std::thread::Builder::new()
            .name("digipot-engine".into())
            .spawn(move || {
                let mut head = engine.log_head();
                loop {
                    let first = match rx.recv_timeout(IDLE_REFRESH) {
                        Ok(j) => Some(j),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    let mut jobs: Vec<Job> = first.into_iter().collect();
                    jobs.extend(rx.try_iter());
                    let taken = jobs.len();
                    let mut replies = vec![];
                    let mut reset = false;
                    for job in coalesce(jobs) {
                        reset |= run(&mut engine, job, &mut replies);
                    }
                    if reset {
                        head = 0;
                    }
                    let state = engine.state();
                    let new_log: Vec<Txn> = engine.log_since(head).cloned().collect();
                    head = engine.log_head();
                    {
                        let mut ib = lock(&inbox2);
                        ib.state = Some(state);
                        if reset {
                            ib.log.clear();
                            ib.log_reset = true;
                        }
                        ib.log.extend(new_log);
                        ib.replies.extend(replies);
                    }
                    pending2.fetch_sub(taken, Ordering::SeqCst);
                    ctx.request_repaint();
                }
            })
            .expect("не удалось запустить поток движка");
        (Worker { tx, inbox, pending }, state, log)
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

    pub fn take(&self) -> Update {
        let mut ib = lock(&self.inbox);
        Update {
            state: ib.state.take(),
            log_reset: std::mem::take(&mut ib.log_reset),
            log: std::mem::take(&mut ib.log),
            replies: std::mem::take(&mut ib.replies),
        }
    }
}

/// Key under which a job may be replaced by a later one of the same kind.
fn coalesce_key(j: &Job) -> Option<String> {
    match j {
        Job::Rpc { cmd: "set_field", args, ok: None } => args["name"].as_str().map(|n| format!("set_field:{}", n)),
        Job::Rpc { cmd: "goto", args, ok: None } => args["target"].as_str().map(|t| format!("goto:{}", t)),
        _ => None,
    }
}

/// Keep only the last of each coalescible group, preserving order otherwise.
/// A job of another kind in between is a barrier: `set_field A; step; set_field A`
/// stays as is, because `step` depends on the first write.
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

/// Run one job. Returns `true` when the bus log was cleared.
fn run(engine: &mut Engine, job: Job, replies: &mut Vec<Reply>) -> bool {
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
                return cmd == "clear_log";
            }
        }
        Job::Raw { hex, drive } => {
            let args = match drive {
                Some(d) => json!({ "hex": hex, "drive": d }),
                None => json!({ "hex": hex }),
            };
            if let Some(r) = call("raw", args, replies) {
                let rx = r["rx"].as_str().filter(|s| !s.is_empty()).unwrap_or("—").to_string();
                replies.push(Reply::Toast(Tone::Info, format!("Ответ: {}", rx)));
            }
        }
        Job::ReadNv => {
            for r in (0..regs::NUM_REGS).filter(|&r| regs::nv(r)) {
                if call("read_register", json!({ "index": r }), replies).is_none() {
                    break;
                }
            }
        }
        Job::Ports => {
            let ports = transport::list_ports();
            let usb = transport::scan_usb().unwrap_or_default();
            replies.push(Reply::Ports(ports, usb));
        }
        Job::Export => {
            if let Some(r) = call("export_config", json!({}), replies) {
                replies.push(Reply::ExportText(r["text"].as_str().unwrap_or_default().to_string()));
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

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
    fn coalesces_slider_runs_but_keeps_barriers() {
        let jobs = vec![
            set("WIPER0", 1),
            set("WIPER0", 2),
            set("R0A", 0),
            set("WIPER0", 3),
            Job::Rpc { cmd: "step", args: json!({ "up": true }), ok: None },
            set("WIPER0", 4),
            set("WIPER0", 5),
        ];
        assert_eq!(describe(&coalesce(jobs)), ["set_field:0", "set_field:3", "step:null", "set_field:5"]);
    }

    /// Wait until the worker has drained the queue and published.
    fn settle(w: &Worker) -> Update {
        let t0 = Instant::now();
        let mut acc = Update { state: None, log_reset: false, log: vec![], replies: vec![] };
        while (w.busy() || acc.state.is_none()) && t0.elapsed() < Duration::from_secs(5) {
            let u = w.take();
            if u.log_reset {
                acc.log.clear();
                acc.log_reset = true;
            }
            acc.state = u.state.or(acc.state);
            acc.log.extend(u.log);
            acc.replies.extend(u.replies);
            std::thread::sleep(Duration::from_millis(5));
        }
        acc
    }

    #[test]
    fn runs_jobs_off_thread_and_publishes() {
        let (w, st0, log0) = Worker::spawn(Engine::new(None), egui::Context::default());
        assert!(!log0.is_empty(), "startup note");
        let head0 = st0.log_head;
        assert!(w.send(set("WIPER0", 0x40)));
        let u = settle(&w);
        assert_eq!(u.state.unwrap().fields["WIPER0"], 0x40);
        assert!(u.log.iter().all(|t| t.id > head0), "only new entries");

        // errors come back as toasts, the EEPROM rule still holds
        w.send(set("NV_WIPER0", 1));
        let u = settle(&w);
        assert!(matches!(u.replies.first(), Some(Reply::Toast(Tone::Err, _))));

        w.send(Job::Rpc { cmd: "clear_log", args: json!({}), ok: None });
        w.send(set("WIPER0", 0x41));
        let u = settle(&w);
        assert!(u.log_reset);
        assert!(u.log.iter().all(|t| t.reg == Some(regs::R_WIPER0) || t.op == "note"), "only entries after the clear");
        assert!(!w.busy());
    }
}
