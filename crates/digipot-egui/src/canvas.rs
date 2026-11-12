//! Tiny helper for drawings defined in SVG-like "viewBox" coordinates.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui};

pub struct Canvas {
    pub rect: Rect,
    pub k: f32,
    vx: f32,
    vy: f32,
    pub painter: Painter,
}

impl Canvas {
    /// Allocate the full available width; height follows the view box aspect.
    pub fn new(ui: &mut Ui, vb: [f32; 4], max_h: f32) -> (Canvas, Response) {
        let w = ui.available_width();
        let mut k = w / vb[2];
        if vb[3] * k > max_h {
            k = max_h / vb[3];
        }
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, vb[3] * k), Sense::hover());
        // centre horizontally when height-limited
        let pad = (w - vb[2] * k) / 2.0;
        let c = Canvas {
            rect: Rect::from_min_size(rect.min + egui::vec2(pad, 0.0), egui::vec2(vb[2] * k, vb[3] * k)),
            k,
            vx: vb[0],
            vy: vb[1],
            painter: ui.painter_at(rect),
        };
        (c, resp)
    }

    pub fn p(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + egui::vec2((x - self.vx) * self.k, (y - self.vy) * self.k)
    }
    pub fn r(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.p(x, y), egui::vec2(w * self.k, h * self.k))
    }
    pub fn s(&self, v: f32) -> f32 {
        v * self.k
    }
    pub fn line(&self, pts: &[(f32, f32)], c: Color32, width: f32) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.p(*x, *y)).collect();
        self.painter.add(Shape::line(pts, Stroke::new(self.s(width).max(1.0), c)));
    }
    pub fn dashed(&self, pts: &[(f32, f32)], c: Color32, width: f32, dash: f32, gap: f32, offset: f32) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.p(*x, *y)).collect();
        self.painter.extend(Shape::dashed_line_with_offset(
            &pts,
            Stroke::new(self.s(width).max(1.0), c),
            &[self.s(dash)],
            &[self.s(gap)],
            self.s(offset),
        ));
    }
    pub fn text(&self, x: f32, y: f32, align: Align2, s: impl ToString, size: f32, c: Color32) {
        self.painter.text(self.p(x, y), align, s.to_string(), FontId::proportional(self.s(size).max(8.0)), c);
    }
    pub fn text_bold(&self, x: f32, y: f32, align: Align2, s: impl ToString, size: f32, c: Color32) {
        // egui has no bold weight in the default fonts: draw twice with a hair offset
        let pos = self.p(x, y);
        let f = FontId::proportional(self.s(size).max(8.0));
        self.painter.text(pos, align, s.to_string(), f.clone(), c);
        self.painter.text(pos + egui::vec2(0.5, 0.0), align, s.to_string(), f, c);
    }
    pub fn circle(&self, x: f32, y: f32, r: f32, fill: Color32, stroke: Stroke) {
        let stroke = Stroke::new(self.s(stroke.width).max(1.0), stroke.color);
        self.painter.circle(self.p(x, y), self.s(r), fill, stroke);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, rad: f32, fill: Color32, stroke: Stroke) {
        let stroke = Stroke::new(self.s(stroke.width).max(1.0), stroke.color);
        self.painter.rect(self.r(x, y, w, h), self.s(rad), fill, stroke, egui::StrokeKind::Inside);
    }
    /// Clickable region in view-box coordinates.
    pub fn hit(&self, ui: &mut Ui, id: impl std::hash::Hash, x: f32, y: f32, w: f32, h: f32) -> Response {
        ui.interact(self.r(x, y, w, h), ui.id().with(("hit", id)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}
