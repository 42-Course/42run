//! The 42run game engine.
//!
//! Built on the two libraries: [`pulga_win_lib`] (window, input, GL context,
//! meshes, matrices, drawing pipelines) and [`pulga_gui_lib`] (widgets,
//! text). The engine adds what makes a game out of those:
//!
//! - [`run`] and the [`Game`] trait: the frame loop and timing,
//! - [`scene`]: a [`scene::World`] of entities in a transform hierarchy,
//! - [`tile`] and [`track`]: the building blocks a run is glued from,
//! - [`project`]: what the editor saves and the game embeds,
//! - [`physics`]: bounding boxes and ray casts (mouse picking, hits),
//! - [`render`]: drawing the world and the UI each frame.
//!
//! The `pulga_editor` program (`src/bin/pulga_editor.rs`) is built with this
//! crate: it edits a project's tiles and previews runs. A game is a separate
//! crate that depends on this one and implements [`Game`]:
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
//!     }
//! }
//!
//! pulga_engine::run(WindowConfig::default(), Spinner(None)).unwrap();
//! ```

#![warn(missing_docs)]

pub mod physics;
pub mod project;
pub mod render;
pub mod scene;
pub mod tile;
pub mod track;

pub use pulga_gui_lib::{Color, DrawCommand, font};
pub use pulga_win_lib::{Camera, Event, Key, MouseButton, WindowConfig, obj, primitives};

use pulga_win_lib::{App, time::Instant};

use physics::Vec3;
use render::Renderer;
use scene::World;

/// Longest frame step handed to [`Game::update`], in seconds. Longer frames
/// (window drag, debugger pause) are treated as this long.
pub const MAX_DT: f32 = 0.05;

/// What a game implements. The engine calls these on the window thread.
///
/// Each frame the engine measures `dt` (at most [`MAX_DT`]), clears
/// [`Engine::ui`], calls [`Game::update`], then draws the world from the
/// camera with the UI on top.
pub trait Game {
    /// Called once, after the window and GL context exist: upload meshes,
    /// spawn the initial scene, place the camera.
    fn init(&mut self, engine: &mut Engine);

    /// Called every frame with the seconds elapsed since the last one.
    /// Move things, queue UI.
    fn update(&mut self, engine: &mut Engine, dt: f32);

    /// Called for every input/window event. The engine has already handled
    /// what it owns (window size, camera aspect, mouse position) by then.
    /// Default: ignore.
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
    /// GPU resources: meshes, clear color.
    pub renderer: Renderer,
    /// This frame's 2D overlay (HUD, panels), drawn over the world in
    /// order. Cleared before every [`Game::update`].
    pub ui: Vec<DrawCommand>,
    screen: (u32, u32),
    mouse: (f32, f32),
    quit: bool,
}

impl Engine {
    /// Window size in pixels, as `(width, height)`.
    pub fn screen_size(&self) -> (f32, f32) {
        (self.screen.0 as f32, self.screen.1 as f32)
    }

    /// Last known cursor position, in pixels from the window's top-left.
    pub fn mouse(&self) -> (f32, f32) {
        self.mouse
    }

    /// The world-space ray under the cursor, as `(origin, unit direction)`.
    /// Feed it to [`physics`] to find what the mouse points at.
    pub fn mouse_ray(&self) -> (Vec3, Vec3) {
        let (w, h) = self.screen_size();
        let ndc_x = self.mouse.0 / w.max(1.0) * 2.0 - 1.0;
        let ndc_y = 1.0 - self.mouse.1 / h.max(1.0) * 2.0;
        self.camera.ray(ndc_x, ndc_y)
    }

    /// Queues `text` on this frame's UI: top-left at (`x`, `y`) pixels,
    /// `size` pixels per line.
    pub fn text(&mut self, x: f32, y: f32, size: f32, color: Color, text: impl Into<String>) {
        self.ui.push(DrawCommand::Text {
            x,
            y,
            size,
            color,
            text: text.into(),
        });
    }

    /// Closes the window at the end of this frame; [`run`] then returns.
    pub fn quit(&mut self) {
        self.quit = true;
    }
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
            camera: Camera::new(60f32.to_radians(), w as f32 / h.max(1) as f32, 0.1, 400.0),
            // SAFETY: App::init runs right after the GL context is made current.
            renderer: unsafe { Renderer::new() },
            ui: Vec::new(),
            screen: (w, h),
            mouse: (0.0, 0.0),
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
        match event {
            Event::Resized { width, height } => {
                engine.screen = (width, height);
                if height > 0 {
                    engine.camera.set_aspect(width as f32 / height as f32);
                }
            }
            Event::MouseMoved { x, y } => engine.mouse = (x, y),
            _ => {}
        }
        self.game.event(engine, event);
    }

    fn draw(&mut self) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(MAX_DT);
        self.last_frame = now;

        engine.ui.clear();
        self.game.update(engine, dt);
        let screen = engine.screen_size();
        engine
            .renderer
            .draw(&engine.world, &engine.camera, &engine.ui, screen);
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
