//! Run preview: the game's runner on a track repeating the selected zone or
//! group, right in the viewport.

use pulga_engine::project::{Item, Project};
use pulga_engine::render::Fog;
use pulga_engine::runner::{Death, Runner, RunnerConfig};
use pulga_engine::scene::{Entity, EntityId, Transform};
use pulga_engine::track::{Source, Track};
use pulga_engine::view::ChaseCamera;
use pulga_engine::{Color, Engine, Key};

use super::Editor;

/// Preview running speed, world units per second.
const SPEED: f32 = 9.0;
const SKY: Color = Color::rgb(0.55, 0.72, 0.9);

pub struct Preview {
    project: Project,
    item: Item,
    track: Option<Track>,
    runner: Runner,
    chase: ChaseCamera,
    body: EntityId,
    seed: u64,
    /// Steering/jumps pressed since the last update.
    actions: Vec<i32>,
}

impl Preview {
    fn start(engine: &mut Engine, project: Project, item: Item) -> Self {
        let cube = engine.renderer.cube();
        let body = engine
            .world
            .spawn(Entity::with_mesh(cube).with_color(Color::hex(0xff9f1c)));
        let mut preview = Self {
            project,
            item,
            track: None,
            runner: Runner::new(RunnerConfig::default()),
            chase: ChaseCamera::default(),
            body,
            seed: 1,
            actions: Vec::new(),
        };
        preview.restart(engine);
        preview
    }

    /// Starts over on a fresh track.
    pub fn restart(&mut self, engine: &mut Engine) {
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
            Source::Repeat(self.item),
            self.seed,
            &mut engine.world,
            cube,
        ));
        self.runner = Runner::new(RunnerConfig::default());
        self.chase.reset(&self.runner);
        self.actions.clear();
    }

    /// Queues the action a key means while previewing.
    pub fn key(&mut self, key: Key) {
        match key {
            Key::Left | Key::Char('a') => self.actions.push(-1),
            Key::Right | Key::Char('d') => self.actions.push(1),
            Key::Space | Key::Up | Key::Char('w') => self.actions.push(0),
            _ => {}
        }
    }

    pub fn death(&self) -> Option<Death> {
        self.runner.death
    }

    pub fn distance(&self) -> f32 {
        self.runner.distance
    }

    fn update(&mut self, engine: &mut Engine, dt: f32) {
        let track = self.track.as_mut().expect("a preview always has a track");
        for action in self.actions.drain(..) {
            match action {
                0 => {
                    self.runner.jump();
                }
                dir => {
                    self.runner.steer(dir, track);
                }
            }
        }
        self.runner.update(dt, SPEED, track);
        track.update(&mut engine.world, self.runner.position);

        let c = self.runner.config;
        let [x, y, z] = self.runner.position;
        if let Some(body) = engine.world.get_mut(self.body) {
            body.transform =
                Transform::at([x, y + c.height / 2.0, z]).with_scale([c.width, c.height, c.width]);
        }
        self.chase.follow(&mut engine.camera, &self.runner, dt);
    }

    fn stop(mut self, engine: &mut Engine) {
        if let Some(track) = self.track.take() {
            track.clear(&mut engine.world);
        }
        engine.world.despawn(self.body);
    }
}

impl Editor {
    pub(super) fn start_preview(&mut self, engine: &mut Engine) {
        let Some(item) = self.selection.item() else {
            engine.gui.notify(
                "Select a zone or group to preview",
                pulga_engine::gui::ToastKind::Info,
            );
            return;
        };
        if self.project.expand(item, &mut |_| 0).is_empty() {
            engine.gui.notify(
                "This group is empty: add zones to it first",
                pulga_engine::gui::ToastKind::Info,
            );
            return;
        }
        self.clear_view(engine);
        self.drag = None;
        self.looking = false;
        self.preview = Some(Preview::start(engine, self.project.clone(), item));
    }

    pub(super) fn stop_preview(&mut self, engine: &mut Engine) {
        if let Some(preview) = self.preview.take() {
            preview.stop(engine);
        }
        self.view.stale = true;
    }

    pub(super) fn update_preview(&mut self, engine: &mut Engine, dt: f32) {
        engine.renderer.clear_color = SKY;
        engine.renderer.fog = Some(Fog {
            color: SKY,
            density: 0.018,
        });
        if let Some(preview) = self.preview.as_mut() {
            preview.update(engine, dt);
        }
    }
}
