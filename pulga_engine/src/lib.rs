//! The 42run game engine.
//!
//! Sits between the platform layer ([`pulga_win_lib`]: window, input, GL
//! context, meshes, matrices) and a game. A game implements [`Game`] and
//! hands it to [`run`]. The engine owns the window loop, frame timing, the
//! scene ([`scene::World`]), the camera, the renderer and the HUD, so the
//! game never touches OpenGL.
//!
//! Each frame the engine:
//! 1. measures `dt` (clamped, so a hitch can't teleport anything),
//! 2. clears the [`hud::Hud`],
//! 3. calls [`Game::update`],
//! 4. draws the world from the camera, then the HUD on top.
//!
//! ```no_run
//! use pulga_engine::{Engine, Game, WindowConfig, primitives, scene::Entity};
//!
//! struct Spinner(Option<pulga_engine::scene::EntityId>);
//!
//! impl Game for Spinner {
//!     fn init(&mut self, engine: &mut Engine) {
//!         let cube = engine.renderer.add_mesh(&primitives::cube(1.0));
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

pub mod font;
pub mod hud;
pub mod render;
pub mod scene;

pub use pulga_gui_lib::Color;
pub use pulga_win_lib::{Camera, Event, Key, WindowConfig, obj, primitives};

use pulga_win_lib::{App, time::Instant};

use hud::Hud;
use render::Renderer;
use scene::World;

/// Longest frame step handed to [`Game::update`], in seconds. Longer frames
/// (window drag, debugger pause) are treated as this long.
pub const MAX_DT: f32 = 0.05;

/// What a game implements. The engine calls these on the window thread.
pub trait Game {
    /// Called once, after the window and GL context exist: upload meshes,
    /// spawn the initial scene, place the camera.
    fn init(&mut self, engine: &mut Engine);

    /// Called every frame with the seconds elapsed since the last one
    /// (at most [`MAX_DT`]). Move things, queue HUD text.
    fn update(&mut self, engine: &mut Engine, dt: f32);

    /// Called for every input/window event. The engine has already handled
    /// what it owns (window size, camera aspect) by then. Default: ignore.
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
    /// GPU resources: upload meshes, set the clear color.
    pub renderer: Renderer,
    /// This frame's on-screen text. Cleared before every [`Game::update`].
    pub hud: Hud,
    screen: (u32, u32),
    quit: bool,
}

impl Engine {
    /// Window size in pixels, as `(width, height)`.
    pub fn screen_size(&self) -> (f32, f32) {
        (self.screen.0 as f32, self.screen.1 as f32)
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
            camera: Camera::new(60f32.to_radians(), w as f32 / h.max(1) as f32, 0.1, 200.0),
            // SAFETY: App::init runs right after the GL context is made current.
            renderer: unsafe { Renderer::new() },
            hud: Hud::default(),
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
        self.game.event(engine, event);
    }

    fn draw(&mut self) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(MAX_DT);
        self.last_frame = now;

        engine.hud.clear();
        self.game.update(engine, dt);
        engine
            .renderer
            .draw(&engine.world, &engine.camera, &engine.hud, engine.screen);
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
