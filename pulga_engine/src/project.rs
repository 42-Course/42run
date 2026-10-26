//! A project: everything the editor saves for one game.
//!
//! A project holds **zones** (see [`crate::zone`]) and **groups**. A group
//! is a zone made of other zones and groups (the Composite pattern): when a
//! run uses it, its children are glued one after the other, either in the
//! order they were added ([`Order::Sequence`]) or shuffled each time
//! ([`Order::Shuffle`]). Groups nest.
//!
//! Free mode picks randomly among the project's **roots**: the zones and
//! groups that no group contains. Putting zones in a group takes them out of
//! the random pool and makes them one unit.
//!
//! On disk a project is a directory holding [`PROJECT_FILE`] (TOML, so it
//! reads and diffs well in git) next to the game's own Rust crate. The game
//! embeds the file at build time:
//!
//! ```ignore
//! let project = Project::from_toml(include_str!("../project.toml"))?;
//! ```
//!
//! ```
//! use pulga_engine::project::{Item, Order, Project};
//! use pulga_engine::zone::ZoneKind;
//!
//! let mut p = Project::new("demo");
//! let a = p.add_zone("Hall", ZoneKind::Straight);
//! let b = p.add_zone("Corner", ZoneKind::Left);
//! let g = p.add_group("Hall then corner", vec![Item::Zone(a), Item::Zone(b)]);
//! assert_eq!(p.roots(), [Item::Group(g)]); // grouped zones leave the pool
//! assert_eq!(p.expand(Item::Group(g), &mut |_| 0), [a, b]);
//! assert_eq!(Project::from_toml(&p.to_toml()).unwrap(), p);
//! ```

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::zone::{Prop, Zone, ZoneKind};

/// File name of a project inside its directory.
pub const PROJECT_FILE: &str = "project.toml";

/// How a group glues its children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    /// In the order they were added.
    #[default]
    Sequence,
    /// In a random order, drawn again each time the group is used.
    Shuffle,
}

/// Something a group (or the free-mode pool) can contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Item {
    /// A zone, by id.
    Zone(u32),
    /// A group, by id.
    Group(u32),
}

/// A zone made of zones and groups. See the [module docs](self).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// Stable identity.
    pub id: u32,
    /// Human-readable name.
    pub name: String,
    /// How the children are glued.
    #[serde(default)]
    pub order: Order,
    /// The children, in authored order.
    #[serde(default)]
    pub children: Vec<Item>,
}

/// See the [module docs](self).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// Display name.
    pub name: String,
    /// Every zone, in list order.
    #[serde(default)]
    pub zones: Vec<Zone>,
    /// Every group, in list order.
    #[serde(default)]
    pub groups: Vec<Group>,
    /// The next id to hand out.
    #[serde(default)]
    next_id: u32,
}

/// Why a project failed to load or save.
#[derive(Debug)]
pub enum ProjectError {
    /// Reading or writing the file failed.
    Io(std::io::Error),
    /// The file isn't a valid project.
    Parse(toml::de::Error),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Parse(e) => write!(f, "invalid {PROJECT_FILE}: {e}"),
        }
    }
}

impl std::error::Error for ProjectError {}

/// Deepest group nesting followed; deeper (or cyclic) references are
/// ignored instead of recursing forever.
const MAX_DEPTH: usize = 32;

impl Project {
    /// An empty project.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            zones: Vec::new(),
            groups: Vec::new(),
            next_id: 1,
        }
    }

    fn alloc_id(&mut self) -> u32 {
        let used = self
            .zones
            .iter()
            .map(|z| z.id)
            .chain(self.groups.iter().map(|g| g.id))
            .max()
            .unwrap_or(0);
        self.next_id = self.next_id.max(used + 1);
        self.next_id += 1;
        self.next_id - 1
    }

    /// `base`, or `base 2`, `base 3`... whichever no zone or group uses.
    pub fn unique_name(&self, base: &str) -> String {
        let taken = |n: &str| {
            self.zones.iter().any(|z| z.name == n) || self.groups.iter().any(|g| g.name == n)
        };
        if !taken(base) {
            return base.to_owned();
        }
        (2..)
            .map(|i| format!("{base} {i}"))
            .find(|n| !taken(n))
            .unwrap()
    }

    // --------------------------------------------------------------- zones

    /// The zone with `id`.
    pub fn zone(&self, id: u32) -> Option<&Zone> {
        self.zones.iter().find(|z| z.id == id)
    }

    /// The zone with `id`, mutably.
    pub fn zone_mut(&mut self, id: u32) -> Option<&mut Zone> {
        self.zones.iter_mut().find(|z| z.id == id)
    }

    /// Adds a zone with a starter floor (name made unique); returns its id.
    pub fn add_zone(&mut self, name: &str, kind: ZoneKind) -> u32 {
        let id = self.alloc_id();
        let name = self.unique_name(name);
        self.zones.push(Zone::new(id, name, kind));
        id
    }

    /// Copies zone `id` right after it; returns the copy's id.
    pub fn duplicate_zone(&mut self, id: u32) -> Option<u32> {
        let at = self.zones.iter().position(|z| z.id == id)?;
        let mut copy = self.zones[at].clone();
        copy.id = self.alloc_id();
        copy.name = self.unique_name(&copy.name);
        let new_id = copy.id;
        self.zones.insert(at + 1, copy);
        Some(new_id)
    }

    /// Removes zone `id` and every group's reference to it.
    pub fn delete_zone(&mut self, id: u32) -> Option<Zone> {
        let at = self.zones.iter().position(|z| z.id == id)?;
        for g in &mut self.groups {
            g.children.retain(|c| *c != Item::Zone(id));
        }
        Some(self.zones.remove(at))
    }

    // -------------------------------------------------------------- groups

    /// The group with `id`.
    pub fn group(&self, id: u32) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// The group with `id`, mutably.
    pub fn group_mut(&mut self, id: u32) -> Option<&mut Group> {
        self.groups.iter_mut().find(|g| g.id == id)
    }

    /// Adds a group (name made unique) holding `children` in order;
    /// returns its id.
    pub fn add_group(&mut self, name: &str, children: Vec<Item>) -> u32 {
        let id = self.alloc_id();
        let name = self.unique_name(name);
        self.groups.push(Group {
            id,
            name,
            order: Order::Sequence,
            children,
        });
        id
    }

    /// Copies group `id` (same children, same order mode) right after it;
    /// returns the copy's id.
    pub fn duplicate_group(&mut self, id: u32) -> Option<u32> {
        let at = self.groups.iter().position(|g| g.id == id)?;
        let mut copy = self.groups[at].clone();
        copy.id = self.alloc_id();
        copy.name = self.unique_name(&copy.name);
        let new_id = copy.id;
        self.groups.insert(at + 1, copy);
        Some(new_id)
    }

    /// Removes group `id` and every reference to it. Its children stay in
    /// the project (back in the pool if nothing else holds them).
    pub fn delete_group(&mut self, id: u32) -> Option<Group> {
        let at = self.groups.iter().position(|g| g.id == id)?;
        for g in &mut self.groups {
            g.children.retain(|c| *c != Item::Group(id));
        }
        Some(self.groups.remove(at))
    }

    /// Whether `group` contains `item`, directly or through nested groups.
    pub fn contains(&self, group: u32, item: Item) -> bool {
        fn walk(p: &Project, group: u32, item: Item, depth: usize) -> bool {
            depth < MAX_DEPTH
                && p.group(group).is_some_and(|g| {
                    g.children.iter().any(|&c| {
                        c == item
                            || matches!(c, Item::Group(inner) if walk(p, inner, item, depth + 1))
                    })
                })
        }
        walk(self, group, item, 0)
    }

    /// Whether `item` may be added to `group` without making a group
    /// contain itself.
    pub fn can_add(&self, group: u32, item: Item) -> bool {
        match item {
            Item::Zone(id) => self.zone(id).is_some(),
            Item::Group(id) => {
                id != group && self.group(id).is_some() && !self.contains(id, Item::Group(group))
            }
        }
    }

    /// The zones and groups no group contains: what free mode picks from.
    pub fn roots(&self) -> Vec<Item> {
        let held = |item: Item| self.groups.iter().any(|g| g.children.contains(&item));
        self.zones
            .iter()
            .map(|z| Item::Zone(z.id))
            .chain(self.groups.iter().map(|g| Item::Group(g.id)))
            .filter(|&item| !held(item))
            .collect()
    }

    /// The zone ids `item` glues together, in order: a zone is itself; a
    /// group is its children expanded, in order or shuffled with `pick`
    /// (which must return an index below its argument). Missing ids and
    /// over-deep nesting are skipped.
    pub fn expand(&self, item: Item, pick: &mut impl FnMut(usize) -> usize) -> Vec<u32> {
        let mut out = Vec::new();
        self.expand_into(item, pick, &mut out, 0);
        out
    }

    fn expand_into(
        &self,
        item: Item,
        pick: &mut impl FnMut(usize) -> usize,
        out: &mut Vec<u32>,
        depth: usize,
    ) {
        if depth >= MAX_DEPTH {
            return;
        }
        match item {
            Item::Zone(id) => {
                if self.zone(id).is_some() {
                    out.push(id);
                }
            }
            Item::Group(id) => {
                let Some(group) = self.group(id) else { return };
                let mut children = group.children.clone();
                if group.order == Order::Shuffle {
                    for i in (1..children.len()).rev() {
                        children.swap(i, pick(i + 1)); // Fisher–Yates
                    }
                }
                for child in children {
                    self.expand_into(child, pick, out, depth + 1);
                }
            }
        }
    }

    /// A project to start from: a hall, turns, a fork, three obstacle
    /// zones, and a group gluing the obstacles into one gauntlet.
    pub fn starter(name: impl Into<String>) -> Self {
        const WALL: [f32; 3] = [0.32, 0.34, 0.4];
        const BENCH: [f32; 3] = [0.86, 0.3, 0.3];
        const BLUE: [f32; 3] = [0.23, 0.51, 0.96];
        let mut p = Project::new(name);
        let hall = p.add_zone("Hall", ZoneKind::Straight);
        let walls = |z: &mut Zone| {
            for x in [-4.25, 4.25] {
                z.props
                    .push(Prop::solid([x, 1.0, -5.0], [1.5, 2.0, 10.0], WALL));
            }
        };
        walls(p.zone_mut(hall).unwrap());
        p.add_zone("Left corner", ZoneKind::Left);
        p.add_zone("Right corner", ZoneKind::Right);
        p.add_zone("Junction", ZoneKind::Fork);

        let bench = p.add_zone("Bench", ZoneKind::Straight);
        p.zone_mut(bench).unwrap().props.push(Prop::solid(
            [0.0, 0.4, -5.0],
            [1.6, 0.8, 0.8],
            BENCH,
        ));
        let low = p.add_zone("Low wall", ZoneKind::Straight);
        p.zone_mut(low)
            .unwrap()
            .props
            .push(Prop::solid([0.0, 0.35, -5.0], [7.0, 0.7, 0.5], BLUE));
        let pillars = p.add_zone("Pillars", ZoneKind::Straight);
        for x in [-2.2, 2.2] {
            p.zone_mut(pillars).unwrap().props.push(Prop::solid(
                [x, 1.0, -5.0],
                [1.0, 2.0, 1.0],
                WALL,
            ));
        }
        p.add_group(
            "Gauntlet",
            vec![Item::Zone(bench), Item::Zone(low), Item::Zone(pillars)],
        );
        p
    }

    /// Parses a project from TOML text.
    pub fn from_toml(text: &str) -> Result<Self, ProjectError> {
        toml::from_str(text).map_err(ProjectError::Parse)
    }

    /// Serializes the project to TOML text.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("projects only hold TOML-friendly data")
    }

    /// Loads `dir`/[`PROJECT_FILE`].
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let text =
            std::fs::read_to_string(dir.as_ref().join(PROJECT_FILE)).map_err(ProjectError::Io)?;
        Self::from_toml(&text)
    }

    /// Writes `dir`/[`PROJECT_FILE`], creating `dir` if needed.
    pub fn save(&self, dir: impl AsRef<Path>) -> Result<(), ProjectError> {
        std::fs::create_dir_all(dir.as_ref()).map_err(ProjectError::Io)?;
        std::fs::write(dir.as_ref().join(PROJECT_FILE), self.to_toml()).map_err(ProjectError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("pulga-project-test-{}", std::process::id()));
        let project = Project::starter("round trip");
        project.save(&dir).unwrap();
        let loaded = Project::load(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(loaded, project);
    }

    #[test]
    fn ids_stay_unique_through_adds_duplicates_and_deletes() {
        let mut p = Project::new("p");
        let a = p.add_zone("A", ZoneKind::Straight);
        let b = p.duplicate_zone(a).unwrap();
        p.delete_zone(a);
        let c = p.add_zone("C", ZoneKind::Left);
        let g = p.add_group("G", vec![]);
        let ids = [b, c, g];
        assert!(ids.iter().all(|&i| i != a), "{ids:?}");
        assert_eq!(p.zone(b).unwrap().name, "A 2");
    }

    #[test]
    fn deleting_a_zone_removes_it_from_groups() {
        let mut p = Project::new("p");
        let a = p.add_zone("A", ZoneKind::Straight);
        let b = p.add_zone("B", ZoneKind::Straight);
        let g = p.add_group("G", vec![Item::Zone(a), Item::Zone(b)]);
        p.delete_zone(a);
        assert_eq!(p.group(g).unwrap().children, [Item::Zone(b)]);
    }

    #[test]
    fn deleting_a_group_returns_its_children_to_the_pool() {
        let mut p = Project::new("p");
        let a = p.add_zone("A", ZoneKind::Straight);
        let g = p.add_group("G", vec![Item::Zone(a)]);
        assert_eq!(p.roots(), [Item::Group(g)]);
        p.delete_group(g);
        assert_eq!(p.roots(), [Item::Zone(a)]);
    }

    #[test]
    fn nested_groups_expand_in_order_and_cycles_are_refused() {
        let mut p = Project::new("p");
        let [a, b, c] = ["A", "B", "C"].map(|n| p.add_zone(n, ZoneKind::Straight));
        let inner = p.add_group("Inner", vec![Item::Zone(b), Item::Zone(c)]);
        let outer = p.add_group("Outer", vec![Item::Zone(a), Item::Group(inner)]);
        assert_eq!(p.expand(Item::Group(outer), &mut |_| 0), [a, b, c]);
        assert!(p.contains(outer, Item::Zone(c)));
        assert!(
            !p.can_add(inner, Item::Group(outer)),
            "outer already holds inner"
        );
        assert!(!p.can_add(inner, Item::Group(inner)));
        assert!(p.can_add(outer, Item::Zone(c)));
    }

    #[test]
    fn shuffle_keeps_every_child_once() {
        let mut p = Project::new("p");
        let ids: Vec<u32> = (0..6)
            .map(|i| p.add_zone(&format!("Z{i}"), ZoneKind::Straight))
            .collect();
        let g = p.add_group("G", ids.iter().map(|&i| Item::Zone(i)).collect());
        p.group_mut(g).unwrap().order = Order::Shuffle;
        let mut n = 7usize;
        let mut out = p.expand(Item::Group(g), &mut |len| {
            n = n * 31 + 3;
            n % len
        });
        assert_ne!(out, ids, "this seed reorders");
        out.sort();
        assert_eq!(out, ids);
    }

    #[test]
    fn hand_written_toml_loads_with_defaults() {
        let text = "name = \"x\"\n[[zones]]\nid = 4\nname = \"t\"\nkind = \"left\"\n\
                    [[zones.props]]\nposition = [0.0, 0.5, -5.0]\nsize = [1.0, 1.0, 1.0]\ncolor = [1.0, 0.0, 0.0]\n";
        let mut p = Project::from_toml(text).unwrap();
        assert!(
            p.zones[0].props[0].solid,
            "props are solid unless said otherwise"
        );
        assert_ne!(
            p.add_zone("new", ZoneKind::Straight),
            4,
            "ids continue past loaded ones"
        );
        assert!(matches!(
            Project::from_toml("name = 3"),
            Err(ProjectError::Parse(_))
        ));
    }
}
