//! Gluing tiles into a run: each tile's entry goes onto an exit
//! [`crate::tile::Socket`] of the one before it.
//!
//! The heading is kept within one quarter turn of the start (never facing
//! back toward where the run began). Every turn then moves the run forward
//! a row or sideways along a row it never comes back to, so the track can't
//! cross itself, however long it gets.
//!
//! ```
//! use pulga_engine::tile::{Tile, TileKind};
//! use pulga_engine::track::generate;
//!
//! let tiles = [Tile::new("a", TileKind::Straight), Tile::new("b", TileKind::Left)];
//! let mut n = 0;
//! let run = generate(&tiles, 10, |_| true, |len| { n += 1; n % len });
//! assert_eq!(run.len(), 10);
//! ```

use std::f32::consts::FRAC_PI_2;

use crate::physics::Vec3;
use crate::scene::Transform;
use crate::tile::Tile;

/// Where one tile of a run sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Index into the tile list given to [`generate`].
    pub tile: usize,
    /// World position of the tile's local origin (its entry).
    pub origin: Vec3,
    /// Heading, in quarter turns around +y from the run's start (-1..=1).
    pub heading: i32,
}

/// `v` turned by `quarters` × 90° around +y, exactly (no float drift).
pub fn rotate_quarters(v: Vec3, quarters: i32) -> Vec3 {
    let [x, y, z] = v;
    match quarters.rem_euclid(4) {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    }
}

impl Placement {
    /// A tile-local point in world space.
    pub fn to_world(&self, local: Vec3) -> Vec3 {
        let r = rotate_quarters(local, self.heading);
        [0, 1, 2].map(|i| self.origin[i] + r[i])
    }

    /// The transform for the tile's root entity.
    pub fn transform(&self) -> Transform {
        Transform::at(self.origin).with_rotation([0.0, self.heading as f32 * FRAC_PI_2, 0.0])
    }
}

/// Picks up to `count` tiles (only those `allowed` accepts) and glues them
/// end to end from the origin, heading -z. `pick(len)` must return an index
/// below `len`; it chooses between candidate tiles and between a fork's
/// exits, so pass your RNG. Stops early if no allowed tile fits.
pub fn generate(
    tiles: &[Tile],
    count: usize,
    allowed: impl Fn(&Tile) -> bool,
    mut pick: impl FnMut(usize) -> usize,
) -> Vec<Placement> {
    let fits = |tile: &Tile, heading: i32| {
        tile.kind
            .exits()
            .iter()
            .any(|e| (heading + e.turn).abs() <= 1)
    };
    let mut run = Vec::with_capacity(count);
    let (mut origin, mut heading) = ([0.0; 3], 0);
    for _ in 0..count {
        let candidates: Vec<usize> = (0..tiles.len())
            .filter(|&i| allowed(&tiles[i]) && fits(&tiles[i], heading))
            .collect();
        if candidates.is_empty() {
            break;
        }
        let tile = candidates[pick(candidates.len())];
        let placement = Placement {
            tile,
            origin,
            heading,
        };
        run.push(placement);

        let exits: Vec<_> = tiles[tile]
            .kind
            .exits()
            .into_iter()
            .filter(|e| (heading + e.turn).abs() <= 1)
            .collect();
        let exit = exits[pick(exits.len())];
        origin = placement.to_world(exit.position);
        heading += exit.turn;
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{TILE_SIZE, TileKind};

    fn all_kinds() -> Vec<Tile> {
        TileKind::ALL
            .iter()
            .map(|&k| Tile::new(k.name(), k))
            .collect()
    }

    /// Deterministic pseudo-random picks.
    fn lcg(seed: u64) -> impl FnMut(usize) -> usize {
        let mut s = seed;
        move |len| {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 33) as usize) % len
        }
    }

    #[test]
    fn quarter_turns_match_the_exit_layout() {
        // Heading -z, a left turn faces -x.
        assert_eq!(rotate_quarters([0.0, 0.0, -1.0], 1), [-1.0, 0.0, 0.0]);
        assert_eq!(rotate_quarters([0.0, 0.0, -1.0], -1), [1.0, 0.0, 0.0]);
        assert_eq!(rotate_quarters([1.0, 2.0, 3.0], 4), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn straight_tiles_line_up_down_minus_z() {
        let tiles = [Tile::new("s", TileKind::Straight)];
        let run = generate(&tiles, 3, |_| true, |_| 0);
        let z: Vec<f32> = run.iter().map(|p| p.origin[2]).collect();
        assert_eq!(z, [0.0, -TILE_SIZE, -2.0 * TILE_SIZE]);
    }

    #[test]
    fn long_runs_never_overlap_or_turn_back() {
        let tiles = all_kinds();
        for seed in 0..20 {
            let run = generate(&tiles, 300, |_| true, lcg(seed));
            assert_eq!(run.len(), 300);
            let mut cells = std::collections::HashSet::new();
            for p in &run {
                assert!(p.heading.abs() <= 1);
                let c = p.to_world([0.0, 0.0, -TILE_SIZE / 2.0]);
                let cell = (
                    (c[0] / TILE_SIZE).round() as i32,
                    (c[2] / TILE_SIZE).round() as i32,
                );
                assert!(cells.insert(cell), "seed {seed}: cell {cell:?} used twice");
            }
        }
    }

    #[test]
    fn filter_limits_tiles_and_empty_choice_stops_early() {
        let tiles = all_kinds();
        let run = generate(&tiles, 50, |t| t.kind == TileKind::Straight, lcg(1));
        assert!(run.iter().all(|p| tiles[p.tile].kind == TileKind::Straight));
        // Only right turns: after one, a second would face backwards.
        let rights = generate(&tiles, 5, |t| t.kind == TileKind::Right, lcg(1));
        assert_eq!(rights.len(), 1);
    }
}
