//! `42run` — a Temple Run–style endless runner (see `../subject.en.pdf`).
//!
//! This crate is a `pulga_engine` project: `project.toml` holds the zones and
//! groups designed in `pulga_editor` and is embedded at compile time; this
//! file is the game's rules (free mode). The track, the runner and its
//! collisions come from the engine:
//!
//! - the track glues the project's free-mode pool at random, forks included
//!   ([`pulga_engine::track`]),
//! - Left/Right change lane, or turn when at a junction; missing a turn runs
//!   off the floor ([`pulga_engine::runner`]),
//! - solid props are obstacles: jump them, dodge them, or crash.

use pulga_engine::gui::{DrawCommand, Rect};
use pulga_engine::project::Project;
use pulga_engine::render::Fog;
use pulga_engine::runner::{Death, Runner, RunnerConfig};
use pulga_engine::scene::{Entity, EntityId, Transform};
use pulga_engine::track::{Source, Track};
use pulga_engine::view::ChaseCamera;
use pulga_engine::{Color, Engine, Event, Game, Key, WindowConfig, font};

const PROJECT: &str = include_str!("../project.toml");

const BASE_SPEED: f32 = 9.0; // world units/sec
const SPEED_RAMP_PER_SEC: f32 = 0.12; // the run gets gradually faster
const MAX_SPEED: f32 = 24.0;

const SKY: Color = Color::rgb(0.55, 0.72, 0.9);
const RUNNER_COLOR: Color = Color::rgb(1.0, 0.62, 0.11);
const TEXT: Color = Color::rgb(1.0, 1.0, 1.0);
const SHADOW: Color = Color::rgba(0.05, 0.07, 0.12, 0.65);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Action {
    Steer(i32),
    Jump,
}

struct Run {
    project: Project,
    track: Option<Track>,
    runner: Runner,
    chase: ChaseCamera,
    body: Option<EntityId>,
    speed: f32,
    seed: u64,
    best: f32,
    actions: Vec<Action>,
}

impl Run {
    fn new(project: Project) -> Result<Self, String> {
        if project.roots().is_empty() {
            return Err("project.toml has no zones in its free-mode pool".into());
        }
        Ok(Self {
            project,
            track: None,
            runner: Runner::new(RunnerConfig::default()),
            chase: ChaseCamera::default(),
            body: None,
            speed: BASE_SPEED,
            seed: 0x42,
            best: 0.0,
            actions: Vec::new(),
        })
    }

    fn reset(&mut self, engine: &mut Engine) {
        if let Some(track) = self.track.take() {
            track.clear(&mut engine.world);
        }
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let cube = engine.renderer.cube();
        self.track = Some(Track::new(
            self.project.clone(),
            Source::Pool,
            self.seed,
            &mut engine.world,
            cube,
        ));
        self.runner = Runner::new(RunnerConfig::default());
        self.chase.reset(&self.runner);
        self.speed = BASE_SPEED;
        self.actions.clear();
    }

    fn simulate(&mut self, engine: &mut Engine, dt: f32) {
        let track = self.track.as_mut().expect("reset creates the track");
        for action in self.actions.drain(..) {
            match action {
                Action::Steer(dir) => {
                    self.runner.steer(dir, track);
                }
                Action::Jump => {
                    self.runner.jump();
                }
            }
        }
        self.speed = (self.speed + SPEED_RAMP_PER_SEC * dt).min(MAX_SPEED);
        if self.runner.update(dt, self.speed, track).is_some() {
            self.best = self.best.max(self.runner.distance);
        }
        track.update(&mut engine.world, self.runner.position);
    }

    fn sync_scene(&mut self, engine: &mut Engine, dt: f32) {
        let c = self.runner.config;
        let [x, y, z] = self.runner.position;
        if let Some(body) = self.body.and_then(|b| engine.world.get_mut(b)) {
            body.transform =
                Transform::at([x, y + c.height / 2.0, z]).with_scale([c.width, c.height, c.width]);
        }
        self.chase.follow(&mut engine.camera, &self.runner, dt);
    }

    fn draw_hud(&self, engine: &mut Engine) {
        let (w, h) = engine.screen_size();
        let ui = &mut engine.gui;
        let shadowed =
            |ui: &mut pulga_engine::Ui, x: f32, y: f32, size: f32, color: Color, text: &str| {
                let o = size / 10.0;
                ui.text_at(x + o, y + o, size, SHADOW, text);
                ui.text_at(x, y, size, color, text);
            };
        shadowed(
            ui,
            28.0,
            24.0,
            40.0,
            TEXT,
            &format!("{} m", self.runner.distance as i32),
        );
        if self.best > 0.0 {
            shadowed(
                ui,
                30.0,
                72.0,
                20.0,
                TEXT.with_alpha(0.8),
                &format!("Best {} m", self.best as i32),
            );
        }
        let Some(death) = self.runner.death else {
            return;
        };
        let theme = ui.theme.clone();
        let (title, reason) = (
            "GAME OVER",
            match death {
                Death::Crash => "You crashed into an obstacle.",
                Death::Fall => "You fell off the track.",
            },
        );
        let prompt = "Press Space to run again";
        let card_w = font::text_width(title, 60.0).max(font::text_width(reason, 20.0)) + 100.0;
        let card = Rect::new((w - card_w) / 2.0, h / 2.0 - 110.0, card_w, 200.0);
        ui.paint(DrawCommand::Shadow {
            rect: card,
            radius: 14.0,
            blur: 30.0,
            color: theme.shadow,
        });
        ui.paint(DrawCommand::RoundedRect {
            rect: card,
            radius: 14.0,
            fill: theme.panel.with_alpha(0.94),
            border: 1.0,
            border_color: theme.border,
        });
        let center = |text: &str, size: f32| card.x + (card.w - font::text_width(text, size)) / 2.0;
        ui.text_at(
            center(title, 60.0),
            card.y + 26.0,
            60.0,
            theme.danger,
            title,
        );
        ui.text_at(
            center(reason, 20.0),
            card.y + 100.0,
            20.0,
            theme.text,
            reason,
        );
        ui.text_at(
            center(prompt, 20.0),
            card.y + 140.0,
            20.0,
            theme.text_dim,
            prompt,
        );
    }
}

impl Game for Run {
    fn init(&mut self, engine: &mut Engine) {
        engine.renderer.clear_color = SKY;
        engine.renderer.fog = Some(Fog {
            color: SKY,
            density: 0.016,
        });
        let cube = engine.renderer.cube();
        self.body = Some(
            engine
                .world
                .spawn(Entity::with_mesh(cube).with_color(RUNNER_COLOR)),
        );
        self.reset(engine);
    }

    fn event(&mut self, engine: &mut Engine, event: Event) {
        let alive = self.runner.death.is_none();
        match event {
            Event::KeyDown(Key::Escape) => engine.quit(),
            Event::KeyDown(Key::Left | Key::Char('a')) if alive => {
                self.actions.push(Action::Steer(-1))
            }
            Event::KeyDown(Key::Right | Key::Char('d')) if alive => {
                self.actions.push(Action::Steer(1))
            }
            Event::KeyDown(Key::Space | Key::Up | Key::Char('w')) if alive => {
                self.actions.push(Action::Jump)
            }
            Event::KeyDown(Key::Space | Key::Enter) => self.reset(engine),
            _ => {}
        }
    }

    fn update(&mut self, engine: &mut Engine, dt: f32) {
        if self.runner.death.is_none() {
            self.simulate(engine, dt);
        }
        self.sync_scene(engine, dt);
        self.draw_hud(engine);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let run = Run::new(Project::from_toml(PROJECT)?)?;
    pulga_engine::run(
        WindowConfig {
            title: "42run".into(),
            width: 1280,
            height: 720,
        },
        run,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_project_is_playable() {
        let run = Run::new(Project::from_toml(PROJECT).unwrap()).unwrap();
        assert!(!run.project.roots().is_empty());
    }
}
