//! The scene: a [`World`] of [`Entity`]s, each placed by a [`Transform`]
//! relative to an optional parent entity (a transform hierarchy).
//!
//! Moving a parent moves everything attached to it: the world matrix of an
//! entity is its parent's world matrix times its own local
//! [`Transform::matrix`].
//!
//! ```
//! use pulga_engine::scene::{Entity, Transform, World};
//!
//! let mut world = World::default();
//! let cart = world.spawn(Entity::default().with_transform(Transform::at([10.0, 0.0, 0.0])));
//! let rider = world.spawn(
//!     Entity::default()
//!         .with_transform(Transform::at([0.0, 1.0, 0.0]))
//!         .with_parent(cart),
//! );
//!
//! // The rider sits 1 unit above the cart, wherever the cart goes.
//! let m = world.world_matrix(rider).unwrap();
//! assert_eq!((m.data[0][3], m.data[1][3]), (10.0, 1.0));
//!
//! // Despawning the cart takes the rider with it.
//! world.despawn(cart);
//! assert!(world.get(rider).is_none());
//! ```

use pulga_gui_lib::Color;
use pulga_win_lib::transform::{self, Mat4};

use crate::render::MeshId;

/// Position, rotation and scale of an entity, relative to its parent (or to
/// the world origin when it has none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Translation, in world units.
    pub position: [f32; 3],
    /// Euler angles in radians around X (pitch), Y (yaw) and Z (roll),
    /// applied in yaw → pitch → roll order (see [`Transform::matrix`]).
    pub rotation: [f32; 3],
    /// Scale factor per axis.
    pub scale: [f32; 3],
}

impl Default for Transform {
    /// The identity: at the origin, unrotated, unit scale.
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

impl Transform {
    /// An unrotated, unit-scale transform at `position`.
    pub fn at(position: [f32; 3]) -> Self {
        Self {
            position,
            ..Self::default()
        }
    }

    /// The same transform with its scale replaced.
    pub fn with_scale(self, scale: [f32; 3]) -> Self {
        Self { scale, ..self }
    }

    /// The same transform with its rotation (radians) replaced.
    pub fn with_rotation(self, rotation: [f32; 3]) -> Self {
        Self { rotation, ..self }
    }

    /// The local matrix `T · Ry · Rx · Rz · S`: scale first, then roll,
    /// pitch, yaw, and finally translate.
    pub fn matrix(&self) -> Mat4 {
        let [rx, ry, rz] = self.rotation;
        transform::translate(self.position)
            .mul_mat(&transform::rotate_axis_angle([0.0, 1.0, 0.0], ry))
            .mul_mat(&transform::rotate_axis_angle([1.0, 0.0, 0.0], rx))
            .mul_mat(&transform::rotate_axis_angle([0.0, 0.0, 1.0], rz))
            .mul_mat(&transform::scale(self.scale))
    }
}

/// Handle to an entity in a [`World`].
///
/// Carries a generation, so a handle kept after its entity was despawned
/// never silently points at whatever entity reuses the slot: lookups with it
/// just return `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId {
    index: u32,
    generation: u32,
}

/// Something in the scene: where it is, what it hangs off, and what to draw.
#[derive(Debug, Clone)]
pub struct Entity {
    /// Placement relative to [`Entity::parent`] (or the world origin).
    pub transform: Transform,
    /// The entity this one is attached to, if any. Must not form a cycle.
    pub parent: Option<EntityId>,
    /// Mesh to draw, from [`crate::render::Renderer::add_mesh`]. `None`
    /// makes a pure grouping/pivot node.
    pub mesh: Option<MeshId>,
    /// Base color the mesh is shaded with.
    pub color: Color,
    /// Hidden entities (and only they, not their children) are skipped when
    /// drawing.
    pub visible: bool,
}

impl Default for Entity {
    /// Visible, white, no mesh, no parent, identity transform.
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            parent: None,
            mesh: None,
            color: Color::rgb(1.0, 1.0, 1.0),
            visible: true,
        }
    }
}

impl Entity {
    /// A default entity that draws `mesh`.
    pub fn with_mesh(mesh: MeshId) -> Self {
        Self {
            mesh: Some(mesh),
            ..Self::default()
        }
    }

    /// The same entity with its transform replaced.
    pub fn with_transform(self, transform: Transform) -> Self {
        Self { transform, ..self }
    }

    /// The same entity with its color replaced.
    pub fn with_color(self, color: Color) -> Self {
        Self { color, ..self }
    }

    /// The same entity, attached to `parent`.
    pub fn with_parent(self, parent: EntityId) -> Self {
        Self {
            parent: Some(parent),
            ..self
        }
    }
}

struct Slot {
    generation: u32,
    entity: Option<Entity>,
}

/// Owns every [`Entity`] in the scene. Despawned slots are reused, with a
/// bumped generation so stale [`EntityId`]s stay invalid.
#[derive(Default)]
pub struct World {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

impl World {
    /// Adds `entity` and returns its handle.
    pub fn spawn(&mut self, entity: Entity) -> EntityId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.entity = Some(entity);
            return EntityId {
                index,
                generation: slot.generation,
            };
        }
        self.slots.push(Slot {
            generation: 0,
            entity: Some(entity),
        });
        EntityId {
            index: (self.slots.len() - 1) as u32,
            generation: 0,
        }
    }

    /// Removes the entity and, recursively, everything attached to it.
    /// Returns the removed entity, or `None` if `id` was already stale.
    pub fn despawn(&mut self, id: EntityId) -> Option<Entity> {
        self.get(id)?;
        // ponytail: O(n) child scan per despawned entity, keep a children
        // list per entity if worlds grow to many thousands of entities.
        let children: Vec<EntityId> = self
            .iter()
            .filter(|(_, e)| e.parent == Some(id))
            .map(|(child, _)| child)
            .collect();
        for child in children {
            self.despawn(child);
        }
        let slot = &mut self.slots[id.index as usize];
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index);
        slot.entity.take()
    }

    /// The entity behind `id`, unless it was despawned.
    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.slots
            .get(id.index as usize)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.entity.as_ref())
    }

    /// Mutable access to the entity behind `id`, unless it was despawned.
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.slots
            .get_mut(id.index as usize)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.entity.as_mut())
    }

    /// Number of live entities.
    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    /// Whether the world has no live entities.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every live entity with its handle, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (EntityId, &Entity)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            let entity = slot.entity.as_ref()?;
            let id = EntityId {
                index: index as u32,
                generation: slot.generation,
            };
            Some((id, entity))
        })
    }

    /// The entity's world matrix: its local matrix pre-multiplied by every
    /// ancestor's, root first. `None` if `id` is stale.
    ///
    /// # Panics
    /// If the parent chain loops back on itself.
    pub fn world_matrix(&self, id: EntityId) -> Option<Mat4> {
        let entity = self.get(id)?;
        let mut matrix = entity.transform.matrix();
        let mut parent = entity.parent.and_then(|p| self.get(p));
        let mut hops = 0;
        while let Some(p) = parent {
            matrix = p.transform.matrix().mul_mat(&matrix);
            parent = p.parent.and_then(|pp| self.get(pp));
            hops += 1;
            assert!(hops <= self.slots.len(), "entity parent chain has a cycle");
        }
        Some(matrix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulgamecanica_matrix::pulgamecanica::Vector;

    fn apply(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
        let r = m.mul_vec(&Vector::new(vec![p[0], p[1], p[2], 1.0]));
        [r.data[0], r.data[1], r.data[2]]
    }

    fn assert_close(a: [f32; 3], b: [f32; 3]) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-5, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn default_transform_is_identity() {
        assert_close(
            apply(&Transform::default().matrix(), [1.0, 2.0, 3.0]),
            [1.0, 2.0, 3.0],
        );
    }

    #[test]
    fn transform_scales_then_rotates_then_translates() {
        let t = Transform::at([0.0, 0.0, -5.0])
            .with_rotation([0.0, std::f32::consts::FRAC_PI_2, 0.0])
            .with_scale([2.0, 2.0, 2.0]);
        // +x scaled to 2, yawed 90° onto -z, then pushed 5 further down -z.
        assert_close(apply(&t.matrix(), [1.0, 0.0, 0.0]), [0.0, 0.0, -7.0]);
    }

    #[test]
    fn spawn_then_get() {
        let mut world = World::default();
        let id = world.spawn(Entity::default().with_transform(Transform::at([1.0, 0.0, 0.0])));
        assert_eq!(world.get(id).unwrap().transform.position, [1.0, 0.0, 0.0]);
        assert_eq!(world.len(), 1);
    }

    #[test]
    fn despawned_id_goes_stale_even_after_slot_reuse() {
        let mut world = World::default();
        let old = world.spawn(Entity::default());
        world.despawn(old);
        let new = world.spawn(Entity::default());
        assert!(world.get(old).is_none());
        assert!(world.get(new).is_some());
        assert!(world.despawn(old).is_none());
        assert_eq!(world.len(), 1);
    }

    #[test]
    fn despawn_takes_descendants_along() {
        let mut world = World::default();
        let root = world.spawn(Entity::default());
        let child = world.spawn(Entity::default().with_parent(root));
        let grandchild = world.spawn(Entity::default().with_parent(child));
        let other = world.spawn(Entity::default());
        world.despawn(root);
        assert!(world.get(child).is_none());
        assert!(world.get(grandchild).is_none());
        assert!(world.get(other).is_some());
        assert_eq!(world.len(), 1);
    }

    #[test]
    fn child_follows_parent_rotation_and_translation() {
        let mut world = World::default();
        let parent = world.spawn(Entity::default().with_transform(
            Transform::at([10.0, 0.0, 0.0]).with_rotation([0.0, std::f32::consts::FRAC_PI_2, 0.0]),
        ));
        let child = world.spawn(
            Entity::default()
                .with_transform(Transform::at([1.0, 0.0, 0.0]))
                .with_parent(parent),
        );
        // The child's local +x offset is yawed onto -z by the parent.
        let m = world.world_matrix(child).unwrap();
        assert_close(apply(&m, [0.0, 0.0, 0.0]), [10.0, 0.0, -1.0]);
    }

    #[test]
    #[should_panic(expected = "cycle")]
    fn parent_cycle_panics_instead_of_hanging() {
        let mut world = World::default();
        let a = world.spawn(Entity::default());
        let b = world.spawn(Entity::default().with_parent(a));
        world.get_mut(a).unwrap().parent = Some(b);
        world.world_matrix(a);
    }
}
