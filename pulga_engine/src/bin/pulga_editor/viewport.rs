//! The 3D view: what's shown for the selection, the fly camera, mouse
//! picking, and dragging props with the move gizmo.

use pulga_engine::gizmo::{Axis, Gizmo};
use pulga_engine::physics::{Vec3, point_at, ray_plane_y};
use pulga_engine::project::{Item, Project};
use pulga_engine::scene::{Entity, EntityId, Transform};
use pulga_engine::track::{Placement, rotate_quarters};
use pulga_engine::view::FlyCamera;
use pulga_engine::zone::{Prop, ZONE_SIZE, spawn_zone};
use pulga_engine::{Color, Engine, Key};

use super::{Editor, SNAP, Selection};

/// Where the camera starts, and returns with "reset view".
pub fn home_camera() -> FlyCamera {
    FlyCamera::looking_at([10.0, 9.0, 7.0], [0.0, 0.0, -5.0])
}

/// Box colors offered for props.
pub const PALETTE: [[f32; 3]; 8] = [
    [0.56, 0.57, 0.6],
    [0.32, 0.34, 0.4],
    [0.92, 0.92, 0.94],
    [0.86, 0.3, 0.3],
    [0.95, 0.72, 0.22],
    [0.23, 0.51, 0.96],
    [0.25, 0.7, 0.42],
    [0.0, 0.73, 0.74],
];

/// What the viewport currently shows.
#[derive(Default)]
pub struct View {
    /// Needs rebuilding (the project or selection changed).
    pub stale: bool,
    roots: Vec<EntityId>,
    gizmo: Option<EntityId>,
    /// Each shown zone and where it sits.
    pub shown: Vec<(u32, Placement)>,
    /// The gizmo handle under the mouse.
    pub hot: Option<Axis>,
}

/// A gizmo drag in progress.
pub struct Drag {
    axis: Axis,
    zone: u32,
    prop: usize,
    placement: Placement,
    gizmo: Gizmo,
    start_param: f32,
    start_local: Vec3,
}

/// `item`'s zones in authored order (shuffle ignored), for display.
fn authored(project: &Project, item: Item, out: &mut Vec<u32>, depth: usize) {
    if depth > 32 {
        return;
    }
    match item {
        Item::Zone(id) => out.push(id),
        Item::Group(id) => {
            for &child in project.group(id).map_or(&[][..], |g| &g.children) {
                authored(project, child, out, depth + 1);
            }
        }
    }
}

/// Zones glued one after another on their first exit, from the origin.
fn arrange(project: &Project, item: Item) -> Vec<(u32, Placement)> {
    let mut ids = Vec::new();
    authored(project, item, &mut ids, 0);
    let mut placement = Placement {
        origin: [0.0; 3],
        heading: 0,
    };
    let mut out = Vec::new();
    for id in ids {
        let Some(zone) = project.zone(id) else {
            continue;
        };
        out.push((id, placement));
        let exit = zone.kind.exits()[0];
        placement = Placement {
            origin: placement.to_world(exit.position),
            heading: placement.heading + exit.turn,
        };
    }
    out
}

impl Editor {
    fn shown_placement(&self, zone: u32) -> Option<Placement> {
        self.view
            .shown
            .iter()
            .find(|(z, _)| *z == zone)
            .map(|(_, p)| *p)
    }

    /// World position of the selected prop's center.
    fn selected_prop_world(&self) -> Option<(u32, usize, Placement, Vec3)> {
        let (Selection::Zone(zone), Some(prop)) = (self.selection, self.prop) else {
            return None;
        };
        let placement = self.shown_placement(zone)?;
        let p = self.project.zone(zone)?.props.get(prop)?;
        Some((zone, prop, placement, placement.to_world(p.position)))
    }

    fn gizmo(&self) -> Option<Gizmo> {
        let (.., world) = self.selected_prop_world()?;
        Some(Gizmo::for_view(world, self.camera.position))
    }

    pub(super) fn update_viewport(&mut self, engine: &mut Engine, dt: f32, delta: (f32, f32)) {
        engine.renderer.clear_color = engine.gui.theme.background;
        engine.renderer.fog = None;

        if self.looking {
            self.camera.look(delta.0, delta.1);
            let speed = if engine.key_down(Key::Shift) {
                24.0
            } else {
                8.0
            } * dt;
            let axis = |plus: char, minus: char| {
                (engine.key_down(Key::Char(plus)) as i32 - engine.key_down(Key::Char(minus)) as i32)
                    as f32
            };
            self.camera.fly(
                axis('w', 's') * speed,
                axis('d', 'a') * speed,
                axis('e', 'q') * speed,
            );
        }
        self.camera.apply(&mut engine.camera);

        if self.drag.is_some() {
            self.drag_gizmo(engine);
        }
        let (origin, dir) = engine.mouse_ray();
        self.view.hot = match &self.drag {
            Some(d) => Some(d.axis),
            None if !self.looking && !engine.gui.wants_pointer() => {
                self.gizmo().and_then(|g| g.pick(origin, dir))
            }
            None => None,
        };

        if self.view.stale {
            self.rebuild(engine);
        }
        // The gizmo follows the camera distance, so it's rebuilt every frame.
        if let Some(g) = self.view.gizmo.take() {
            engine.world.despawn(g);
        }
        if let Some(gizmo) = self.gizmo() {
            let (cube, sphere) = (engine.renderer.cube(), engine.renderer.sphere());
            self.view.gizmo = Some(gizmo.spawn(&mut engine.world, cube, sphere, self.view.hot));
        }
    }

    pub(super) fn clear_view(&mut self, engine: &mut Engine) {
        for root in self.view.roots.drain(..).chain(self.view.gizmo.take()) {
            engine.world.despawn(root);
        }
        self.view.shown.clear();
        self.view.stale = true;
    }

    /// Respawns everything shown: grid, the selection's zones, sockets.
    fn rebuild(&mut self, engine: &mut Engine) {
        self.clear_view(engine);
        self.view.stale = false;
        self.view.shown = match self.selection.item() {
            Some(item) => arrange(&self.project, item),
            None => Vec::new(),
        };
        let cube = engine.renderer.cube();
        let theme = engine.gui.theme.clone();

        // Grid under everything: a line per unit, stronger on cell edges.
        let (mut min, mut max) = ([-15.0f32, -25.0f32], [15.0f32, 5.0f32]);
        for (_, p) in &self.view.shown {
            let c = p.to_world([0.0, 0.0, -ZONE_SIZE / 2.0]);
            min = [min[0].min(c[0] - 15.0), min[1].min(c[2] - 15.0)];
            max = [max[0].max(c[0] + 15.0), max[1].max(c[2] + 15.0)];
        }
        let snap10 = |v: f32, up: bool| {
            if up {
                (v / 10.0).ceil() * 10.0
            } else {
                (v / 10.0).floor() * 10.0
            }
        };
        let (x0, x1, z0, z1) = (
            snap10(min[0], false),
            snap10(max[0], true),
            snap10(min[1], false),
            snap10(max[1], true),
        );
        let grid = engine.world.spawn(Entity::default());
        let faint = theme.background.lerp(theme.text, 0.07);
        let strong = theme.background.lerp(theme.text, 0.16);
        let mut line = |pos: Vec3, size: Vec3, color: Color| {
            engine.world.spawn(
                Entity::with_mesh(cube)
                    .with_parent(grid)
                    .with_color(color)
                    .with_transform(Transform::at(pos).with_scale(size)),
            );
        };
        let y = -0.21;
        for x in (x0 as i32..=x1 as i32).map(|v| v as f32) {
            let edge = (x + 5.0).rem_euclid(10.0) == 0.0;
            let w = if edge { 0.05 } else { 0.025 };
            line(
                [x, y, (z0 + z1) / 2.0],
                [w, 0.005, z1 - z0],
                if edge { strong } else { faint },
            );
        }
        for z in (z0 as i32..=z1 as i32).map(|v| v as f32) {
            let edge = z.rem_euclid(10.0) == 0.0;
            let w = if edge { 0.05 } else { 0.025 };
            line(
                [(x0 + x1) / 2.0, y, z],
                [x1 - x0, 0.005, w],
                if edge { strong } else { faint },
            );
        }
        self.view.roots.push(grid);

        // The zones, the selected prop highlighted.
        for (i, (id, placement)) in self.view.shown.clone().into_iter().enumerate() {
            let Some(mut zone) = self.project.zone(id).cloned() else {
                continue;
            };
            if self.selection == Selection::Zone(id)
                && let Some(prop) = self.prop.and_then(|p| zone.props.get_mut(p))
            {
                prop.color = prop.color.map(|c| c * 0.55 + 0.45);
            }
            let root = spawn_zone(&mut engine.world, cube, &zone, placement.transform());
            self.view.roots.push(root);

            // Socket arrows: where the runner comes in (first zone) and leaves.
            let arrow =
                |world: &mut pulga_engine::scene::World, from: Vec3, dir: Vec3, color: Color| {
                    let len = 1.6;
                    let center = [
                        from[0] + dir[0] * len / 2.0,
                        0.03,
                        from[2] + dir[2] * len / 2.0,
                    ];
                    let size = if dir[0] != 0.0 {
                        [len, 0.04, 0.22]
                    } else {
                        [0.22, 0.04, len]
                    };
                    world.spawn(
                        Entity::with_mesh(cube)
                            .with_parent(root)
                            .with_color(color)
                            .with_transform(Transform::at(center).with_scale(size)),
                    );
                    let tip = [from[0] + dir[0] * len, 0.03, from[2] + dir[2] * len];
                    world.spawn(
                        Entity::with_mesh(cube)
                            .with_parent(root)
                            .with_color(color)
                            .with_transform(
                                Transform::at(tip)
                                    .with_rotation([0.0, std::f32::consts::FRAC_PI_4, 0.0])
                                    .with_scale([0.55, 0.04, 0.55]),
                            ),
                    );
                };
            if i == 0 {
                arrow(
                    &mut engine.world,
                    [0.0, 0.0, 1.6],
                    [0.0, 0.0, -1.0],
                    theme.success,
                );
            }
            let last = i + 1 == self.view.shown.len();
            if last || self.view.shown.len() == 1 {
                for exit in zone.kind.exits() {
                    let dir = rotate_quarters([0.0, 0.0, -1.0], exit.turn);
                    arrow(&mut engine.world, exit.position, dir, theme.warning);
                }
            }
        }
    }

    /// Left click in the viewport: grab a gizmo handle, or pick a prop.
    pub(super) fn viewport_press(&mut self, engine: &mut Engine) {
        let (origin, dir) = engine.mouse_ray();
        if let (Some(gizmo), Some((zone, prop, placement, _))) =
            (self.gizmo(), self.selected_prop_world())
            && let Some(axis) = gizmo.pick(origin, dir)
            && let Some(start_param) = gizmo.axis_param(axis, origin, dir)
        {
            let start_local = self.project.zone(zone).unwrap().props[prop].position;
            self.history.record(&self.project);
            self.drag = Some(Drag {
                axis,
                zone,
                prop,
                placement,
                gizmo,
                start_param,
                start_local,
            });
            return;
        }
        let mut best: Option<(f32, u32, usize)> = None;
        for &(id, placement) in &self.view.shown {
            let Some(zone) = self.project.zone(id) else {
                continue;
            };
            let (lo, ld) = (
                placement.to_local(origin),
                rotate_quarters(dir, -placement.heading),
            );
            for (i, prop) in zone.props.iter().enumerate() {
                if let Some(t) = prop.aabb().ray_hit(lo, ld)
                    && best.is_none_or(|(bt, ..)| t < bt)
                {
                    best = Some((t, id, i));
                }
            }
        }
        match best {
            Some((_, zone, prop)) => {
                self.select(Selection::Zone(zone));
                self.prop = Some(prop);
            }
            None => self.prop = None,
        }
        self.view.stale = true;
    }

    fn drag_gizmo(&mut self, engine: &mut Engine) {
        let Some(drag) = &self.drag else { return };
        let (origin, dir) = engine.mouse_ray();
        let Some(param) = drag.gizmo.axis_param(drag.axis, origin, dir) else {
            return;
        };
        let mut moved = param - drag.start_param;
        if self.snap {
            moved = (moved / SNAP).round() * SNAP;
        }
        let world_delta = drag.axis.unit().map(|c| c * moved);
        let local_delta = rotate_quarters(world_delta, -drag.placement.heading);
        let target = [0, 1, 2].map(|i| drag.start_local[i] + local_delta[i]);
        let (zone, prop) = (drag.zone, drag.prop);
        if let Some(p) = self
            .project
            .zone_mut(zone)
            .and_then(|z| z.props.get_mut(prop))
            && p.position != target
        {
            p.position = target;
            self.dirty = true;
            self.view.stale = true;
        }
    }

    /// `A`: a new box on the floor under the cursor, in the selected zone.
    pub(super) fn add_box_under_cursor(&mut self, engine: &mut Engine) {
        let Selection::Zone(id) = self.selection else {
            engine.gui.notify(
                "Select a zone to add boxes to",
                pulga_engine::gui::ToastKind::Info,
            );
            return;
        };
        let Some(placement) = self.shown_placement(id) else {
            return;
        };
        let (origin, dir) = engine.mouse_ray();
        let local = match ray_plane_y(origin, dir, 0.0) {
            Some(t) => placement.to_local(point_at(origin, dir, t)),
            None => [0.0, 0.0, -ZONE_SIZE / 2.0],
        };
        let snap = |v: f32| (v / SNAP).round() * SNAP;
        let prop = Prop::solid([snap(local[0]), 0.5, snap(local[2])], [1.0; 3], PALETTE[3]);
        self.prop = self.edit(|p| {
            let zone = p.zone_mut(id)?;
            zone.props.push(prop);
            Some(zone.props.len() - 1)
        });
    }

    /// `F`: frames the selected prop, or the selection's zones.
    pub(super) fn focus_selection(&mut self) {
        let target = match self.selected_prop_world() {
            Some((.., world)) => world,
            None if !self.view.shown.is_empty() => {
                let n = self.view.shown.len() as f32;
                let sum = self.view.shown.iter().fold([0.0; 3], |acc, (_, p)| {
                    let c = p.to_world([0.0, 0.0, -ZONE_SIZE / 2.0]);
                    [acc[0] + c[0], acc[1] + c[1], acc[2] + c[2]]
                });
                sum.map(|v| v / n)
            }
            None => [0.0, 0.0, -ZONE_SIZE / 2.0],
        };
        let distance = if self.prop.is_some() {
            8.0
        } else {
            10.0 + 6.0 * self.view.shown.len() as f32
        };
        self.camera.focus(target, distance.min(60.0));
    }
}
