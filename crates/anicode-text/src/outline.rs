//! Turns a laid-out Typst frame into one vector path: glyph outlines plus
//! the lines and shapes math uses (fraction bars, radicals, overlines).

use anicode_core::kurbo::{self, Affine, BezPath, Point, Rect, Shape as _, Stroke, StrokeOpts};
use typst::layout::{Frame, FrameItem, Transform};
use typst::text::TextItem;
use typst::visualize::{CurveItem, Geometry, Shape};

/// Accuracy of strokes turned into filled outlines, in points.
const STROKE_TOLERANCE: f64 = 0.01;

/// The frame as a path in points, with y pointing down like Typst.
pub fn frame_path(frame: &Frame) -> BezPath {
    let mut path = BezPath::new();
    add_frame(&mut path, frame, Affine::IDENTITY);
    path
}

fn add_frame(path: &mut BezPath, frame: &Frame, transform: Affine) {
    for (position, item) in frame.items() {
        let at = transform * Affine::translate((position.x.to_pt(), position.y.to_pt()));
        match item {
            FrameItem::Group(group) => {
                add_frame(path, &group.frame, at * affine(group.transform));
            }
            FrameItem::Text(text) => add_text(path, text, at),
            FrameItem::Shape(shape, _) => add_shape(path, shape, at),
            FrameItem::Image(..) | FrameItem::Link(..) | FrameItem::Tag(..) => {}
        }
    }
}

fn affine(transform: Transform) -> Affine {
    Affine::new([
        transform.sx.get(),
        transform.ky.get(),
        transform.kx.get(),
        transform.sy.get(),
        transform.tx.to_pt(),
        transform.ty.to_pt(),
    ])
}

fn add_text(path: &mut BezPath, text: &TextItem, at: Affine) {
    let size = text.size.to_pt();
    let scale = size / text.font.units_per_em();
    let face = text.font.ttf();
    let (mut x, mut y) = (0.0, 0.0);
    for glyph in &text.glyphs {
        let origin_x = x + glyph.x_offset.at(text.size).to_pt();
        let origin_y = y + glyph.y_offset.at(text.size).to_pt();
        // Font units point up; Typst frames point down.
        let glyph_transform = at * Affine::new([scale, 0.0, 0.0, -scale, origin_x, -origin_y]);
        let mut builder = Outline {
            path: BezPath::new(),
        };
        face.outline_glyph(ttf_parser::GlyphId(glyph.id), &mut builder);
        path.extend(glyph_transform * builder.path);
        x += glyph.x_advance.at(text.size).to_pt();
        y += glyph.y_advance.at(text.size).to_pt();
    }
}

fn add_shape(path: &mut BezPath, shape: &Shape, at: Affine) {
    let geometry = match &shape.geometry {
        Geometry::Line(end) => {
            let mut line = BezPath::new();
            line.move_to(Point::ZERO);
            line.line_to((end.x.to_pt(), end.y.to_pt()));
            line
        }
        Geometry::Rect(size) => Rect::new(0.0, 0.0, size.x.to_pt(), size.y.to_pt()).to_path(0.0),
        Geometry::Curve(curve) => {
            let mut result = BezPath::new();
            let point = |p: &typst::layout::Point| (p.x.to_pt(), p.y.to_pt());
            for item in &curve.0 {
                match item {
                    CurveItem::Move(p) => result.move_to(point(p)),
                    CurveItem::Line(p) => result.line_to(point(p)),
                    CurveItem::Cubic(a, b, c) => result.curve_to(point(a), point(b), point(c)),
                    CurveItem::Close => result.close_path(),
                }
            }
            result
        }
    };
    if shape.fill.is_some() {
        path.extend(at * geometry.clone());
    }
    if let Some(stroke) = &shape.stroke {
        let outline = kurbo::stroke(
            geometry,
            &Stroke::new(stroke.thickness.to_pt()),
            &StrokeOpts::default(),
            STROKE_TOLERANCE,
        );
        path.extend(at * outline);
    }
}

struct Outline {
    path: BezPath,
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to((f64::from(x), f64::from(y)));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to((f64::from(x), f64::from(y)));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path
            .quad_to((f64::from(x1), f64::from(y1)), (f64::from(x), f64::from(y)));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.curve_to(
            (f64::from(x1), f64::from(y1)),
            (f64::from(x2), f64::from(y2)),
            (f64::from(x), f64::from(y)),
        );
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}
