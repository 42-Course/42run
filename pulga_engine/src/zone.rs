//! Zones: the designed pieces a run is glued together from.
//!
//! A zone is one [`ZONE_SIZE`]-square cell of track, designed in the editor
//! as a set of box [`Prop`]s (floor, walls, obstacles, decoration). Every
//! zone is built in the same local frame: the runner **enters at the origin,
//! on the south edge, heading -z**, and the cell spans `x ∈ [-5, 5]`,
//! `z ∈ [-10, 0]`. Its [`ZoneKind`] decides where the runner can leave: the
//! exit [`Socket`]s the next zone's entry gets glued to (see
//! [`crate::track`]).
//!
//! ```text
//!          z = -10
//!        +----N----+      Straight: exit N
//!        |         |      Left:     exit W (turn +90°)
//!        W    .    E      Right:    exit E (turn -90°)
//!        |         |      Fork:     exits W and E
//!        +----S----+
//!          z = 0  (entry, heading -z)
//! ```
//!
//! Props with [`Prop::solid`] set have mass: the runner stands on them,
//! crashes into their sides, and falls where there are none.

use serde::{Deserialize, Serialize};

use crate::Color;
use crate::physics::{Aabb, Vec3};
use crate::render::MeshId;
use crate::scene::{Entity, EntityId, Transform, World};

/// Side length of a zone's square cell, in world units.
pub const ZONE_SIZE: f32 = 10.0;
/// Width of the walkable floor the starter templates give each zone.
pub const TRACK_WIDTH: f32 = 7.0;

/// Where a zone can be left from: its exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneKind {
    /// Exit straight ahead (north).
    Straight,
    /// Exit on the left (west); the run turns +90°.
    Left,
    /// Exit on the right (east); the run turns -90°.
    Right,
    /// Exits both left and right; the runner picks.
    Fork,
}

impl ZoneKind {
    /// Every kind, in editor order.
    pub const ALL: [ZoneKind; 4] = [Self::Straight, Self::Left, Self::Right, Self::Fork];

    /// The exits a zone of this kind offers, in its local frame.
    pub fn exits(self) -> Vec<Socket> {
        let h = ZONE_SIZE / 2.0;
        let north = Socket {
            position: [0.0, 0.0, -ZONE_SIZE],
            turn: 0,
        };
        let west = Socket {
            position: [-h, 0.0, -h],
            turn: 1,
        };
        let east = Socket {
            position: [h, 0.0, -h],
            turn: -1,
        };
        match self {
            Self::Straight => vec![north],
            Self::Left => vec![west],
            Self::Right => vec![east],
            Self::Fork => vec![west, east],
        }
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Straight => "Straight",
            Self::Left => "Left turn",
            Self::Right => "Right turn",
            Self::Fork => "Fork",
        }
    }

    /// Short lowercase tag (list badges).
    pub fn tag(self) -> &'static str {
        match self {
            Self::Straight => "straight",
            Self::Left => "left",
            Self::Right => "right",
            Self::Fork => "fork",
        }
    }
}

/// A zone exit: where the next zone's entry (its local origin) goes, and
/// how far the run turns there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Socket {
    /// Position on the zone's edge, in the zone's local frame.
    pub position: Vec3,
    /// Turn in quarter turns around +y: `1` = left (+90°), `-1` = right.
    pub turn: i32,
}

fn solid_default() -> bool {
    true
}

/// A box inside a zone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    /// Center, in the zone's local frame.
    pub position: Vec3,
    /// Size along x, y, z.
    pub size: Vec3,
    /// RGB, 0..1.
    pub color: [f32; 3],
    /// Has mass: the runner collides with it (stands on it, crashes into
    /// it). Off for decoration the runner passes through.
    #[serde(default = "solid_default")]
    pub solid: bool,
}

impl Prop {
    /// A solid box.
    pub fn solid(position: Vec3, size: Vec3, color: [f32; 3]) -> Self {
        Self {
            position,
            size,
            color,
            solid: true,
        }
    }

    /// The prop's bounds in the zone's local frame.
    pub fn aabb(&self) -> Aabb {
        Aabb::from_center_size(self.position, self.size)
    }
}

/// One designed building block. See the [module docs](self).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    /// Stable identity (groups refer to zones by it).
    pub id: u32,
    /// Human-readable name.
    pub name: String,
    /// Which exits the zone has.
    pub kind: ZoneKind,
    /// The boxes it is made of.
    #[serde(default)]
    pub props: Vec<Prop>,
}

/// Starter floor color.
pub const FLOOR_COLOR: [f32; 3] = [0.56, 0.57, 0.6];
const FLOOR_THICKNESS: f32 = 0.2;

fn floor(center_x: f32, center_z: f32, size_x: f32, size_z: f32) -> Prop {
    Prop::solid(
        [center_x, -FLOOR_THICKNESS / 2.0, center_z],
        [size_x, FLOOR_THICKNESS, size_z],
        FLOOR_COLOR,
    )
}

impl Zone {
    /// A zone of `kind` with a starter floor (top at y = 0) shaped to
    /// connect its entry to its exits.
    pub fn new(id: u32, name: impl Into<String>, kind: ZoneKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            props: Self::starter_floor(kind),
        }
    }

    /// The floor [`Zone::new`] gives a zone of `kind`.
    pub fn starter_floor(kind: ZoneKind) -> Vec<Prop> {
        let (h, w) = (ZONE_SIZE / 2.0, TRACK_WIDTH);
        // Entry corridor up to the far side of the crossing at the center.
        let to_center = floor(0.0, -(h + w / 2.0) / 2.0, w, h + w / 2.0);
        let arm_len = h - w / 2.0;
        let west = floor(-(h + w / 2.0) / 2.0, -h, arm_len, w);
        let east = floor((h + w / 2.0) / 2.0, -h, arm_len, w);
        match kind {
            ZoneKind::Straight => vec![floor(0.0, -h, w, ZONE_SIZE)],
            ZoneKind::Left => vec![to_center, west],
            ZoneKind::Right => vec![to_center, east],
            ZoneKind::Fork => vec![to_center, west, east],
        }
    }
}

/// Spawns `zone` into `world` as one root entity placed by `transform`, with
/// one child per prop drawn with `cube` (a unit cube mesh, e.g.
/// [`crate::render::Renderer::cube`]). Despawn the returned root to remove
/// the whole zone.
pub fn spawn_zone(world: &mut World, cube: MeshId, zone: &Zone, transform: Transform) -> EntityId {
    let root = world.spawn(Entity::default().with_transform(transform));
    for prop in &zone.props {
        let [r, g, b] = prop.color;
        world.spawn(
            Entity::with_mesh(cube)
                .with_parent(root)
                .with_color(Color::rgb(r, g, b))
                .with_transform(Transform::at(prop.position).with_scale(prop.size)),
        );
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `p` is on the walkable top of one of the zone's props.
    fn on_floor(zone: &Zone, p: Vec3) -> bool {
        zone.props.iter().any(|prop| {
            let b = prop.aabb();
            prop.solid
                && b.max[1] == 0.0
                && (b.min[0]..=b.max[0]).contains(&p[0])
                && (b.min[2]..=b.max[2]).contains(&p[2])
        })
    }

    #[test]
    fn starter_floor_reaches_entry_and_every_exit() {
        for kind in ZoneKind::ALL {
            let zone = Zone::new(0, "z", kind);
            assert!(on_floor(&zone, [0.0, 0.0, 0.0]), "{kind:?} entry");
            for exit in kind.exits() {
                assert!(on_floor(&zone, exit.position), "{kind:?} exit {exit:?}");
            }
        }
    }

    #[test]
    fn turns_have_no_floor_straight_ahead() {
        // Missing the turn must drop the runner.
        for kind in [ZoneKind::Left, ZoneKind::Right, ZoneKind::Fork] {
            assert!(
                !on_floor(&Zone::new(0, "z", kind), [0.0, 0.0, -9.5]),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn starter_floor_stays_inside_the_cell() {
        let h = ZONE_SIZE / 2.0;
        for kind in ZoneKind::ALL {
            for prop in Zone::starter_floor(kind) {
                let b = prop.aabb();
                assert!(b.min[0] >= -h && b.max[0] <= h, "{kind:?} {b:?}");
                assert!(b.min[2] >= -ZONE_SIZE && b.max[2] <= 0.0, "{kind:?} {b:?}");
            }
        }
    }

    #[test]
    fn spawned_zone_despawns_as_a_unit() {
        let mut world = World::default();
        let zone = Zone::new(0, "z", ZoneKind::Fork);
        let root = spawn_zone(&mut world, MeshId::test_id(), &zone, Transform::default());
        assert_eq!(world.len(), 1 + zone.props.len());
        world.despawn(root);
        assert!(world.is_empty());
    }
}
