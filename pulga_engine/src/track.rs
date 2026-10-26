//! The track: zones glued end to end on their exit sockets, generated just
//! ahead of the runner and dropped behind it.
//!
//! The track is a tree. A fork zone gets a child on each exit; when the
//! runner enters one branch, the other is removed (and so is everything
//! behind the zone the runner left).
//!
//! Zones come from a [`Source`]: the project's free-mode pool (its roots,
//! picked at random) or one item repeated (the editor's run preview).
//! Picking a group queues all its zones (in order or shuffled, see
//! [`crate::project`]); every branch continues that queue.
//!
//! The heading is kept within one quarter turn of the start, never facing
//! back toward where the run began, and a group is only picked if every
//! path through it (forks included) respects that. Every turn then moves
//! the run forward a row, or sideways along a row it never comes back to,
//! so the track can't cross itself, however long it gets.

use std::collections::VecDeque;
use std::f32::consts::FRAC_PI_2;

use crate::physics::{Aabb, Vec3};
use crate::project::{Item, Project};
use crate::render::MeshId;
use crate::scene::{EntityId, Transform, World};
use crate::zone::{Socket, ZONE_SIZE, Zone, ZoneKind, spawn_zone};

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

/// Where a zone sits: its local origin (entry) and heading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// World position of the zone's local origin (its entry).
    pub origin: Vec3,
    /// Heading, in quarter turns around +y from the run's start (-1..=1).
    pub heading: i32,
}

impl Placement {
    /// A zone-local point in world space.
    pub fn to_world(&self, local: Vec3) -> Vec3 {
        let r = rotate_quarters(local, self.heading);
        [0, 1, 2].map(|i| self.origin[i] + r[i])
    }

    /// A world point in the zone's local frame.
    pub fn to_local(&self, world: Vec3) -> Vec3 {
        rotate_quarters([0, 1, 2].map(|i| world[i] - self.origin[i]), -self.heading)
    }

    /// A zone-local box in world space (exact, since turns are quarters).
    pub fn aabb_to_world(&self, local: &Aabb) -> Aabb {
        let center = [0, 1, 2].map(|i| (local.min[i] + local.max[i]) / 2.0);
        let mut size = [0, 1, 2].map(|i| local.max[i] - local.min[i]);
        if self.heading.rem_euclid(2) == 1 {
            size.swap(0, 2);
        }
        Aabb::from_center_size(self.to_world(center), size)
    }

    /// Whether a world point is over this zone's cell.
    pub fn covers(&self, world: Vec3) -> bool {
        let l = self.to_local(world);
        l[0].abs() <= ZONE_SIZE / 2.0 && (-ZONE_SIZE..=0.0).contains(&l[2])
    }

    /// The transform for the zone's root entity.
    pub fn transform(&self) -> Transform {
        Transform::at(self.origin).with_rotation([0.0, self.heading as f32 * FRAC_PI_2, 0.0])
    }
}

/// Where a [`Track`] gets its zones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    /// The project's roots, picked at random (free mode).
    Pool,
    /// One zone or group, over and over (previews).
    Repeat(Item),
}

/// One placed zone.
pub struct Node {
    /// The zone's id in the track's project.
    pub zone: u32,
    /// Where it is.
    pub placement: Placement,
    /// The node it was glued onto.
    pub parent: Option<usize>,
    /// One slot per exit of the zone (in [`ZoneKind::exits`] order);
    /// `None` until generated, or if nothing fits there.
    pub children: Vec<Option<usize>>,
    /// The spawned entity (despawning it removes the zone).
    pub root: EntityId,
    /// World-space bounds of the zone's solid props.
    pub solids: Vec<Aabb>,
    /// Zones queued after this one (rest of a group).
    plan: VecDeque<u32>,
    generated: bool,
}

/// A junction the runner can take: see [`Track::junction`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Junction {
    /// The zone it's in.
    pub node: usize,
    /// The exit taken.
    pub exit: Socket,
    /// The exit's position in the world (a point on the new centerline).
    pub world_exit: Vec3,
    /// Heading after turning.
    pub heading: i32,
}

/// See the [module docs](self).
pub struct Track {
    project: Project,
    source: Source,
    nodes: Vec<Option<Node>>,
    current: usize,
    rng: u64,
    cube: MeshId,
    /// How many zones ahead of the runner are kept generated.
    pub lookahead: usize,
}

/// Empty straight zones every track starts with, before the source's
/// zones: a runway to get going on.
pub const RUNWAY: usize = 2;

/// Id of the runway zone (added to the track's copy of the project).
const RUNWAY_ID: u32 = u32::MAX;

/// Zones generated down each branch of a fork before the runner commits to
/// one. Two keeps the branches apart: they leave the fork in opposite
/// directions and need at least two turns to head back toward each other.
pub const BRANCH_DEPTH: usize = 2;

/// How far before a junction's crossing square a turn is already accepted
/// (world units), so a slightly early press still turns.
pub const TURN_EARLY: f32 = 0.5;

impl Track {
    /// Starts a track at the origin heading -z, spawning its first zones
    /// into `world` (drawn with the unit `cube`): [`RUNWAY`] empty straight
    /// zones, then zones from `source`. Different seeds give different
    /// tracks. If the source yields nothing (no zones), the track ends
    /// after the runway.
    pub fn new(
        mut project: Project,
        source: Source,
        seed: u64,
        world: &mut World,
        cube: MeshId,
    ) -> Self {
        project
            .zones
            .push(Zone::new(RUNWAY_ID, "Runway", ZoneKind::Straight));
        let mut track = Self {
            project,
            source,
            nodes: Vec::new(),
            current: 0,
            rng: seed.max(1),
            cube,
            lookahead: 6,
        };
        let start = Placement {
            origin: [0.0; 3],
            heading: 0,
        };
        let plan = std::iter::repeat_n(RUNWAY_ID, RUNWAY - 1).collect();
        track.spawn_node(world, RUNWAY_ID, start, None, plan);
        track.extend(world);
        track
    }

    /// The project the zones come from.
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// The node the runner is in.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Node `i`. Panics if it was removed.
    pub fn node(&self, i: usize) -> &Node {
        self.nodes[i].as_ref().expect("node was removed")
    }

    /// The zone placed at node `i`.
    pub fn zone(&self, i: usize) -> &Zone {
        self.project
            .zone(self.node(i).zone)
            .expect("tracks only place existing zones")
    }

    /// Every live node index.
    pub fn nodes(&self) -> impl Iterator<Item = usize> + '_ {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.as_ref().map(|_| i))
    }

    /// The current node, its parent and its children: everything the
    /// runner can touch.
    fn nearby(&self) -> Vec<usize> {
        let cur = self.node(self.current);
        let mut out = vec![self.current];
        out.extend(cur.parent);
        out.extend(cur.children.iter().flatten());
        out
    }

    /// The node whose cell is under `p`, among those near the runner.
    pub fn node_at(&self, p: Vec3) -> Option<usize> {
        self.nearby()
            .into_iter()
            .find(|&i| self.node(i).placement.covers(p))
    }

    /// Solid boxes the runner at `p` can collide with.
    pub fn solids_near(&self, _p: Vec3) -> Vec<Aabb> {
        self.nearby()
            .into_iter()
            .flat_map(|i| self.node(i).solids.clone())
            .collect()
    }

    /// The junction a runner at `p` can turn into with a `turn` (+1 left,
    /// -1 right), if `p` is in a zone with such an exit, at (or just
    /// before) its crossing.
    pub fn junction(&self, p: Vec3, turn: i32) -> Option<Junction> {
        let node = self.node_at(p)?;
        let n = self.node(node);
        let exit = self
            .zone(node)
            .kind
            .exits()
            .into_iter()
            .find(|e| e.turn == turn)?;
        let local = n.placement.to_local(p);
        // The crossing square spans the track width around the exit row.
        let half = crate::zone::TRACK_WIDTH / 2.0;
        let row = exit.position[2];
        let in_window = local[2] <= row + half + TURN_EARLY && local[2] >= row - half;
        in_window.then(|| Junction {
            node,
            exit,
            world_exit: n.placement.to_world(exit.position),
            heading: n.placement.heading + turn,
        })
    }

    /// Follows the runner at `p`: when it enters a child of the current
    /// zone, that child becomes current, the other branches and the zone
    /// left behind are removed, and new zones are generated ahead.
    pub fn update(&mut self, world: &mut World, p: Vec3) {
        let cur = self.node(self.current);
        let entered = cur
            .children
            .iter()
            .flatten()
            .copied()
            .find(|&c| self.node(c).placement.covers(p));
        let Some(next) = entered else { return };
        let left = self.current;
        let siblings: Vec<usize> = self
            .node(left)
            .children
            .iter()
            .flatten()
            .copied()
            .filter(|&c| c != next)
            .collect();
        for s in siblings {
            self.remove_subtree(world, s);
        }
        if let Some(grand) = self.node(left).parent {
            self.remove_node(world, grand);
        }
        self.nodes[left].as_mut().unwrap().parent = None;
        self.current = next;
        self.extend(world);
    }

    /// Removes every zone from `world`.
    pub fn clear(self, world: &mut World) {
        for node in self.nodes.into_iter().flatten() {
            world.despawn(node.root);
        }
    }

    // ---------------------------------------------------------- generation

    fn pick(&mut self, len: usize) -> usize {
        // xorshift64: plenty for level generation.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % len.max(1) as u64) as usize
    }

    /// Whether every path through `zones` from `heading` stays within one
    /// quarter turn of the start.
    fn feasible(&self, zones: &[u32], heading: i32) -> bool {
        let mut headings = vec![heading];
        for id in zones {
            let Some(zone) = self.project.zone(*id) else {
                return false;
            };
            let mut next = Vec::new();
            for h in &headings {
                for e in zone.kind.exits() {
                    let nh = h + e.turn;
                    if nh.abs() > 1 {
                        return false;
                    }
                    if !next.contains(&nh) {
                        next.push(nh);
                    }
                }
            }
            headings = next;
        }
        true
    }

    /// The zone to place next on a branch heading `heading`: the next one
    /// queued, or the first of a freshly picked item (queueing the rest).
    fn next_zone(&mut self, plan: &mut VecDeque<u32>, heading: i32) -> Option<u32> {
        if let Some(id) = plan.pop_front() {
            return Some(id);
        }
        let items: Vec<Item> = match self.source {
            Source::Pool => self
                .project
                .roots()
                .into_iter()
                .filter(|&i| i != Item::Zone(RUNWAY_ID))
                .collect(),
            Source::Repeat(item) => vec![item],
        };
        for _ in 0..8 {
            if items.is_empty() {
                break;
            }
            let item = items[self.pick(items.len())];
            let mut rng = self.rng;
            let seq = self.project.expand(item, &mut |len| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                (rng % len as u64) as usize
            });
            self.rng = rng;
            if !seq.is_empty() && self.feasible(&seq, heading) {
                plan.extend(&seq[1..]);
                return Some(seq[0]);
            }
        }
        self.fallback_zone(heading)
    }

    /// A single zone that fits when the source can't: one turning back
    /// toward the start heading if possible, else a straight, else any.
    fn fallback_zone(&self, heading: i32) -> Option<u32> {
        let back = match heading.signum() {
            1 => Some(ZoneKind::Right),
            -1 => Some(ZoneKind::Left),
            _ => None,
        };
        // Zones that fit, never the runway.
        let fitting = || {
            self.project
                .zones
                .iter()
                .filter(|z| z.id != RUNWAY_ID && self.feasible(&[z.id], heading))
        };
        fitting()
            .find(|z| Some(z.kind) == back)
            .or_else(|| fitting().find(|z| z.kind == ZoneKind::Straight))
            .or_else(|| fitting().next())
            .map(|z| z.id)
    }

    fn spawn_node(
        &mut self,
        world: &mut World,
        zone: u32,
        placement: Placement,
        parent: Option<usize>,
        plan: VecDeque<u32>,
    ) -> usize {
        let def = self
            .project
            .zone(zone)
            .expect("tracks only place existing zones");
        let root = spawn_zone(world, self.cube, def, placement.transform());
        let solids = def
            .props
            .iter()
            .filter(|p| p.solid)
            .map(|p| placement.aabb_to_world(&p.aabb()))
            .collect();
        let node = Node {
            zone,
            placement,
            parent,
            children: vec![None; def.kind.exits().len()],
            root,
            solids,
            plan,
            generated: false,
        };
        let slot = self.nodes.iter().position(Option::is_none);
        match slot {
            Some(i) => {
                self.nodes[i] = Some(node);
                i
            }
            None => {
                self.nodes.push(Some(node));
                self.nodes.len() - 1
            }
        }
    }

    /// Generates children breadth-first from the current node, down to
    /// [`Track::lookahead`] zones ahead. Past a fork, each branch only gets
    /// [`BRANCH_DEPTH`] zones until the runner commits to one: deeper, two
    /// branches could turn toward each other and share a cell.
    fn extend(&mut self, world: &mut World) {
        let mut frontier = vec![(self.current, 0usize, self.lookahead)];
        while let Some((i, depth, limit)) = frontier.pop() {
            if depth >= limit {
                continue;
            }
            if !self.node(i).generated {
                let n = self.node(i);
                let (placement, plan) = (n.placement, n.plan.clone());
                let exits = self.zone(i).kind.exits();
                for (k, exit) in exits.iter().enumerate() {
                    let child_placement = Placement {
                        origin: placement.to_world(exit.position),
                        heading: placement.heading + exit.turn,
                    };
                    if child_placement.heading.abs() > 1 {
                        continue; // only reachable from infeasible data; leave a dead end
                    }
                    let mut branch_plan = plan.clone();
                    let Some(zone) = self.next_zone(&mut branch_plan, child_placement.heading)
                    else {
                        continue;
                    };
                    let child = self.spawn_node(world, zone, child_placement, Some(i), branch_plan);
                    self.nodes[i].as_mut().unwrap().children[k] = Some(child);
                }
                self.nodes[i].as_mut().unwrap().generated = true;
            }
            let children: Vec<usize> = self.node(i).children.iter().flatten().copied().collect();
            let child_limit = if children.len() > 1 {
                limit.min(depth + BRANCH_DEPTH)
            } else {
                limit
            };
            frontier.extend(children.into_iter().map(|c| (c, depth + 1, child_limit)));
        }
    }

    fn remove_node(&mut self, world: &mut World, i: usize) {
        if let Some(node) = self.nodes[i].take() {
            world.despawn(node.root);
        }
    }

    fn remove_subtree(&mut self, world: &mut World, i: usize) {
        let children: Vec<usize> = self.node(i).children.iter().flatten().copied().collect();
        for c in children {
            self.remove_subtree(world, c);
        }
        self.remove_node(world, i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Order;

    fn track(project: Project, source: Source, seed: u64) -> (Track, World) {
        let mut world = World::default();
        let t = Track::new(project, source, seed, &mut world, MeshId::test_id());
        (t, world)
    }

    fn cell(p: &Placement) -> (i32, i32) {
        let c = p.to_world([0.0, 0.0, -ZONE_SIZE / 2.0]);
        (
            (c[0] / ZONE_SIZE).round() as i32,
            (c[2] / ZONE_SIZE).round() as i32,
        )
    }

    /// Walks a runner into a random generated child, `steps` times.
    fn walk(
        t: &mut Track,
        world: &mut World,
        steps: usize,
        mut choose: impl FnMut(usize) -> usize,
    ) -> Vec<u32> {
        let mut zones = vec![t.node(t.current()).zone];
        for _ in 0..steps {
            let kids: Vec<usize> = t
                .node(t.current())
                .children
                .iter()
                .flatten()
                .copied()
                .collect();
            if kids.is_empty() {
                break;
            }
            let next = kids[choose(kids.len())];
            let center = t
                .node(next)
                .placement
                .to_world([0.0, 0.0, -ZONE_SIZE / 2.0]);
            t.update(world, center);
            assert_eq!(t.current(), next);
            zones.push(t.node(next).zone);
        }
        zones
    }

    #[test]
    fn every_track_starts_with_an_empty_runway() {
        let (mut t, mut w) = track(Project::starter("p"), Source::Pool, 9);
        let seen = walk(&mut t, &mut w, RUNWAY + 3, |_| 0);
        assert!(seen[..RUNWAY].iter().all(|&z| z == RUNWAY_ID));
        assert!(
            seen[RUNWAY..].iter().all(|&z| z != RUNWAY_ID),
            "never again later"
        );
        let runway = t.project().zone(RUNWAY_ID).unwrap();
        assert!(
            runway.props.iter().all(|p| p.position[1] < 0.0),
            "floor only"
        );
    }

    #[test]
    fn quarter_turns_match_the_exit_layout() {
        assert_eq!(rotate_quarters([0.0, 0.0, -1.0], 1), [-1.0, 0.0, 0.0]);
        assert_eq!(rotate_quarters([0.0, 0.0, -1.0], -1), [1.0, 0.0, 0.0]);
        let p = Placement {
            origin: [3.0, 0.0, -7.0],
            heading: 1,
        };
        assert_eq!(p.to_local(p.to_world([1.0, 2.0, -4.0])), [1.0, 2.0, -4.0]);
    }

    #[test]
    fn rotated_boxes_swap_their_footprint() {
        let p = Placement {
            origin: [0.0; 3],
            heading: 1,
        };
        let b = p.aabb_to_world(&Aabb::from_center_size([0.0, 0.5, -5.0], [4.0, 1.0, 2.0]));
        assert_eq!(b, Aabb::from_center_size([-5.0, 0.5, 0.0], [2.0, 1.0, 4.0]));
    }

    #[test]
    fn free_mode_never_overlaps_whatever_branch_is_taken() {
        let project = Project::starter("p");
        for seed in 1..15 {
            let (mut t, mut world) = track(project.clone(), Source::Pool, seed);
            let mut s = seed * 7 + 1;
            for _ in 0..150 {
                let cells: Vec<_> = t.nodes().map(|i| cell(&t.node(i).placement)).collect();
                let mut unique = cells.clone();
                unique.sort();
                unique.dedup();
                assert_eq!(unique.len(), cells.len(), "seed {seed}: live zones overlap");
                if walk(&mut t, &mut world, 1, |n| {
                    s = s.wrapping_mul(31).wrapping_add(7);
                    (s as usize) % n
                })
                .len()
                    < 2
                {
                    break;
                }
            }
        }
    }

    #[test]
    fn forks_get_both_branches_and_the_untaken_one_disappears() {
        let mut p = Project::new("p");
        let fork = p.add_zone("Fork", ZoneKind::Fork);
        p.add_zone("Hall", ZoneKind::Straight);
        let (mut t, mut world) = track(p, Source::Repeat(Item::Zone(fork)), 1);
        walk(&mut t, &mut world, RUNWAY, |_| 0);
        let kids: Vec<usize> = t
            .node(t.current())
            .children
            .iter()
            .flatten()
            .copied()
            .collect();
        assert_eq!(kids.len(), 2);
        let untaken = t.node(kids[1]).placement;
        walk(&mut t, &mut world, 1, |_| 0);
        assert!(
            t.nodes().all(|i| t.node(i).placement != untaken),
            "the other branch was removed"
        );
    }

    #[test]
    fn sequence_groups_keep_their_order_along_the_run() {
        let mut p = Project::new("p");
        let ids: Vec<u32> = ["A", "B", "C"]
            .iter()
            .map(|n| p.add_zone(n, ZoneKind::Straight))
            .collect();
        p.add_group("ABC", ids.iter().map(|&i| Item::Zone(i)).collect());
        let (mut t, mut world) = track(p, Source::Pool, 3);
        let seen = &walk(&mut t, &mut world, 8 + RUNWAY, |_| 0)[RUNWAY..];
        assert_eq!(
            seen,
            [
                ids[0], ids[1], ids[2], ids[0], ids[1], ids[2], ids[0], ids[1], ids[2]
            ]
        );
    }

    #[test]
    fn shuffled_groups_use_every_child_each_round() {
        let mut p = Project::new("p");
        let ids: Vec<u32> = ["A", "B", "C", "D"]
            .iter()
            .map(|n| p.add_zone(n, ZoneKind::Straight))
            .collect();
        let g = p.add_group("G", ids.iter().map(|&i| Item::Zone(i)).collect());
        p.group_mut(g).unwrap().order = Order::Shuffle;
        let (mut t, mut world) = track(p, Source::Pool, 5);
        let seen = &walk(&mut t, &mut world, 11 + RUNWAY, |_| 0)[RUNWAY..];
        for round in seen.chunks(4) {
            let mut r = round.to_vec();
            r.sort();
            assert_eq!(r, ids);
        }
    }

    #[test]
    fn repeating_a_turn_recovers_with_a_turn_back() {
        let mut p = Project::new("p");
        let left = p.add_zone("L", ZoneKind::Left);
        p.add_zone("R", ZoneKind::Right);
        let (mut t, mut world) = track(p, Source::Repeat(Item::Zone(left)), 1);
        walk(&mut t, &mut world, 20, |_| 0);
        assert!(t.nodes().all(|i| t.node(i).placement.heading.abs() <= 1));
    }

    #[test]
    fn junction_window_is_around_the_crossing_only() {
        let mut p = Project::new("p");
        let left = p.add_zone("L", ZoneKind::Left);
        let (mut t, mut w) = track(p, Source::Repeat(Item::Zone(left)), 1);
        assert!(
            t.junction([0.0, 0.0, -5.0], 1).is_none(),
            "the runway has no junction"
        );
        walk(&mut t, &mut w, RUNWAY, |_| 0);
        let z0 = -(RUNWAY as f32) * ZONE_SIZE; // the left turn's entry
        assert!(t.junction([0.0, 0.0, z0 - 0.5], 1).is_none(), "too early");
        let j = t
            .junction([0.0, 0.0, z0 - 5.0], 1)
            .expect("at the crossing");
        assert_eq!((j.world_exit, j.heading), ([-5.0, 0.0, z0 - 5.0], 1));
        assert!(
            t.junction([0.0, 0.0, z0 - 5.0], -1).is_none(),
            "no right exit"
        );
    }
}
