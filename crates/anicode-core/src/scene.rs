use std::collections::HashMap;

use kurbo::{Affine, BezPath, Circle, Point, RoundedRect, Shape as _};

use crate::property::{Properties, PropertyId};
use crate::timeline::{self, BlockTime};

pub type NodeId = usize;
/// Straight (not premultiplied) RGBA, each channel 0 to 1.
pub type Color = [f32; 4];

#[derive(Debug)]
pub enum Shape {
    Group,
    Rect {
        width: PropertyId,
        height: PropertyId,
        radius: PropertyId,
    },
    Circle {
        radius: PropertyId,
    },
    Line {
        from: Point,
        to: Point,
    },
    Path {
        points: Vec<Point>,
        closed: bool,
    },
    /// A ready-made outline in scene units, such as typeset math.
    Outline {
        path: BezPath,
    },
}

#[derive(Debug)]
pub struct Node {
    pub shape: Shape,
    pub parent: Option<NodeId>,
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    /// Named properties that scenes can read, set and animate.
    pub properties: Vec<(&'static str, PropertyId)>,
}

impl Node {
    pub fn property(&self, name: &str) -> Option<PropertyId> {
        self.properties
            .iter()
            .find(|(property_name, _)| *property_name == name)
            .map(|(_, id)| *id)
    }

    fn required(&self, name: &str) -> PropertyId {
        self.property(name)
            .expect("every node has transform properties")
    }
}

/// One shape to draw, in pixel space.
#[derive(Clone, Debug)]
pub struct DrawItem {
    pub path: BezPath,
    pub transform: Affine,
    pub fill: Option<Color>,
    pub stroke: Option<(Color, f64)>,
}

#[derive(Debug, Default)]
pub struct Scene {
    pub properties: Properties,
    pub nodes: Vec<Node>,
    /// Marker names of the `at` blocks, in the order the scene declares them.
    pub blocks: Vec<String>,
    block_times: Vec<BlockTime>,
}

impl Scene {
    pub fn add_group(&mut self) -> NodeId {
        self.add_node(Shape::Group, vec![])
    }

    pub fn add_rect(&mut self, width: f64, height: f64, radius: f64) -> NodeId {
        let width = self.properties.add(width);
        let height = self.properties.add(height);
        let radius = self.properties.add(radius);
        self.add_node(
            Shape::Rect {
                width,
                height,
                radius,
            },
            vec![("w", width), ("h", height), ("radius", radius)],
        )
    }

    pub fn add_circle(&mut self, radius: f64) -> NodeId {
        let radius = self.properties.add(radius);
        self.add_node(Shape::Circle { radius }, vec![("r", radius)])
    }

    pub fn add_line(&mut self, from: Point, to: Point) -> NodeId {
        self.add_node(Shape::Line { from, to }, vec![])
    }

    pub fn add_path(&mut self, points: Vec<Point>, closed: bool) -> NodeId {
        self.add_node(Shape::Path { points, closed }, vec![])
    }

    pub fn add_outline(&mut self, path: BezPath) -> NodeId {
        self.add_node(Shape::Outline { path }, vec![])
    }

    fn add_node(
        &mut self,
        shape: Shape,
        mut properties: Vec<(&'static str, PropertyId)>,
    ) -> NodeId {
        for (name, base) in [
            ("x", 0.0),
            ("y", 0.0),
            ("scale", 1.0),
            ("rotation", 0.0),
            ("opacity", 1.0),
            ("stroke_width", 0.04),
        ] {
            properties.push((name, self.properties.add(base)));
        }
        self.nodes.push(Node {
            shape,
            parent: None,
            fill: None,
            stroke: None,
            properties,
        });
        self.nodes.len() - 1
    }

    pub fn set_parent(&mut self, child: NodeId, parent: NodeId) -> Result<(), String> {
        let mut ancestor = Some(parent);
        while let Some(node) = ancestor {
            if node == child {
                return Err("a node cannot be added to itself or its own children".into());
            }
            ancestor = self.nodes[node].parent;
        }
        self.nodes[child].parent = Some(parent);
        Ok(())
    }

    /// Fixes animation times. Call after the scene script has run, and again
    /// whenever markers change.
    pub fn resolve(
        &mut self,
        markers: &HashMap<String, f64>,
        default_block_seconds: f64,
        default_animation_seconds: f64,
    ) -> Result<(), String> {
        if let Some(block) = self.properties.animated_derive_block() {
            return Err(format!(
                "at \"{}\" animates a value that is set with derive; derive replaces animations, so remove one of them",
                self.blocks[block]
            ));
        }
        self.block_times = timeline::block_times(&self.blocks, markers, default_block_seconds);
        self.properties
            .resolve(&self.block_times, default_animation_seconds);
        Ok(())
    }

    /// Scene length in seconds: the end of the last block, or of the last
    /// animation if that runs longer.
    pub fn duration(&self) -> f64 {
        let last_block_end = self.block_times.last().map_or(0.0, |block| block.end);
        last_block_end.max(self.properties.last_animation_end())
    }

    /// Builds the shapes to draw for one frame. `value` returns a property's
    /// value at that frame's time. Scene units have y pointing up and the
    /// origin at the center of the frame.
    pub fn display_list<E>(
        &self,
        value: &mut dyn FnMut(PropertyId) -> Result<f64, E>,
        width: u32,
        height: u32,
        frame_height: f64,
    ) -> Result<Vec<DrawItem>, E> {
        let pixels_per_unit = f64::from(height) / frame_height;
        let to_pixels = Affine::translate((f64::from(width) / 2.0, f64::from(height) / 2.0))
            * Affine::scale_non_uniform(pixels_per_unit, -pixels_per_unit);
        // Curves are flattened to within a tenth of a pixel.
        let tolerance = 0.1 / pixels_per_unit;

        let mut world: Vec<Option<(Affine, f64)>> = vec![None; self.nodes.len()];
        let mut items = Vec::new();
        for id in 0..self.nodes.len() {
            let (transform, opacity) = self.world_transform(id, value, &mut world)?;
            let node = &self.nodes[id];
            if opacity <= 0.0 {
                continue;
            }
            let Some(path) = self.local_path(node, value, tolerance)? else {
                continue;
            };
            let stroke_width = value(node.required("stroke_width"))?;
            items.push(DrawItem {
                path,
                transform: to_pixels * transform,
                fill: node.fill.map(|color| with_opacity(color, opacity)),
                stroke: node
                    .stroke
                    .map(|color| (with_opacity(color, opacity), stroke_width)),
            });
        }
        Ok(items)
    }

    fn world_transform<E>(
        &self,
        id: NodeId,
        value: &mut dyn FnMut(PropertyId) -> Result<f64, E>,
        world: &mut Vec<Option<(Affine, f64)>>,
    ) -> Result<(Affine, f64), E> {
        if let Some(known) = world[id] {
            return Ok(known);
        }
        let node = &self.nodes[id];
        let local = Affine::translate((value(node.required("x"))?, value(node.required("y"))?))
            * Affine::rotate(value(node.required("rotation"))?)
            * Affine::scale(value(node.required("scale"))?);
        let opacity = value(node.required("opacity"))?;
        let result = match node.parent {
            Some(parent) => {
                let (parent_transform, parent_opacity) =
                    self.world_transform(parent, value, world)?;
                (parent_transform * local, parent_opacity * opacity)
            }
            None => (local, opacity),
        };
        world[id] = Some(result);
        Ok(result)
    }

    fn local_path<E>(
        &self,
        node: &Node,
        value: &mut dyn FnMut(PropertyId) -> Result<f64, E>,
        tolerance: f64,
    ) -> Result<Option<BezPath>, E> {
        let path = match &node.shape {
            Shape::Group => return Ok(None),
            Shape::Rect {
                width,
                height,
                radius,
            } => {
                let (w, h) = (value(*width)?, value(*height)?);
                RoundedRect::new(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0, value(*radius)?)
                    .to_path(tolerance)
            }
            Shape::Circle { radius } => {
                Circle::new(Point::ZERO, value(*radius)?).to_path(tolerance)
            }
            Shape::Line { from, to } => polyline(&[*from, *to], false),
            Shape::Path { points, closed } => polyline(points, *closed),
            Shape::Outline { path } => path.clone(),
        };
        Ok(Some(path))
    }
}

fn polyline(points: &[Point], closed: bool) -> BezPath {
    let mut path = BezPath::new();
    for (i, point) in points.iter().enumerate() {
        if i == 0 {
            path.move_to(*point);
        } else {
            path.line_to(*point);
        }
    }
    if closed {
        path.close_path();
    }
    path
}

fn with_opacity(color: Color, opacity: f64) -> Color {
    [color[0], color[1], color[2], color[3] * opacity as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    fn draw(scene: &Scene) -> Vec<DrawItem> {
        let mut value = |id| Ok::<f64, Infallible>(scene.properties.value_at(id, 0.0));
        scene.display_list(&mut value, 200, 100, 4.0).unwrap()
    }

    #[test]
    fn scene_origin_maps_to_frame_center_with_y_up() {
        let mut scene = Scene::default();
        let rect = scene.add_rect(1.0, 1.0, 0.0);
        scene.nodes[rect].fill = Some([1.0; 4]);
        let items = draw(&scene);
        assert_eq!(items[0].transform * Point::ZERO, Point::new(100.0, 50.0));
        assert_eq!(
            items[0].transform * Point::new(0.0, 1.0),
            Point::new(100.0, 25.0)
        );
    }

    #[test]
    fn children_inherit_position_and_opacity() {
        let mut scene = Scene::default();
        let group = scene.add_group();
        let circle = scene.add_circle(0.5);
        scene.nodes[circle].fill = Some([1.0; 4]);
        scene.set_parent(circle, group).unwrap();
        let group_x = scene.nodes[group].property("x").unwrap();
        let group_opacity = scene.nodes[group].property("opacity").unwrap();
        scene.properties.set_base(group_x, 1.0);
        scene.properties.set_base(group_opacity, 0.5);
        let items = draw(&scene);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].transform * Point::ZERO, Point::new(125.0, 50.0));
        assert_eq!(items[0].fill.unwrap()[3], 0.5);
    }

    #[test]
    fn parent_cycles_are_rejected() {
        let mut scene = Scene::default();
        let a = scene.add_group();
        let b = scene.add_group();
        scene.set_parent(b, a).unwrap();
        assert!(scene.set_parent(a, b).is_err());
    }
}
