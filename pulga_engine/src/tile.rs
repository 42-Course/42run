//! Tiles: the building blocks a run is glued together from.
//!
//! A tile is one [`TILE_SIZE`]-square cell of track, authored in the editor
//! as a set of box [`Prop`]s (floor, walls, obstacles, decoration). Every
//! tile is built in the same local frame: the runner **enters at the
//! origin, on the south edge, heading -z**, and the cell spans
//! `x ∈ [-5, 5]`, `z ∈ [-10, 0]`. Its [`TileKind`] decides where the runner
//! can leave: the exit [`Socket`]s that the next tile's entry gets glued to
//! (see [`crate::track`]).
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

use serde::{Deserialize, Serialize};

use crate::Color;
use crate::physics::{Aabb, Vec3};
use crate::render::MeshId;
use crate::scene::{Entity, EntityId, Transform, World};

/// Side length of a tile's square cell, in world units.
pub const TILE_SIZE: f32 = 10.0;
/// Width of the walkable floor the starter templates give each tile.
pub const TRACK_WIDTH: f32 = 7.0;

/// Where a tile can be left from: its exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileKind {
    /// Exit straight ahead (north).
    Straight,
    /// Exit on the left (west); the run turns +90°.
    Left,
    /// Exit on the right (east); the run turns -90°.
    Right,
    /// Exits both left and right; the runner picks.
    Fork,
}

impl TileKind {
    /// Every kind, in editor order.
    pub const ALL: [TileKind; 4] = [Self::Straight, Self::Left, Self::Right, Self::Fork];

    /// The exits a tile of this kind offers, in its local frame.
    pub fn exits(self) -> Vec<Socket> {
        let h = TILE_SIZE / 2.0;
        let north = Socket {
            position: [0.0, 0.0, -TILE_SIZE],
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

    /// Lowercase display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Straight => "straight",
            Self::Left => "left",
            Self::Right => "right",
            Self::Fork => "fork",
        }
    }

    /// The next kind in [`TileKind::ALL`], wrapping around.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&k| k == self).unwrap();
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// A tile exit: where the next tile's entry (its local origin) goes, and
/// how far the run turns there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Socket {
    /// Position on the tile's edge, in the tile's local frame.
    pub position: Vec3,
    /// Turn in quarter turns around +y: `1` = left (+90°), `-1` = right.
    pub turn: i32,
}

/// What a prop means to the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropKind {
    /// Floor, walls, decoration: drawn, never ends the run.
    #[default]
    Scenery,
    /// Ends the run when the runner hits it (jump over it, or dodge it).
    Obstacle,
}

/// A box inside a tile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    /// Scenery or obstacle.
    #[serde(default)]
    pub kind: PropKind,
    /// Center, in the tile's local frame.
    pub position: Vec3,
    /// Size along x, y, z.
    pub size: Vec3,
    /// RGB, 0..1.
    pub color: [f32; 3],
}

impl Prop {
    /// The prop's bounds in the tile's local frame.
    pub fn aabb(&self) -> Aabb {
        Aabb::from_center_size(self.position, self.size)
    }
}

/// One authored building block. See the [module docs](self).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// Unique, human-readable name.
    pub name: String,
    /// Which exits the tile has.
    pub kind: TileKind,
    /// The boxes it is made of.
    #[serde(default)]
    pub props: Vec<Prop>,
}

const FLOOR_COLOR: [f32; 3] = [0.55, 0.55, 0.58];
const FLOOR_THICKNESS: f32 = 0.2;

fn floor(center_x: f32, center_z: f32, size_x: f32, size_z: f32) -> Prop {
    Prop {
        kind: PropKind::Scenery,
        position: [center_x, -FLOOR_THICKNESS / 2.0, center_z],
        size: [size_x, FLOOR_THICKNESS, size_z],
        color: FLOOR_COLOR,
    }
}

impl Tile {
    /// A tile of `kind` with a starter floor (top at y = 0) shaped to
    /// connect its entry to its exits.
    pub fn new(name: impl Into<String>, kind: TileKind) -> Self {
        let (h, w) = (TILE_SIZE / 2.0, TRACK_WIDTH);
        // Entry corridor up to the far side of the crossing at the center.
        let to_center = floor(0.0, -(h + w / 2.0) / 2.0, w, h + w / 2.0);
        let arm_len = h - w / 2.0;
        let west = floor(-(h + w / 2.0) / 2.0, -h, arm_len, w);
        let east = floor((h + w / 2.0) / 2.0, -h, arm_len, w);
        let props = match kind {
            TileKind::Straight => vec![floor(0.0, -h, w, TILE_SIZE)],
            TileKind::Left => vec![to_center, west],
            TileKind::Right => vec![to_center, east],
            TileKind::Fork => vec![to_center, west, east],
        };
        Self {
            name: name.into(),
            kind,
            props,
        }
    }
}

/// Spawns `tile` into `world` as one root entity placed by `transform`, with
/// one child per prop drawn with `cube` (a unit cube mesh, e.g.
/// [`crate::render::Renderer::cube`]). Despawn the returned root to remove
/// the whole tile.
pub fn spawn_tile(world: &mut World, cube: MeshId, tile: &Tile, transform: Transform) -> EntityId {
    let root = world.spawn(Entity::default().with_transform(transform));
    for prop in &tile.props {
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

    /// Whether `p` is on the walkable top of one of the tile's floor props.
    fn on_floor(tile: &Tile, p: Vec3) -> bool {
        tile.props.iter().any(|prop| {
            let b = prop.aabb();
            b.max[1] == 0.0
                && (b.min[0]..=b.max[0]).contains(&p[0])
                && (b.min[2]..=b.max[2]).contains(&p[2])
        })
    }

    #[test]
    fn starter_floor_reaches_entry_and_every_exit() {
        for kind in TileKind::ALL {
            let tile = Tile::new("t", kind);
            assert!(on_floor(&tile, [0.0, 0.0, 0.0]), "{kind:?} entry");
            for exit in kind.exits() {
                assert!(on_floor(&tile, exit.position), "{kind:?} exit {exit:?}");
            }
        }
    }

    #[test]
    fn starter_floor_stays_inside_the_cell() {
        let h = TILE_SIZE / 2.0;
        for kind in TileKind::ALL {
            for prop in Tile::new("t", kind).props {
                let b = prop.aabb();
                assert!(b.min[0] >= -h && b.max[0] <= h, "{kind:?} {b:?}");
                assert!(b.min[2] >= -TILE_SIZE && b.max[2] <= 0.0, "{kind:?} {b:?}");
            }
        }
    }

    #[test]
    fn next_kind_cycles_through_all() {
        let mut k = TileKind::Straight;
        for _ in 0..TileKind::ALL.len() {
            k = k.next();
        }
        assert_eq!(k, TileKind::Straight);
    }

    #[test]
    fn spawned_tile_despawns_as_a_unit() {
        let mut world = World::default();
        let tile = Tile::new("t", TileKind::Fork);
        let root = spawn_tile(&mut world, MeshId::test_id(), &tile, Transform::default());
        assert_eq!(world.len(), 1 + tile.props.len());
        world.despawn(root);
        assert!(world.is_empty());
    }
}
