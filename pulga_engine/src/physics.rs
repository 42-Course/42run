//! Collision shapes and ray casts: what the editor uses to pick things under
//! the mouse, and games use to detect hits.
//!
//! Everything is axis-aligned for now. That is enough for a track made of
//! boxes on a grid. Rotated shapes come when something needs them.
//!
//! ```
//! use pulga_engine::physics::{Aabb, ray_plane_y};
//!
//! let crate_box = Aabb::from_center_size([0.0, 0.5, -5.0], [1.0, 1.0, 1.0]);
//! // Looking straight down -z from the origin, 0.5 up: hits the near face at z = -4.5.
//! let t = crate_box.ray_hit([0.0, 0.5, 0.0], [0.0, 0.0, -1.0]).unwrap();
//! assert_eq!(t, 4.5);
//!
//! // Looking down at 45°, the ray meets the floor (y = 0) 2 units ahead.
//! let s = std::f32::consts::FRAC_1_SQRT_2;
//! let t = ray_plane_y([0.0, 2.0, 0.0], [0.0, -s, -s], 0.0).unwrap();
//! assert!((t * s - 2.0).abs() < 1e-5);
//! ```

/// A point or direction in 3D.
pub type Vec3 = [f32; 3];

/// An axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Corner with the smallest coordinates.
    pub min: Vec3,
    /// Corner with the largest coordinates.
    pub max: Vec3,
}

impl Aabb {
    /// The box centered on `center`, `size` wide along each axis.
    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        let half = size.map(|s| s.abs() / 2.0);
        Self {
            min: [0, 1, 2].map(|i| center[i] - half[i]),
            max: [0, 1, 2].map(|i| center[i] + half[i]),
        }
    }

    /// Whether the two boxes overlap. Boxes that only touch don't count.
    pub fn intersects(&self, other: &Aabb) -> bool {
        (0..3).all(|i| self.min[i] < other.max[i] && other.min[i] < self.max[i])
    }

    /// Distance along the ray (`dir` need not be unit length; `t` is in
    /// multiples of it) to where it enters the box, `0.0` if it starts
    /// inside, `None` if it misses or the box is behind it.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3) -> Option<f32> {
        // Slab method: intersect the ray's entry/exit intervals per axis.
        let (mut t_enter, mut t_exit) = (0.0f32, f32::INFINITY);
        for i in 0..3 {
            if dir[i] == 0.0 {
                if origin[i] < self.min[i] || origin[i] > self.max[i] {
                    return None;
                }
                continue;
            }
            let (a, b) = (
                (self.min[i] - origin[i]) / dir[i],
                (self.max[i] - origin[i]) / dir[i],
            );
            t_enter = t_enter.max(a.min(b));
            t_exit = t_exit.min(a.max(b));
        }
        (t_enter <= t_exit).then_some(t_enter)
    }
}

/// Distance along the ray to the horizontal plane at height `y`, or `None`
/// if the ray is parallel to it or points away.
pub fn ray_plane_y(origin: Vec3, dir: Vec3, y: f32) -> Option<f32> {
    if dir[1] == 0.0 {
        return None;
    }
    let t = (y - origin[1]) / dir[1];
    (t >= 0.0).then_some(t)
}

/// The point `t` steps along the ray.
pub fn point_at(origin: Vec3, dir: Vec3, t: f32) -> Vec3 {
    [0, 1, 2].map(|i| origin[i] + dir[i] * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_box() -> Aabb {
        Aabb::from_center_size([0.0; 3], [2.0; 3])
    }

    #[test]
    fn overlapping_boxes_intersect_touching_ones_dont() {
        let a = unit_box();
        assert!(a.intersects(&Aabb::from_center_size([1.5, 0.0, 0.0], [2.0; 3])));
        assert!(!a.intersects(&Aabb::from_center_size([2.0, 0.0, 0.0], [2.0; 3])));
    }

    #[test]
    fn ray_hits_front_face_misses_beside_and_behind() {
        let b = unit_box();
        assert_eq!(b.ray_hit([0.0, 0.0, 5.0], [0.0, 0.0, -1.0]), Some(4.0));
        assert_eq!(b.ray_hit([3.0, 0.0, 5.0], [0.0, 0.0, -1.0]), None);
        assert_eq!(b.ray_hit([0.0, 0.0, 5.0], [0.0, 0.0, 1.0]), None);
    }

    #[test]
    fn ray_starting_inside_hits_at_zero() {
        assert_eq!(unit_box().ray_hit([0.0; 3], [1.0, 0.0, 0.0]), Some(0.0));
    }

    #[test]
    fn diagonal_ray_enters_at_the_corner_edge() {
        let t = unit_box()
            .ray_hit([3.0, 3.0, 0.0], [-1.0, -1.0, 0.0])
            .unwrap();
        assert_eq!(
            point_at([3.0, 3.0, 0.0], [-1.0, -1.0, 0.0], t),
            [1.0, 1.0, 0.0]
        );
    }

    #[test]
    fn plane_hit_only_in_front() {
        assert_eq!(
            ray_plane_y([0.0, 2.0, 0.0], [0.0, -1.0, 0.0], 0.0),
            Some(2.0)
        );
        assert_eq!(ray_plane_y([0.0, 2.0, 0.0], [0.0, 1.0, 0.0], 0.0), None);
        assert_eq!(ray_plane_y([0.0, 2.0, 0.0], [1.0, 0.0, 0.0], 0.0), None);
    }
}
