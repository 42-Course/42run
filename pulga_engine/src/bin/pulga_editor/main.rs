//! `pulga_editor`: the scene designer for 42run projects.
//!
//! ```text
//! pulga_editor [PROJECT_DIR]    (default: the current directory)
//! ```
//!
//! Opens `PROJECT_DIR/project.toml`, or creates it from
//! [`Project::starter`] if the directory has none. Without `PROJECT_DIR`,
//! opens the current directory's project (unless it's a Cargo workspace
//! root), or the one project found in its subdirectories (e.g.
//! `pulga_run/` from the repo root); it never creates one then. The editor is itself a
//! [`Game`] running on the engine: what you see is what the engine draws.
//!
//! - **Scene** (left): the project's zones and groups. Groups expand to show
//!   their children; zones inside a group leave the random free-mode pool.
//!   New zones and groups from the `+ New` menu.
//! - **Viewport** (center): the selected zone, or a group's zones glued in
//!   order. Hold the right mouse button to look around and fly with
//!   WASD / Q / E (Shift: faster); the wheel moves forward and back; `F`
//!   focuses the selection. Click a box to select it and drag the arrows of
//!   its move gizmo; `A` adds a box on the floor under the cursor.
//! - **Inspector** (right): everything about the selection, editable.
//! - **Run preview** (`P`): run through the selection right there, with
//!   the game's runner: arrows/A/D steer and turn, Space/W jumps, `R`
//!   restarts, Escape stops.
//! - `Ctrl+Z` / `Ctrl+Y` undo/redo, `Ctrl+S` saves, `Ctrl+D` duplicates,
//!   Delete removes, F5 plays the game (`cargo run --release` in the
//!   project). Unsaved changes are saved on close.
//!
//! Startup options, for scripted screenshots (see `PULGA_SCREENSHOT` in
//! `pulga_win_lib::run`): `--select NAME` selects a zone or group,
//! `--prop N` selects its N-th box (from 1), `--preview` starts a run
//! preview, `--light` uses the light theme.

mod panels;
mod preview;
mod viewport;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use pulga_engine::gui::{Theme, ToastKind};
use pulga_engine::history::History;
use pulga_engine::project::{Item, PROJECT_FILE, Project};
use pulga_engine::view::FlyCamera;
use pulga_engine::zone::Zone;
use pulga_engine::{Engine, Event, Game, Key, MouseButton, WindowConfig};

use preview::Preview;
use viewport::{Drag, View};

/// What the inspector and viewport are about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    Nothing,
    Zone(u32),
    Group(u32),
}

impl Selection {
    fn item(self) -> Option<Item> {
        match self {
            Selection::Nothing => None,
            Selection::Zone(id) => Some(Item::Zone(id)),
            Selection::Group(id) => Some(Item::Group(id)),
        }
    }
}

/// A destructive action waiting for confirmation.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Confirm {
    DeleteZone(u32),
    DeleteGroup(u32),
}

struct Editor {
    dir: PathBuf,
    project: Project,
    history: History<Project>,
    dirty: bool,
    selection: Selection,
    /// Selected prop of the selected zone.
    prop: Option<usize>,
    /// Which groups are expanded in the hierarchy.
    open_groups: HashMap<u32, bool>,
    camera: FlyCamera,
    view: View,
    drag: Option<Drag>,
    looking: bool,
    last_mouse: (f32, f32),
    snap: bool,
    preview: Option<Preview>,
    confirm: Option<Confirm>,
    dark: bool,
    /// Inspector "add to group" / "add child" pickers.
    pick_group: usize,
    pick_child: usize,
    game: Option<Child>,
    status: String,
    /// Toast to show on the next UI frame (set before the UI exists).
    pending_toast: Option<(String, ToastKind)>,
    /// `--preview`: start a run preview on the first frame.
    preview_on_start: bool,
}

/// Grid step for moving and sizing, in world units.
const SNAP: f32 = 0.25;

impl Editor {
    fn new(dir: PathBuf, project: Project, status: String) -> Self {
        let selection = project
            .zones
            .first()
            .map_or(Selection::Nothing, |z| Selection::Zone(z.id));
        Self {
            dir,
            project,
            history: History::new(200),
            dirty: false,
            selection,
            prop: None,
            open_groups: HashMap::new(),
            camera: viewport::home_camera(),
            view: View::default(),
            drag: None,
            looking: false,
            last_mouse: (0.0, 0.0),
            snap: true,
            preview: None,
            confirm: None,
            dark: true,
            pick_group: 0,
            pick_child: 0,
            game: None,
            status,
            pending_toast: None,
            preview_on_start: false,
        }
    }

    /// Applies the startup options (see the module docs).
    fn apply_options(&mut self, args: &[String]) {
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--select" => {
                    let name = it.next().cloned().unwrap_or_default();
                    let zone = self
                        .project
                        .zones
                        .iter()
                        .find(|z| z.name == name)
                        .map(|z| Selection::Zone(z.id));
                    let group = self
                        .project
                        .groups
                        .iter()
                        .find(|g| g.name == name)
                        .map(|g| Selection::Group(g.id));
                    match zone.or(group) {
                        Some(sel) => self.select(sel),
                        None => eprintln!("pulga_editor: no zone or group named {name:?}"),
                    }
                }
                "--prop" => {
                    self.prop = it
                        .next()
                        .and_then(|n| n.parse::<usize>().ok())
                        .and_then(|n| n.checked_sub(1))
                }
                "--preview" => self.preview_on_start = true,
                "--light" => self.dark = false,
                other => eprintln!("pulga_editor: unknown option {other:?}"),
            }
        }
        self.fix_selection();
    }

    fn toast(&mut self, engine: &mut Engine, text: impl Into<String>, kind: ToastKind) {
        engine.gui.notify(text, kind);
    }

    // ---------------------------------------------------------------- edits

    /// Every change to the project goes through here: remembered for undo,
    /// marked unsaved, and the viewport rebuilt.
    fn edit<R>(&mut self, f: impl FnOnce(&mut Project) -> R) -> R {
        self.history.record(&self.project);
        let r = f(&mut self.project);
        self.dirty = true;
        self.view.stale = true;
        r
    }

    fn zone(&self) -> Option<&Zone> {
        match self.selection {
            Selection::Zone(id) => self.project.zone(id),
            _ => None,
        }
    }

    fn select(&mut self, selection: Selection) {
        if self.selection != selection {
            self.selection = selection;
            self.prop = None;
            self.view.stale = true;
        }
    }

    /// Drops selections that no longer exist (after undo, delete...).
    fn fix_selection(&mut self) {
        let exists = match self.selection {
            Selection::Nothing => true,
            Selection::Zone(id) => self.project.zone(id).is_some(),
            Selection::Group(id) => self.project.group(id).is_some(),
        };
        if !exists {
            self.selection = Selection::Nothing;
        }
        let props = self.zone().map_or(0, |z| z.props.len());
        if self.prop.is_some_and(|p| p >= props) {
            self.prop = None;
        }
        self.view.stale = true;
    }

    fn undo(&mut self, engine: &mut Engine) {
        match self.history.undo(self.project.clone()) {
            Some(previous) => {
                self.project = previous;
                self.dirty = true;
                self.fix_selection();
                self.toast(engine, "Undone", ToastKind::Info);
            }
            None => self.toast(engine, "Nothing to undo", ToastKind::Info),
        }
    }

    fn redo(&mut self, engine: &mut Engine) {
        match self.history.redo(self.project.clone()) {
            Some(next) => {
                self.project = next;
                self.dirty = true;
                self.fix_selection();
                self.toast(engine, "Redone", ToastKind::Info);
            }
            None => self.toast(engine, "Nothing to redo", ToastKind::Info),
        }
    }

    fn save(&mut self, engine: Option<&mut Engine>) {
        let (text, kind) = match self.project.save(&self.dir) {
            Ok(()) => {
                self.dirty = false;
                (
                    format!("Saved {}", self.dir.join(PROJECT_FILE).display()),
                    ToastKind::Success,
                )
            }
            Err(e) => (format!("Save failed: {e}"), ToastKind::Error),
        };
        self.status = text.clone();
        match engine {
            Some(engine) => engine.gui.notify(text, kind),
            None => self.pending_toast = Some((text, kind)),
        }
    }

    fn duplicate_selection(&mut self, engine: &mut Engine) {
        if let (Selection::Zone(id), Some(p)) = (self.selection, self.prop) {
            let copy = self.edit(|proj| {
                let zone = proj.zone_mut(id)?;
                let mut prop = zone.props.get(p)?.clone();
                prop.position[0] += 1.0;
                zone.props.push(prop);
                Some(zone.props.len() - 1)
            });
            self.prop = copy;
            return;
        }
        let made = match self.selection {
            Selection::Zone(id) => self.edit(|p| p.duplicate_zone(id)).map(Selection::Zone),
            Selection::Group(id) => self.edit(|p| p.duplicate_group(id)).map(Selection::Group),
            Selection::Nothing => None,
        };
        if let Some(sel) = made {
            self.select(sel);
            self.toast(engine, "Duplicated", ToastKind::Success);
        }
    }

    fn delete_selection(&mut self) {
        if let (Selection::Zone(id), Some(p)) = (self.selection, self.prop) {
            self.edit(|proj| proj.zone_mut(id).map(|z| z.props.remove(p)));
            self.prop = None;
            return;
        }
        self.confirm = match self.selection {
            Selection::Zone(id) => Some(Confirm::DeleteZone(id)),
            Selection::Group(id) => Some(Confirm::DeleteGroup(id)),
            Selection::Nothing => None,
        };
    }

    fn confirmed(&mut self, engine: &mut Engine, what: Confirm) {
        let name = match what {
            Confirm::DeleteZone(id) => self.edit(|p| p.delete_zone(id)).map(|z| z.name),
            Confirm::DeleteGroup(id) => self.edit(|p| p.delete_group(id)).map(|g| g.name),
        };
        self.fix_selection();
        if let Some(name) = name {
            self.toast(engine, format!("Deleted {name}"), ToastKind::Info);
        }
    }

    fn play(&mut self, engine: &mut Engine) {
        // The game is the project's own crate: a Cargo.toml with a
        // [package] (a bare workspace manifest has nothing to run).
        if !has_package(&self.dir) {
            let text = format!(
                "{} has no game crate to play (a Cargo.toml with [package]). Open the game's folder, e.g. pulga_editor pulga_run",
                self.dir.display()
            );
            self.toast(engine, text, ToastKind::Error);
            return;
        }
        if self.dirty {
            self.save(Some(engine));
        }
        match Command::new("cargo")
            .args(["run", "--release"])
            .current_dir(&self.dir)
            .spawn()
        {
            Ok(child) => {
                self.game = Some(child);
                self.status = "Building and running the game (cargo run --release)...".into();
            }
            Err(e) => self.toast(
                engine,
                format!("Couldn't start cargo: {e}"),
                ToastKind::Error,
            ),
        }
    }

    fn set_theme(&mut self, engine: &mut Engine, dark: bool) {
        self.dark = dark;
        engine.gui.theme = if dark { Theme::dark() } else { Theme::light() };
        self.view.stale = true;
    }

    // ------------------------------------------------------------ shortcuts

    fn shortcut(&mut self, engine: &mut Engine, key: Key) {
        let ctrl = engine.key_down(Key::Control);
        let shift = engine.key_down(Key::Shift);
        if self.confirm.is_some() {
            return; // the dialog handles Enter/Escape itself
        }
        if let Some(preview) = self.preview.as_mut() {
            match key {
                Key::Escape | Key::Char('p') => self.stop_preview(engine),
                Key::Char('r') => preview.restart(engine),
                _ => preview.key(key),
            }
            return;
        }
        match key {
            Key::Char('z') if ctrl && shift => self.redo(engine),
            Key::Char('z') if ctrl => self.undo(engine),
            Key::Char('y') if ctrl => self.redo(engine),
            Key::Char('s') if ctrl => self.save(Some(engine)),
            Key::Char('d') if ctrl => self.duplicate_selection(engine),
            Key::Delete | Key::Backspace => self.delete_selection(),
            Key::Char('f') if !self.looking => self.focus_selection(),
            Key::Char('a') if !self.looking && !ctrl => self.add_box_under_cursor(engine),
            Key::Char('p') if !self.looking => self.start_preview(engine),
            Key::F(5) => self.play(engine),
            Key::Escape => {
                if self.prop.take().is_none() {
                    self.select(Selection::Nothing);
                }
                self.view.stale = true;
            }
            _ => {}
        }
    }
}

impl Game for Editor {
    fn init(&mut self, engine: &mut Engine) {
        self.set_theme(engine, self.dark);
        if self.preview_on_start {
            self.start_preview(engine);
        }
        if let Some((text, kind)) = self.pending_toast.take() {
            engine.gui.notify(text, kind);
        }
    }

    fn event(&mut self, engine: &mut Engine, event: Event) {
        match event {
            Event::KeyDown(key) if !engine.gui.wants_keyboard() => self.shortcut(engine, key),
            Event::MouseDown(MouseButton::Right)
                if !engine.gui.wants_pointer() && self.preview.is_none() =>
            {
                self.looking = true;
            }
            Event::MouseUp(MouseButton::Right) => self.looking = false,
            Event::MouseDown(MouseButton::Left)
                if !engine.gui.wants_pointer() && self.preview.is_none() =>
            {
                self.viewport_press(engine);
            }
            Event::MouseUp(MouseButton::Left) => self.drag = None,
            Event::Wheel(lines) if !engine.gui.wants_pointer() && self.preview.is_none() => {
                let speed = if engine.key_down(Key::Shift) {
                    6.0
                } else {
                    2.0
                };
                self.camera.fly(lines * speed, 0.0, 0.0);
            }
            Event::CloseRequested if self.dirty => self.save(None),
            _ => {}
        }
    }

    fn update(&mut self, engine: &mut Engine, dt: f32) {
        if let Some(child) = self.game.as_mut()
            && let Ok(Some(exit)) = child.try_wait()
        {
            self.status = format!("Game exited ({exit})");
            self.game = None;
        }
        let mouse = engine.mouse();
        let delta = (mouse.0 - self.last_mouse.0, mouse.1 - self.last_mouse.1);
        self.last_mouse = mouse;

        if self.preview.is_some() {
            self.update_preview(engine, dt);
        } else {
            self.update_viewport(engine, dt, delta);
        }
        self.draw_panels(engine);
    }
}

/// Whether `dir`'s Cargo.toml has a `[package]`: a game crate to play.
fn has_package(dir: &Path) -> bool {
    let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
    manifest.lines().any(|l| l.trim() == "[package]")
}

/// Whether `dir` is a Cargo workspace root (like the 42Run repo root):
/// games live in its members, not in it.
fn is_workspace_root(dir: &Path) -> bool {
    let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
    manifest.lines().any(|l| l.trim() == "[workspace]") && !has_package(dir)
}

/// With no directory given: the current one if it holds a project (unless
/// it's a workspace root), else its only subdirectory holding one (the repo
/// root, or a release folder shipping `pulga_run/` next to the editor).
fn find_project() -> Option<PathBuf> {
    let here = PathBuf::from(".");
    let has_project = |d: &Path| d.join(PROJECT_FILE).is_file();
    if has_project(&here) && !is_workspace_root(&here) {
        return Some(here);
    }
    let mut found: Vec<PathBuf> = std::fs::read_dir(&here)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| has_project(p))
        .collect();
    found.sort();
    match found.len() {
        1 => found.pop(),
        _ if has_project(&here) => Some(here),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (options, dir): (Vec<String>, Vec<String>) = {
        // The project dir is the first argument that isn't an option or an
        // option's value.
        let mut dir = Vec::new();
        let mut options = Vec::new();
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            if a.starts_with("--") {
                let takes_value = matches!(a.as_str(), "--select" | "--prop");
                options.push(a);
                if takes_value {
                    options.extend(it.next());
                }
            } else {
                dir.push(a);
            }
        }
        (options, dir)
    };
    let dir = match dir.into_iter().next() {
        Some(dir) => PathBuf::from(dir),
        None => find_project().unwrap_or_else(|| {
            eprintln!(
                "pulga_editor: no project.toml here or in a subfolder.\n\
                 usage: pulga_editor PROJECT_DIR   (created with a starter project if missing)"
            );
            std::process::exit(2);
        }),
    };
    let (project, status) = if dir.join(PROJECT_FILE).exists() {
        match Project::load(&dir) {
            Ok(p) => (p, format!("Opened {}", dir.display())),
            Err(e) => {
                eprintln!("pulga_editor: {}: {e}", dir.join(PROJECT_FILE).display());
                std::process::exit(1);
            }
        }
    } else {
        let name = std::path::absolute(&dir)
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
            format!("Created a starter project in {}", dir.display()),
        )
    };
    let mut editor = Editor::new(dir, project, status);
    editor.apply_options(&options);
    let config = WindowConfig {
        title: "pulga editor".into(),
        width: 1600,
        height: 940,
    };
    if let Err(e) = pulga_engine::run(config, editor) {
        eprintln!("pulga_editor: {e}");
        std::process::exit(1);
    }
}
