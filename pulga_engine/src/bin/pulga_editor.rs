//! `pulga_editor`: build a project's tiles and preview the runs they make.
//!
//! ```text
//! pulga_editor [PROJECT_DIR]    (default: the current directory)
//! ```
//!
//! Opens `PROJECT_DIR/project.toml`, or creates it from
//! [`Project::starter`] if the directory has none. The editor is itself a
//! [`Game`] running on the engine, so what you see is what the engine draws.
//!
//! - Left panel: the project's tiles (click to edit one), add/delete tiles,
//!   save, preview a generated run, play the game (`cargo run --release` in
//!   the project, when it has a `Cargo.toml`).
//! - Viewport: right-drag orbits, the wheel zooms, left-click selects a prop,
//!   `A` adds a box on the floor under the cursor.
//! - Selected prop: arrows move it on the floor, `Q`/`E` lower/raise it,
//!   holding Shift resizes instead of moving, `O` toggles obstacle/scenery,
//!   `C` cycles its color, `D` duplicates, Delete removes, Escape deselects.
//! - `K` cycles the tile's kind (its exits), Tab the next tile, `Ctrl+S`
//!   saves, `P` previews, F5 plays. Unsaved changes are saved on close.

use std::path::PathBuf;
use std::process::{Child, Command};

use pulga_engine::physics::{Vec3, point_at, ray_plane_y};
use pulga_engine::project::{PROJECT_FILE, Project};
use pulga_engine::scene::{Entity, EntityId, Transform};
use pulga_engine::tile::{Prop, PropKind, TILE_SIZE, Tile, TileKind, spawn_tile};
use pulga_engine::{
    Color, DrawCommand, Engine, Event, Game, Key, MouseButton, WindowConfig, track,
};
use pulga_gui_lib::{Button, Rect, Widget};

const PANEL_W: f32 = 240.0;
const ROW_H: f32 = 32.0;
const ROW_GAP: f32 = 6.0;
const STATUS_H: f32 = 28.0;
const STEP: f32 = 0.25; // move/resize increment, world units
const MIN_SIZE: f32 = 0.25;
const PREVIEW_TILES: usize = 60;
const PREVIEW_SPEED: f32 = 14.0;

const PANEL_COLOR: Color = Color {
    r: 0.1,
    g: 0.1,
    b: 0.12,
    a: 0.92,
};
const TEXT: Color = Color::rgb(0.95, 0.95, 0.95);
const DIM_TEXT: Color = Color::rgb(0.65, 0.65, 0.7);
const ACTIVE: Color = Color::rgb(0.15, 0.45, 0.8);
const EXIT_COLOR: Color = Color::rgb(0.95, 0.8, 0.1);
const ENTRY_COLOR: Color = Color::rgb(0.2, 0.8, 0.3);
const OUTLINE_COLOR: Color = Color::rgb(0.15, 0.15, 0.2);
const PALETTE: [[f32; 3]; 7] = [
    [0.55, 0.55, 0.58], // concrete
    [0.8, 0.25, 0.2],   // obstacle red
    [0.95, 0.75, 0.2],  // yellow
    [0.2, 0.5, 0.9],    // 42 blue
    [0.15, 0.15, 0.18], // black
    [0.95, 0.95, 0.95], // white
    [0.3, 0.7, 0.35],   // green
];

#[derive(Clone, Copy)]
enum Action {
    SelectTile(usize),
    AddTile(TileKind),
    DeleteTile,
    Save,
    Preview,
    Play,
}

struct Orbit {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
}

impl Orbit {
    fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let t = self.target;
        let d = self.distance;
        [t[0] + d * cp * sy, t[1] + d * sp, t[2] + d * cp * cy]
    }
}

struct Preview {
    roots: Vec<EntityId>,
    path: Vec<Vec3>,
    travelled: f32,
}

struct Editor {
    dir: PathBuf,
    project: Project,
    current: usize,
    selected: Option<usize>,
    dirty: bool,
    shown: Option<EntityId>,
    orbit: Orbit,
    orbiting: bool,
    last_mouse: (f32, f32),
    shift: bool,
    control: bool,
    preview: Option<Preview>,
    seed: u64,
    game: Option<Child>,
    status: String,
}

impl Editor {
    fn new(dir: PathBuf, project: Project, status: String) -> Self {
        Self {
            dir,
            project,
            current: 0,
            selected: None,
            dirty: false,
            shown: None,
            orbit: Orbit {
                yaw: 0.6,
                pitch: 0.7,
                distance: 22.0,
                target: [0.0, 0.0, -TILE_SIZE / 2.0],
            },
            orbiting: false,
            last_mouse: (0.0, 0.0),
            shift: false,
            control: false,
            preview: None,
            seed: 0x42_2026,
            game: None,
            status,
        }
    }

    fn tile(&self) -> &Tile {
        &self.project.tiles[self.current]
    }

    fn tile_mut(&mut self) -> &mut Tile {
        self.dirty = true;
        &mut self.project.tiles[self.current]
    }

    /// Respawns the edited tile with its helpers (cell outline, entry/exit
    /// markers, selection highlight) after any change.
    fn rebuild(&mut self, engine: &mut Engine) {
        if let Some(root) = self.shown.take() {
            engine.world.despawn(root);
        }
        let mut tile = self.tile().clone();
        if let Some(prop) = self.selected.and_then(|i| tile.props.get_mut(i)) {
            prop.color = prop.color.map(|c| c * 0.4 + 0.6);
        }
        let cube = engine.renderer.cube();
        let root = spawn_tile(&mut engine.world, cube, &tile, Transform::default());

        let mut helper = |pos: Vec3, size: Vec3, color: Color| {
            engine.world.spawn(
                Entity::with_mesh(cube)
                    .with_parent(root)
                    .with_color(color)
                    .with_transform(Transform::at(pos).with_scale(size)),
            );
        };
        let (h, thin) = (TILE_SIZE / 2.0, 0.06);
        helper([0.0, 0.0, 0.0], [TILE_SIZE, thin, thin], OUTLINE_COLOR);
        helper(
            [0.0, 0.0, -TILE_SIZE],
            [TILE_SIZE, thin, thin],
            OUTLINE_COLOR,
        );
        helper([-h, 0.0, -h], [thin, thin, TILE_SIZE], OUTLINE_COLOR);
        helper([h, 0.0, -h], [thin, thin, TILE_SIZE], OUTLINE_COLOR);
        helper([0.0, 0.05, 0.0], [1.0, 0.1, 0.4], ENTRY_COLOR);
        for exit in tile.kind.exits() {
            let size = if exit.turn == 0 {
                [1.0, 0.1, 0.4]
            } else {
                [0.4, 0.1, 1.0]
            };
            helper([exit.position[0], 0.05, exit.position[2]], size, EXIT_COLOR);
        }
        self.shown = Some(root);
    }

    fn select_tile(&mut self, engine: &mut Engine, index: usize) {
        self.current = index.min(self.project.tiles.len() - 1);
        self.selected = None;
        self.rebuild(engine);
    }

    fn save(&mut self) {
        match self.project.save(&self.dir) {
            Ok(()) => {
                self.dirty = false;
                self.status = format!("saved {}", self.dir.join(PROJECT_FILE).display());
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
    }

    fn add_tile(&mut self, engine: &mut Engine, kind: TileKind) {
        let name = (1..)
            .map(|n| format!("{}-{n}", kind.name()))
            .find(|name| self.project.tiles.iter().all(|t| &t.name != name))
            .unwrap();
        self.project.tiles.push(Tile::new(name, kind));
        self.dirty = true;
        let last = self.project.tiles.len() - 1;
        self.select_tile(engine, last);
    }

    fn delete_tile(&mut self, engine: &mut Engine) {
        if self.project.tiles.len() == 1 {
            self.status = "a project keeps at least one tile".into();
            return;
        }
        let removed = self.project.tiles.remove(self.current);
        self.dirty = true;
        self.status = format!("deleted tile {}", removed.name);
        self.select_tile(engine, self.current);
    }

    fn play(&mut self) {
        if !self.dir.join("Cargo.toml").exists() {
            self.status = "no Cargo.toml in the project: nothing to play".into();
            return;
        }
        if self.dirty {
            self.save();
        }
        match Command::new("cargo")
            .args(["run", "--release"])
            .current_dir(&self.dir)
            .spawn()
        {
            Ok(child) => {
                self.game = Some(child);
                self.status = "building and running the game (cargo run --release)...".into();
            }
            Err(e) => self.status = format!("couldn't start cargo: {e}"),
        }
    }

    fn next_random(&mut self, len: usize) -> usize {
        // xorshift64: plenty for picking preview tiles.
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed % len as u64) as usize
    }

    fn start_preview(&mut self, engine: &mut Engine) {
        if let Some(root) = self.shown.take() {
            engine.world.despawn(root);
        }
        let tiles = self.project.tiles.clone();
        let run = track::generate(&tiles, PREVIEW_TILES, |_| true, |len| self.next_random(len));
        let cube = engine.renderer.cube();
        let mut path = Vec::new();
        let roots = run
            .iter()
            .map(|p| {
                path.push(p.origin);
                if tiles[p.tile].kind != TileKind::Straight {
                    path.push(p.to_world([0.0, 0.0, -TILE_SIZE / 2.0]));
                }
                spawn_tile(&mut engine.world, cube, &tiles[p.tile], p.transform())
            })
            .collect();
        self.status = format!(
            "preview: {} tiles glued together (Escape or P to stop)",
            run.len()
        );
        self.preview = Some(Preview {
            roots,
            path,
            travelled: 0.0,
        });
    }

    fn stop_preview(&mut self, engine: &mut Engine) {
        if let Some(preview) = self.preview.take() {
            for root in preview.roots {
                engine.world.despawn(root);
            }
        }
        self.status.clear();
        self.rebuild(engine);
    }

    fn act(&mut self, engine: &mut Engine, action: Action) {
        if self.preview.is_some() {
            self.stop_preview(engine);
        }
        match action {
            Action::SelectTile(i) => self.select_tile(engine, i),
            Action::AddTile(kind) => self.add_tile(engine, kind),
            Action::DeleteTile => self.delete_tile(engine),
            Action::Save => self.save(),
            Action::Preview => self.start_preview(engine),
            Action::Play => self.play(),
        }
    }

    /// The left panel's buttons, top to bottom.
    fn buttons(&self) -> Vec<(Rect, String, Action, bool)> {
        let mut out = Vec::new();
        let mut y = 56.0;
        let mut row = |label: String, action: Action, active: bool| {
            out.push((
                Rect::new(12.0, y, PANEL_W - 24.0, ROW_H),
                label,
                action,
                active,
            ));
            y += ROW_H + ROW_GAP;
        };
        for (i, tile) in self.project.tiles.iter().enumerate() {
            let label: String = tile.name.chars().take(17).collect();
            row(label, Action::SelectTile(i), i == self.current);
        }
        for kind in TileKind::ALL {
            row(format!("+ {}", kind.name()), Action::AddTile(kind), false);
        }
        row("DELETE TILE".into(), Action::DeleteTile, false);
        row(
            if self.dirty { "SAVE *" } else { "SAVE" }.into(),
            Action::Save,
            false,
        );
        row(
            "PREVIEW (P)".into(),
            Action::Preview,
            self.preview.is_some(),
        );
        row("PLAY (F5)".into(), Action::Play, false);
        out
    }

    fn in_viewport(&self, engine: &Engine) -> bool {
        let (x, y) = engine.mouse();
        x > PANEL_W && y < engine.screen_size().1 - STATUS_H
    }

    fn click_viewport(&mut self, engine: &mut Engine) {
        let (origin, dir) = engine.mouse_ray();
        self.selected = self
            .tile()
            .props
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.aabb().ray_hit(origin, dir).map(|t| (i, t)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        self.rebuild(engine);
    }

    fn add_prop_under_cursor(&mut self, engine: &mut Engine) {
        let (origin, dir) = engine.mouse_ray();
        let Some(t) = ray_plane_y(origin, dir, 0.0) else {
            self.status = "point at the floor to add a box".into();
            return;
        };
        let hit = point_at(origin, dir, t);
        let snap = |v: f32| (v / STEP).round() * STEP;
        let tile = self.tile_mut();
        tile.props.push(Prop {
            kind: PropKind::Scenery,
            position: [snap(hit[0]), 0.5, snap(hit[2])],
            size: [1.0; 3],
            color: PALETTE[0],
        });
        self.selected = Some(tile.props.len() - 1);
        self.rebuild(engine);
    }

    /// Edits the selected prop; returns whether `key` was used.
    fn edit_prop(&mut self, engine: &mut Engine, key: Key) -> bool {
        let Some(i) = self.selected else { return false };
        let shift = self.shift;
        let prop = &mut self.project.tiles[self.current].props[i];
        // Arrows/Q/E: (axis, direction). Up/Down work along z, so "up" is away.
        let axis = match key {
            Key::Left => Some((0, -1.0)),
            Key::Right => Some((0, 1.0)),
            Key::Up => Some((2, if shift { 1.0 } else { -1.0 })),
            Key::Down => Some((2, if shift { -1.0 } else { 1.0 })),
            Key::Char('q') => Some((1, -1.0)),
            Key::Char('e') => Some((1, 1.0)),
            _ => None,
        };
        match (axis, key) {
            (Some((a, d)), _) if shift => prop.size[a] = (prop.size[a] + d * STEP).max(MIN_SIZE),
            (Some((a, d)), _) => prop.position[a] += d * STEP,
            (None, Key::Char('o')) => {
                prop.kind = match prop.kind {
                    PropKind::Scenery => PropKind::Obstacle,
                    PropKind::Obstacle => PropKind::Scenery,
                }
            }
            (None, Key::Char('c')) => {
                let next = PALETTE
                    .iter()
                    .position(|&c| c == prop.color)
                    .map_or(0, |i| i + 1);
                prop.color = PALETTE[next % PALETTE.len()];
            }
            (None, Key::Char('d')) => {
                let mut copy = prop.clone();
                copy.position[0] += 1.0;
                let props = &mut self.project.tiles[self.current].props;
                props.push(copy);
                self.selected = Some(props.len() - 1);
            }
            (None, Key::Delete | Key::Backspace) => {
                self.project.tiles[self.current].props.remove(i);
                self.selected = None;
            }
            _ => return false,
        }
        self.dirty = true;
        self.rebuild(engine);
        true
    }

    fn key(&mut self, engine: &mut Engine, key: Key) {
        if self.preview.is_some() {
            if matches!(key, Key::Escape | Key::Char('p')) {
                self.stop_preview(engine);
            }
            return;
        }
        if self.control && key == Key::Char('s') {
            return self.save();
        }
        if self.edit_prop(engine, key) {
            return;
        }
        match key {
            Key::Char('a') if self.in_viewport(engine) => self.add_prop_under_cursor(engine),
            Key::Char('k') => {
                let tile = self.tile_mut();
                tile.kind = tile.kind.next();
                self.status = format!("kind: {} (the floor isn't reshaped)", tile.kind.name());
                self.rebuild(engine);
            }
            Key::Char('p') => self.start_preview(engine),
            Key::Tab => self.select_tile(engine, (self.current + 1) % self.project.tiles.len()),
            Key::F(5) => self.play(),
            Key::Escape => {
                self.selected = None;
                self.rebuild(engine);
            }
            _ => {}
        }
    }

    fn update_preview_camera(&mut self, engine: &mut Engine, dt: f32) {
        let Some(preview) = self.preview.as_mut() else {
            return;
        };
        let length: f32 = preview.path.windows(2).map(|w| dist(w[0], w[1])).sum();
        preview.travelled = (preview.travelled + PREVIEW_SPEED * dt) % length.max(1.0);
        let at = point_along(&preview.path, preview.travelled);
        let ahead = point_along(&preview.path, preview.travelled + 8.0);
        engine.camera.look_at(
            [at[0], 3.0, at[2]],
            [ahead[0], 1.0, ahead[2]],
            [0.0, 1.0, 0.0],
        );
    }

    fn draw_ui(&self, engine: &mut Engine) {
        let (w, h) = engine.screen_size();
        let mouse = engine.mouse();
        engine.ui.push(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, PANEL_W, h),
            color: PANEL_COLOR,
        });
        engine.text(12.0, 16.0, 16.0, TEXT, "PULGA EDITOR");
        let name: String = self.project.name.chars().take(28).collect();
        engine.text(12.0, 38.0, 8.0, DIM_TEXT, name);
        for (rect, label, _, active) in self.buttons() {
            let mut button = Button::new(label);
            if active {
                button.idle_color = ACTIVE;
                button.hovered_color = ACTIVE;
            }
            button.set_bounds(rect);
            button.on_mouse_move(mouse.0, mouse.1);
            engine.ui.extend(button.draw_commands());
        }

        if self.preview.is_none() {
            let tile = self.tile();
            let mut info = format!("{} ({})", tile.name, tile.kind.name());
            if let Some(prop) = self.selected.and_then(|i| tile.props.get(i)) {
                let f = |v: Vec3| format!("{:.2} {:.2} {:.2}", v[0], v[1], v[2]);
                info += &format!(
                    "\n{}\nPOS  {}\nSIZE {}",
                    if prop.kind == PropKind::Obstacle {
                        "OBSTACLE"
                    } else {
                        "SCENERY"
                    },
                    f(prop.position),
                    f(prop.size)
                );
            }
            engine.text(PANEL_W + 16.0, 16.0, 16.0, TEXT, info);
        }

        engine.ui.push(DrawCommand::Rect {
            rect: Rect::new(PANEL_W, h - STATUS_H, w - PANEL_W, STATUS_H),
            color: PANEL_COLOR,
        });
        let help = if self.preview.is_some() {
            "ESC/P STOP PREVIEW"
        } else if self.selected.is_some() {
            "ARROWS/Q/E MOVE  SHIFT RESIZE  O OBSTACLE  C COLOR  D DUP  DEL REMOVE"
        } else {
            "CLICK SELECT  A ADD BOX  RMB ORBIT  WHEEL ZOOM  K KIND  TAB NEXT  CTRL+S SAVE"
        };
        let line = if self.status.is_empty() {
            help.to_string()
        } else {
            format!("{}   |   {}", self.status, help)
        };
        engine.text(PANEL_W + 10.0, h - STATUS_H + 10.0, 8.0, DIM_TEXT, line);
    }
}

fn dist(a: Vec3, b: Vec3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The point `d` units along the polyline `path`, clamped to its ends.
fn point_along(path: &[Vec3], mut d: f32) -> Vec3 {
    for w in path.windows(2) {
        let len = dist(w[0], w[1]);
        if d <= len && len > 0.0 {
            let t = d / len;
            return [0, 1, 2].map(|i| w[0][i] + (w[1][i] - w[0][i]) * t);
        }
        d -= len;
    }
    *path.last().unwrap_or(&[0.0; 3])
}

impl Game for Editor {
    fn init(&mut self, engine: &mut Engine) {
        engine.renderer.clear_color = Color::rgb(0.22, 0.24, 0.28);
        self.rebuild(engine);
    }

    fn event(&mut self, engine: &mut Engine, event: Event) {
        match event {
            Event::KeyDown(Key::Shift) => self.shift = true,
            Event::KeyUp(Key::Shift) => self.shift = false,
            Event::KeyDown(Key::Control) => self.control = true,
            Event::KeyUp(Key::Control) => self.control = false,
            Event::KeyDown(key) => self.key(engine, key),
            Event::MouseDown(MouseButton::Right) if self.in_viewport(engine) => {
                self.orbiting = true;
            }
            Event::MouseUp(MouseButton::Right) => self.orbiting = false,
            Event::MouseDown(MouseButton::Left) => {
                let (mx, my) = engine.mouse();
                if mx <= PANEL_W {
                    let hit = self
                        .buttons()
                        .into_iter()
                        .find(|(r, ..)| r.contains(mx, my));
                    if let Some((_, _, action, _)) = hit {
                        self.act(engine, action);
                    }
                } else if self.preview.is_none() && self.in_viewport(engine) {
                    self.click_viewport(engine);
                }
            }
            Event::MouseMoved { x, y } => {
                if self.orbiting {
                    let (dx, dy) = (x - self.last_mouse.0, y - self.last_mouse.1);
                    self.orbit.yaw -= dx * 0.008;
                    self.orbit.pitch = (self.orbit.pitch + dy * 0.008).clamp(0.05, 1.5);
                }
                self.last_mouse = (x, y);
            }
            Event::Wheel(lines) if self.in_viewport(engine) => {
                self.orbit.distance = (self.orbit.distance * 0.9f32.powf(lines)).clamp(4.0, 120.0);
            }
            Event::CloseRequested if self.dirty => self.save(),
            _ => {}
        }
    }

    fn update(&mut self, engine: &mut Engine, dt: f32) {
        if let Some(child) = self.game.as_mut()
            && let Ok(Some(exit)) = child.try_wait()
        {
            self.status = format!("game exited ({exit})");
            self.game = None;
        }
        if self.preview.is_some() {
            self.update_preview_camera(engine, dt);
        } else {
            engine
                .camera
                .look_at(self.orbit.eye(), self.orbit.target, [0.0, 1.0, 0.0]);
        }
        self.draw_ui(engine);
    }
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    let (project, status) = if dir.join(PROJECT_FILE).exists() {
        match Project::load(&dir) {
            Ok(p) => (p, format!("opened {}", dir.display())),
            Err(e) => {
                eprintln!("pulga_editor: {}: {e}", dir.join(PROJECT_FILE).display());
                std::process::exit(1);
            }
        }
    } else {
        let name = std::fs::canonicalize(&dir)
            .ok()
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "untitled".into());
        let project = Project::starter(name);
        if let Err(e) = project.save(&dir) {
            eprintln!(
                "pulga_editor: can't create {}: {e}",
                dir.join(PROJECT_FILE).display()
            );
            std::process::exit(1);
        }
        (
            project,
            format!("created a starter project in {}", dir.display()),
        )
    };
    let mut editor = Editor::new(dir, project, status);
    if editor.project.tiles.is_empty() {
        editor
            .project
            .tiles
            .push(Tile::new("straight", TileKind::Straight));
    }
    let config = WindowConfig {
        title: "pulga_editor".into(),
        width: 1440,
        height: 900,
    };
    if let Err(e) = pulga_engine::run(config, editor) {
        eprintln!("pulga_editor: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_along_walks_segments_and_clamps() {
        let path = [[0.0, 0.0, 0.0], [0.0, 0.0, -10.0], [-5.0, 0.0, -10.0]];
        assert_eq!(point_along(&path, 5.0), [0.0, 0.0, -5.0]);
        assert_eq!(point_along(&path, 12.0), [-2.0, 0.0, -10.0]);
        assert_eq!(point_along(&path, 99.0), [-5.0, 0.0, -10.0]);
    }

    #[test]
    fn orbit_eye_sits_at_distance_from_target() {
        let orbit = Orbit {
            yaw: 1.0,
            pitch: 0.5,
            distance: 20.0,
            target: [1.0, 2.0, 3.0],
        };
        assert!((dist(orbit.eye(), orbit.target) - 20.0).abs() < 1e-4);
    }
}
