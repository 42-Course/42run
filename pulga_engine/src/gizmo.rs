//! The move gizmo: three arrows with sphere tips, one per axis, drawn on top
//! of the scene at a selected object, to drag it along one axis.
//!
//! The math (picking a handle with the mouse ray, turning mouse movement
//! into a distance along the axis) is plain and tested; [`Gizmo::spawn`]
//! builds the visible arrows as overlay entities.
//!
//! ```
//! use pulga_engine::gizmo::{Axis, Gizmo};
//!
//! let g = Gizmo { origin: [0.0; 3], length: 2.0 };
//! // A ray straight down onto the X tip picks the X handle.
//! assert_eq!(g.pick([2.0, 5.0, 0.0], [0.0, -1.0, 0.0]), Some(Axis::X));
//! // A ray straight down at x = 3.5 points 3.5 along X.
//! assert_eq!(g.axis_param(Axis::X, [3.5, 5.0, 0.0], [0.0, -1.0, 0.0]), Some(3.5));
//! ```

use crate::Color;
use crate::physics::Vec3;
use crate::render::MeshId;
use crate::scene::{Entity, EntityId, Transform, World};

/// One of the three world axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Red, pointing right.
    X = 0,
    /// Green, pointing up.
    Y = 1,
    /// Blue, pointing toward the viewer at the start (south).
    Z = 2,
}

impl Axis {
    /// All three.
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    /// Unit vector along the axis.
    pub fn unit(self) -> Vec3 {
        let mut v = [0.0; 3];
        v[self as usize] = 1.0;
        v
    }

    /// The axis' handle color.
    pub fn color(self) -> Color {
        match self {
            Axis::X => Color::hex(0xe05555),
            Axis::Y => Color::hex(0x5cbf5c),
            Axis::Z => Color::hex(0x4f8ff0),
        }
    }
}

/// See the [module docs](self).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gizmo {
    /// Where the arrows start (the selected object's center).
    pub origin: Vec3,
    /// Arrow length, in world units.
    pub length: f32,
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Distance along the ray to the sphere, if it hits.
fn ray_sphere(o: Vec3, d: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = sub(o, center);
    let (a, b, c) = (dot(d, d), dot(oc, d), dot(oc, oc) - radius * radius);
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / a;
    (t >= 0.0).then_some(t)
}

/// Parameters `(s, t)` of the closest points between lines `p + s·u` and
/// `q + t·v`, or `None` if they're parallel.
fn closest_params(p: Vec3, u: Vec3, q: Vec3, v: Vec3) -> Option<(f32, f32)> {
    let w = sub(p, q);
    let (a, b, c, d, e) = (dot(u, u), dot(u, v), dot(v, v), dot(u, w), dot(v, w));
    let denom = a * c - b * b;
    if denom.abs() < 1e-8 {
        return None;
    }
    Some(((b * e - c * d) / denom, (a * e - b * d) / denom))
}

impl Gizmo {
    /// A gizmo at `origin` sized to look the same on screen wherever the
    /// camera at `eye` is.
    pub fn for_view(origin: Vec3, eye: Vec3) -> Self {
        let d = sub(origin, eye);
        Self {
            origin,
            length: (dot(d, d).sqrt() * 0.16).clamp(0.4, 12.0),
        }
    }

    /// The tip of an axis' arrow.
    pub fn tip(&self, axis: Axis) -> Vec3 {
        let u = axis.unit();
        [0, 1, 2].map(|i| self.origin[i] + u[i] * self.length)
    }

    /// Radius of the sphere tips.
    pub fn tip_radius(&self) -> f32 {
        self.length * 0.11
    }

    /// The handle under the ray (`origin`, `dir`), nearest first: a tip
    /// sphere, or close enough to an arrow's shaft.
    pub fn pick(&self, origin: Vec3, dir: Vec3) -> Option<Axis> {
        let grab = self.tip_radius() * 1.5;
        Axis::ALL
            .iter()
            .filter_map(|&axis| {
                let on_tip = ray_sphere(origin, dir, self.tip(axis), grab);
                let on_shaft =
                    closest_params(self.origin, axis.unit(), origin, dir).and_then(|(s, t)| {
                        let a = [0, 1, 2].map(|i| self.origin[i] + axis.unit()[i] * s);
                        let b = [0, 1, 2].map(|i| origin[i] + dir[i] * t);
                        let gap = sub(a, b);
                        let close = dot(gap, gap).sqrt() < grab * 0.6;
                        (close && (0.15 * self.length..=self.length).contains(&s) && t >= 0.0)
                            .then_some(t)
                    });
                on_tip.or(on_shaft).map(|t| (axis, t))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(axis, _)| axis)
    }

    /// How far along `axis` (from the gizmo origin) the ray points: the
    /// parameter of the axis point closest to the ray. Differences of this
    /// between frames are how far a dragged handle moved. `None` when the
    /// ray runs parallel to the axis.
    pub fn axis_param(&self, axis: Axis, origin: Vec3, dir: Vec3) -> Option<f32> {
        closest_params(self.origin, axis.unit(), origin, dir).map(|(s, _)| s)
    }

    /// Spawns the arrows (drawn on top of everything) as one root entity.
    /// `hot` (hovered or dragged) is highlighted. `cube` and `sphere` are
    /// unit meshes (see [`crate::render::Renderer`]).
    pub fn spawn(
        &self,
        world: &mut World,
        cube: MeshId,
        sphere: MeshId,
        hot: Option<Axis>,
    ) -> EntityId {
        let root = world.spawn(
            Entity::default()
                .with_transform(Transform::at(self.origin))
                .on_top(),
        );
        let thick = self.length * 0.035;
        for axis in Axis::ALL {
            let u = axis.unit();
            let color = if hot == Some(axis) {
                Color::hex(0xffd84a)
            } else {
                axis.color()
            };
            let mut size = [thick; 3];
            size[axis as usize] = self.length;
            let shaft_center = u.map(|c| c * self.length / 2.0);
            let tip = u.map(|c| c * self.length);
            for (mesh, center, scale) in [
                (cube, shaft_center, size),
                (sphere, tip, [self.tip_radius() * 2.0; 3]),
            ] {
                world.spawn(
                    Entity::with_mesh(mesh)
                        .with_parent(root)
                        .with_color(color)
                        .with_transform(Transform::at(center).with_scale(scale))
                        .on_top(),
                );
            }
        }
        world.spawn(
            Entity::with_mesh(cube)
                .with_parent(root)
                .with_color(Color::hex(0xf2f2f2))
                .with_transform(Transform::default().with_scale([thick * 3.0; 3]))
                .on_top(),
        );
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Gizmo = Gizmo {
        origin: [1.0, 1.0, 1.0],
        length: 2.0,
    };

    #[test]
    fn rays_pick_tips_and_shafts_nearest_first() {
        assert_eq!(G.pick([1.0, 10.0, 1.0], [0.0, -1.0, 0.0]), Some(Axis::Y));
        // From the front, aiming at the middle of the X shaft.
        assert_eq!(G.pick([2.0, 1.0, 10.0], [0.0, 0.0, -1.0]), Some(Axis::X));
        assert_eq!(G.pick([10.0, 10.0, 10.0], [0.0, 0.0, -1.0]), None);
    }

    #[test]
    fn axis_param_follows_the_mouse_ray() {
        // Rays from above, at x = 2.5 and x = 4: the handle moves 1.5 along X.
        let a = G
            .axis_param(Axis::X, [2.5, 9.0, 1.0], [0.0, -1.0, 0.0])
            .unwrap();
        let b = G
            .axis_param(Axis::X, [4.0, 9.0, 1.0], [0.0, -1.0, 0.0])
            .unwrap();
        assert!((b - a - 1.5).abs() < 1e-5);
        // Looking along the axis gives nothing to drag by.
        assert!(
            G.axis_param(Axis::X, [9.0, 1.0, 1.0], [-1.0, 0.0, 0.0])
                .is_none()
        );
    }

    #[test]
    fn size_scales_with_view_distance() {
        let near = Gizmo::for_view([0.0; 3], [0.0, 0.0, 10.0]);
        let far = Gizmo::for_view([0.0; 3], [0.0, 0.0, 40.0]);
        assert!((far.length / near.length - 4.0).abs() < 1e-5);
    }

    #[test]
    fn spawn_builds_two_parts_per_axis_plus_a_center() {
        let mut world = World::default();
        let id = MeshId::test_id();
        let root = G.spawn(&mut world, id, id, Some(Axis::Z));
        assert_eq!(world.len(), 1 + 3 * 2 + 1);
        assert!(world.iter().all(|(_, e)| e.overlay));
        world.despawn(root);
        assert!(world.is_empty());
    }
}
