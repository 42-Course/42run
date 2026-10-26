//! The 42run game engine.
//!
//! Built on the two libraries: [`pulga_win_lib`] (window, input, GL context,
//! meshes, matrices, drawing pipelines) and [`pulga_gui_lib`] (immediate-mode
//! UI, text, icons). The engine adds what makes a game out of those:
//!
//! - [`run`] and the [`Game`] trait: the frame loop, timing, input, UI frame,
//! - [`scene`]: a [`scene::World`] of entities in a transform hierarchy,
//! - [`zone`] and [`project`]: the designed pieces of track, groups of them,
//!   and the file the editor saves,
//! - [`track`]: zones glued into an endless, branching run,
//! - [`runner`]: the character running it (lanes, turns, jumps, collisions),
//! - [`physics`]: bounding boxes and ray casts,
//! - [`gizmo`], [`history`], [`view`]: editing tools (move handles,
//!   undo/redo, fly and chase cameras),
//! - [`render`]: drawing the world and the UI each frame.
//!
//! The `pulga_editor` program (`src/bin/pulga_editor.rs`) is built with this
//! crate: a scene designer for a project's zones and groups. A game is a
//! separate crate that depends on this one and implements [`Game`]:
//!
//! ```no_run
//! use pulga_engine::{Engine, Game, WindowConfig, scene::Entity};
//!
//! struct Spinner(Option<pulga_engine::scene::EntityId>);
//!
//! impl Game for Spinner {
//!     fn init(&mut self, engine: &mut Engine) {
//!         let cube = engine.renderer.cube();
//!         self.0 = Some(engine.world.spawn(Entity::with_mesh(cube)));
//!         engine.camera.look_at([0.0, 1.0, 4.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
//!     }
//!
//!     fn update(&mut self, engine: &mut Engine, dt: f32) {
//!         let cube = engine.world.get_mut(self.0.unwrap()).unwrap();
//!         cube.transform.rotation[1] += dt;
//!         engine.gui.text_at(16.0, 16.0, 20.0, pulga_engine::Color::rgb(1.0, 1.0, 1.0), "Spinning");
//!     }
//! }
//!
//! pulga_engine::run(WindowConfig::default(), Spinner(None)).unwrap();
//! ```

#![warn(missing_docs)]

pub mod gizmo;
pub mod history;
pub mod physics;
pub mod project;
pub mod render;
pub mod runner;
pub mod scene;
pub mod track;
pub mod view;
pub mod zone;

pub use pulga_gui_lib as gui;
pub use pulga_gui_lib::{Color, DrawCommand, Ui, font};
pub use pulga_win_lib::{Camera, Event, Key, MouseButton, WindowConfig, obj, primitives};

use std::collections::HashSet;

use pulga_gui_lib::{Input, Rect, UiKey, input::button};
use pulga_win_lib::{App, time::Instant};

use physics::Vec3;
use render::Renderer;
use scene::World;

/// Longest frame step handed to [`Game::update`], in seconds. Longer frames
/// (window drag, debugger pause) are treated as this long.
pub const MAX_DT: f32 = 0.05;

/// What a game implements. The engine calls these on the window thread.
///
/// Each frame the engine measures `dt` (at most [`MAX_DT`]), starts a UI
/// frame on [`Engine::gui`], calls [`Game::update`], then draws the world
/// from the camera with the UI on top.
pub trait Game {
    /// Called once, after the window and GL context exist: upload meshes,
    /// spawn the initial scene, place the camera.
    fn init(&mut self, engine: &mut Engine);

    /// Called every frame with the seconds elapsed since the last one.
    /// Move things, build the UI.
    fn update(&mut self, engine: &mut Engine, dt: f32);

    /// Called for every input/window event, after the engine has updated
    /// what it tracks (window size, camera aspect, mouse, held keys, UI
    /// input). Check [`Ui::wants_pointer`] / [`Ui::wants_keyboard`] on
    /// [`Engine::gui`] to leave events the UI uses alone. Default: ignore.
    fn event(&mut self, engine: &mut Engine, event: Event) {
        let _ = (engine, event);
    }
}

/// Everything a game can reach from its callbacks.
pub struct Engine {
    /// The scene.
    pub world: World,
    /// The camera the world is drawn from. Its aspect ratio follows the
    /// window automatically.
    pub camera: Camera,
    /// GPU resources: meshes, clear color, fog.
    pub renderer: Renderer,
    /// The immediate-mode UI, drawn over the world. A frame is open during
    /// [`Game::update`].
    pub gui: Ui,
    input: Input,
    keys: HashSet<Key>,
    screen: (u32, u32),
    quit: bool,
}

impl Engine {
    /// Window size in pixels, as `(width, height)`.
    pub fn screen_size(&self) -> (f32, f32) {
        (self.screen.0 as f32, self.screen.1 as f32)
    }

    /// Last known cursor position, in pixels from the window's top-left.
    pub fn mouse(&self) -> (f32, f32) {
        self.input.mouse
    }

    /// Whether `key` is held down.
    pub fn key_down(&self, key: Key) -> bool {
        self.keys.contains(&key)
    }

    /// The world-space ray under the cursor, as `(origin, unit direction)`.
    /// Feed it to [`physics`] to find what the mouse points at.
    pub fn mouse_ray(&self) -> (Vec3, Vec3) {
        self.ray_at(self.input.mouse.0, self.input.mouse.1)
    }

    /// The world-space ray under window pixel (`x`, `y`).
    pub fn ray_at(&self, x: f32, y: f32) -> (Vec3, Vec3) {
        let (w, h) = self.screen_size();
        let ndc_x = x / w.max(1.0) * 2.0 - 1.0;
        let ndc_y = 1.0 - y / h.max(1.0) * 2.0;
        self.camera.ray(ndc_x, ndc_y)
    }

    /// Closes the window at the end of this frame; [`run`] then returns.
    pub fn quit(&mut self) {
        self.quit = true;
    }

    /// Records what `event` means for the UI's input and the held keys.
    fn track(&mut self, event: &Event) {
        let index = |b: &MouseButton| match b {
            MouseButton::Left => button::LEFT,
            MouseButton::Right => button::RIGHT,
            MouseButton::Middle => button::MIDDLE,
        };
        let input = &mut self.input;
        match event {
            Event::MouseMoved { x, y } => input.mouse = (*x, *y),
            Event::MouseDown(b) => {
                input.down[index(b)] = true;
                input.pressed[index(b)] = true;
            }
            Event::MouseUp(b) => {
                input.down[index(b)] = false;
                input.released[index(b)] = true;
            }
            Event::Wheel(lines) => input.wheel += lines,
            Event::Text(c) => input.text.push(*c),
            Event::KeyDown(key) => {
                self.keys.insert(*key);
                input.keys.extend(ui_key(*key));
            }
            Event::KeyUp(key) => {
                self.keys.remove(key);
            }
            _ => {}
        }
        input.shift = self.keys.contains(&Key::Shift);
        input.ctrl = self.keys.contains(&Key::Control);
    }
}

fn ui_key(key: Key) -> Option<UiKey> {
    Some(match key {
        Key::Left => UiKey::Left,
        Key::Right => UiKey::Right,
        Key::Up => UiKey::Up,
        Key::Down => UiKey::Down,
        Key::Home => UiKey::Home,
        Key::End => UiKey::End,
        Key::Backspace => UiKey::Backspace,
        Key::Delete => UiKey::Delete,
        Key::Enter => UiKey::Enter,
        Key::Escape => UiKey::Escape,
        Key::Tab => UiKey::Tab,
        _ => return None,
    })
}

struct Runner<G> {
    game: G,
    engine: Option<Engine>,
    initial_size: (u32, u32),
    last_frame: Instant,
}

impl<G: Game> App for Runner<G> {
    fn init(&mut self) {
        let (w, h) = self.initial_size;
        let mut engine = Engine {
            world: World::default(),
            camera: Camera::new(60f32.to_radians(), w as f32 / h.max(1) as f32, 0.1, 500.0),
            // SAFETY: App::init runs right after the GL context is made current.
            renderer: unsafe { Renderer::new() },
            gui: Ui::default(),
            input: Input::default(),
            keys: HashSet::new(),
            screen: (w, h),
            quit: false,
        };
        self.game.init(&mut engine);
        self.engine = Some(engine);
        self.last_frame = Instant::now();
    }

    fn event(&mut self, event: Event) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        if let Event::Resized { width, height } = event {
            engine.screen = (width, height);
            if height > 0 {
                engine.camera.set_aspect(width as f32 / height as f32);
            }
        }
        engine.track(&event);
        self.game.event(engine, event);
    }

    fn draw(&mut self) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(MAX_DT);
        self.last_frame = now;

        let (w, h) = engine.screen_size();
        engine.input.dt = dt;
        engine
            .gui
            .begin_frame(engine.input.clone(), Rect::new(0.0, 0.0, w, h));
        engine.input.end_frame();
        self.game.update(engine, dt);
        let ui = engine.gui.end_frame();
        engine
            .renderer
            .draw(&engine.world, &engine.camera, &ui, (w, h));
    }

    fn should_quit(&self) -> bool {
        self.engine.as_ref().is_some_and(|e| e.quit)
    }
}

/// Opens a window as described by `config` and runs `game` until it calls
/// [`Engine::quit`] or the window is closed.
pub fn run(
    config: WindowConfig,
    game: impl Game + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let runner = Runner {
        game,
        engine: None,
        initial_size: (config.width, config.height),
        last_frame: Instant::now(),
    };
    pulga_win_lib::run(config, runner)
}
