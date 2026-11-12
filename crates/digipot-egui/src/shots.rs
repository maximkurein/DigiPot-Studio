//! Screenshot tour for visual checks: `DIGIPOT_SHOTS=<dir> digipot-egui`
//! walks every view (dark theme, then a few in light with the bus log open),
//! saves `NN_name.bmp` into `<dir>` and closes the window. No extra crates:
//! the frames are written as plain 24-bit BMP.

use crate::app::{View, NAV};
use eframe::egui::{self, ColorImage};
use std::path::PathBuf;

/// (view, dark theme, bus log open)
fn steps() -> Vec<(View, bool, bool)> {
    let mut v: Vec<_> = NAV.iter().map(|n| (n.0, true, false)).collect();
    v.extend([(View::Pot, false, true), (View::Registers, false, false), (View::Overview, false, false)]);
    v
}

pub struct Shots {
    dir: PathBuf,
    idx: usize,
    frames: u32,
    pending: bool,
}

impl Shots {
    pub fn from_env() -> Option<Shots> {
        let dir = PathBuf::from(std::env::var_os("DIGIPOT_SHOTS")?);
        std::fs::create_dir_all(&dir).ok()?;
        Some(Shots { dir, idx: 0, frames: 0, pending: false })
    }

    /// The step to show now: (view, dark, console).
    pub fn current(&self) -> Option<(View, bool, bool)> {
        steps().get(self.idx).copied()
    }

    /// Call once per frame after drawing.
    pub fn tick(&mut self, ctx: &egui::Context) {
        let Some((view, dark, _)) = self.current() else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            let name = format!("{:02}_{:?}_{}.bmp", self.idx + 1, view, if dark { "dark" } else { "light" });
            let _ = write_bmp(&self.dir.join(name), &img);
            self.idx += 1;
            self.frames = 0;
            self.pending = false;
        } else if !self.pending {
            self.frames += 1;
            // let layout and animations settle
            if self.frames > 12 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.pending = true;
            }
        }
        ctx.request_repaint();
    }
}

fn write_bmp(path: &std::path::Path, img: &ColorImage) -> std::io::Result<()> {
    let [w, h] = img.size;
    let row = (w * 3).div_ceil(4) * 4;
    let size = 54 + row * h;
    let mut b = Vec::with_capacity(size);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&(size as u32).to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(w as i32).to_le_bytes());
    b.extend_from_slice(&(h as i32).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&[0; 24]);
    for y in (0..h).rev() {
        let start = b.len();
        for x in 0..w {
            let c = img.pixels[y * w + x];
            b.extend_from_slice(&[c.b(), c.g(), c.r()]);
        }
        b.resize(start + row, 0);
    }
    std::fs::write(path, b)
}
