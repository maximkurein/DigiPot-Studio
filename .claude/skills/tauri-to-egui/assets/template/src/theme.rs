//! Colour tokens and egui style. Template from DigiPot Studio: replace the hex
//! values in LIGHT/DARK with the CSS variables of the web UI (same names help).

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

#[derive(Clone, Copy, Debug)]
pub struct Pal {
    pub dark: bool,
    pub bg: Color32,
    pub bg2: Color32,
    pub panel: Color32,
    pub panel2: Color32,
    pub panel3: Color32,
    pub line: Color32,
    pub line2: Color32,
    pub fg: Color32,
    pub fg2: Color32,
    pub muted: Color32,
    pub sig: Color32,
    pub clk: Color32,
    pub refc: Color32,
    pub dig: Color32,
    pub pwr: Color32,
    pub ctl: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub accent: Color32,
    pub accent_fg: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const LIGHT: Pal = Pal {
    dark: false,
    bg: hex(0xf4f6fa),
    bg2: hex(0xeceff5),
    panel: hex(0xffffff),
    panel2: hex(0xf7f8fb),
    panel3: hex(0xeef1f6),
    line: hex(0xdde2ea),
    line2: hex(0xc9d0db),
    fg: hex(0x0f1623),
    fg2: hex(0x39465a),
    muted: hex(0x6b778a),
    sig: hex(0xd97706),
    clk: hex(0x0891b2),
    refc: hex(0x7c3aed),
    dig: hex(0x059669),
    pwr: hex(0xe11d48),
    ctl: hex(0x475569),
    ok: hex(0x059669),
    warn: hex(0xc58a06),
    err: hex(0xdc2626),
    accent: hex(0x4f46e5),
    accent_fg: hex(0xffffff),
};

pub const DARK: Pal = Pal {
    dark: true,
    bg: hex(0x090c12),
    bg2: hex(0x0c1018),
    panel: hex(0x10151f),
    panel2: hex(0x141a26),
    panel3: hex(0x1a2130),
    line: hex(0x222b3b),
    line2: hex(0x2d384b),
    fg: hex(0xe8edf5),
    fg2: hex(0xb6c1d3),
    muted: hex(0x7d8aa0),
    sig: hex(0xfbbf24),
    clk: hex(0x22d3ee),
    refc: hex(0xa78bfa),
    dig: hex(0x34d399),
    pwr: hex(0xfb7185),
    ctl: hex(0x94a3b8),
    ok: hex(0x34d399),
    warn: hex(0xfbbf24),
    err: hex(0xf87171),
    accent: hex(0x818cf8),
    accent_fg: hex(0x0b0e16),
};

impl Pal {
    pub fn get(dark: bool) -> Pal {
        if dark {
            DARK
        } else {
            LIGHT
        }
    }

    /// Colour token by name, as used by blocks/metrics ("sig", "clk", "ref" …).
    pub fn tone(&self, name: &str) -> Color32 {
        match name {
            "sig" => self.sig,
            "clk" => self.clk,
            "ref" => self.refc,
            "dig" => self.dig,
            "pwr" => self.pwr,
            "ctl" => self.ctl,
            "ok" => self.ok,
            "warn" => self.warn,
            "err" | "error" => self.err,
            "accent" | "info" => self.accent,
            "muted" | "off" => self.muted,
            _ => self.fg,
        }
    }
}

/// `color-mix(in srgb, c pct%, base)`.
pub fn mix(c: Color32, pct: f32, base: Color32) -> Color32 {
    let t = (pct / 100.0).clamp(0.0, 1.0);
    let l = |a: u8, b: u8| (a as f32 * t + b as f32 * (1.0 - t)).round() as u8;
    Color32::from_rgb(l(c.r(), base.r()), l(c.g(), base.g()), l(c.b(), base.b()))
}

pub fn apply(ctx: &egui::Context, p: &Pal) {
    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.panel2;
    v.faint_bg_color = p.panel2;
    v.code_bg_color = p.panel3;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.override_text_color = Some(p.fg);
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = mix(p.accent, 35.0, p.panel);
    v.selection.stroke = Stroke::new(1.0, p.fg);
    v.slider_trailing_fill = true;
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.err;
    let r = CornerRadius::same(7);
    for (w, fill, stroke) in [
        (&mut v.widgets.noninteractive, p.panel, p.line),
        (&mut v.widgets.inactive, p.panel3, p.line),
        (&mut v.widgets.hovered, p.panel3, p.line2),
        (&mut v.widgets.active, p.panel3, p.accent),
        (&mut v.widgets.open, p.panel3, p.line2),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, stroke);
        w.corner_radius = r;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.fg2);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.fg);
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, p.fg);
    v.widgets.active.fg_stroke = Stroke::new(2.0, p.fg);
    v.window_corner_radius = CornerRadius::same(12);
    // The app picks its own theme; don't follow the OS (that would swap in
    // egui's stock style for the other theme).
    ctx.set_theme(if p.dark { egui::Theme::Dark } else { egui::Theme::Light });
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);

    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(9.0, 4.0);
        s.spacing.interact_size.y = 26.0;
        s.text_styles = [
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(12.5, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
        ]
        .into();
    });
}

/// Fonts: egui defaults plus a system font with wide Unicode coverage
/// (Cyrillic, ≤ ≈ ≠ → ✓ · −) as fallback, so no glyph shows as a box.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates: &[&str] = &[
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\seguisym.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    for (i, path) in candidates.iter().enumerate() {
        if let Ok(bytes) = std::fs::read(path) {
            let name = format!("system-{}", i);
            fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes).into());
            for fam in [FontFamily::Proportional, FontFamily::Monospace] {
                fonts.families.entry(fam).or_default().push(name.clone());
            }
        }
    }
    ctx.set_fonts(fonts);
}
