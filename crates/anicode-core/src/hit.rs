use kurbo::{ParamCurveNearest, Point, Shape};

use crate::scene::DrawItem;

/// Index of the topmost item drawn at `point` (in pixels). Strokes count
/// within `slop` pixels, so thin lines can still be picked.
pub fn hit_test(items: &[DrawItem], point: Point, slop: f64) -> Option<usize> {
    items.iter().rposition(|item| {
        let local = item.transform.inverse() * point;
        if item.fill.is_some() && item.path.contains(local) {
            return true;
        }
        let Some((_, width)) = item.stroke else {
            return false;
        };
        let pixels_per_unit = item.transform.determinant().abs().sqrt();
        let reach = width / 2.0 + slop / pixels_per_unit;
        item.path
            .segments()
            .any(|segment| segment.nearest(local, 1e-6).distance_sq <= reach * reach)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{Affine, Circle, Line};

    fn item(path: impl Shape, x: f64, fill: bool) -> DrawItem {
        DrawItem {
            node: 0,
            path: path.to_path(0.01),
            transform: Affine::translate((x, 0.0)) * Affine::scale(10.0),
            fill: fill.then_some([1.0; 4]),
            stroke: (!fill).then_some(([1.0; 4], 0.1)),
        }
    }

    #[test]
    fn picks_the_topmost_filled_shape() {
        let items = [
            item(Circle::new(Point::ZERO, 1.0), 0.0, true),
            item(Circle::new(Point::ZERO, 1.0), 5.0, true),
        ];
        assert_eq!(hit_test(&items, Point::new(4.0, 0.0), 0.0), Some(1));
        assert_eq!(hit_test(&items, Point::new(-9.0, 0.0), 0.0), Some(0));
        assert_eq!(hit_test(&items, Point::new(0.0, 11.0), 0.0), None);
    }

    #[test]
    fn strokes_are_hit_near_the_line_only() {
        let items = [item(Line::new((0.0, 0.0), (2.0, 0.0)), 0.0, false)];
        // The stroke is 0.1 units (1 pixel) wide.
        assert_eq!(hit_test(&items, Point::new(10.0, 0.4), 0.0), Some(0));
        assert_eq!(hit_test(&items, Point::new(10.0, 2.0), 0.0), None);
        assert_eq!(hit_test(&items, Point::new(10.0, 2.0), 3.0), Some(0));
    }

    #[test]
    fn unfilled_shapes_are_hit_on_the_outline() {
        let items = [item(Circle::new(Point::ZERO, 1.0), 0.0, false)];
        assert_eq!(hit_test(&items, Point::new(0.0, 0.0), 0.0), None);
        assert_eq!(hit_test(&items, Point::new(10.0, 0.0), 0.0), Some(0));
    }
}
