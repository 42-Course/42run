//! `42run` — a Temple Run–style endless runner (see `../subject.en.pdf`).
//! First playable slice: lateral movement, jump, scrolling obstacles,
//! collision, distance meter. No campus-themed set or textures yet — see
//! the README for what's still missing.
//!
//! All rendering, timing and windowing live in `pulga_engine`; this crate is
//! only the game's rules.

use pulga_engine::{
    Color, Engine, Event, Game, Key, WindowConfig, primitives,
    render::MeshId,
    scene::{Entity, EntityId, Transform},
};
use rand::RngExt;

const LANE_WIDTH: f32 = 2.2;
const LANE_COUNT: i32 = 3; // lanes at x = -LANE_WIDTH, 0, +LANE_WIDTH
const PLAYER_SIZE: f32 = 1.0;
const OBSTACLE_SIZE: f32 = 1.0;
const GRAVITY: f32 = -24.0;
const JUMP_VELOCITY: f32 = 9.0;
const BASE_SPEED: f32 = 7.0; // world units/sec the obstacles scroll toward the player
const SPEED_RAMP_PER_SEC: f32 = 0.08; // the run gets gradually faster
const SPAWN_Z: f32 = -45.0;
const DESPAWN_Z: f32 = 4.0;
const SPAWN_EVERY_DISTANCE: f32 = 7.0;
const LANE_SLIDE_SPEED: f32 = 12.0; // how fast the player slides to a new lane

const PLAYER_COLOR: Color = Color::rgb(0.95, 0.75, 0.2);
const OBSTACLE_COLOR: Color = Color::rgb(0.8, 0.25, 0.2);
const GROUND_COLOR: Color = Color::rgb(0.55, 0.55, 0.58);
const TEXT_COLOR: Color = Color::rgb(1.0, 1.0, 1.0);
const SHADOW_COLOR: Color = Color::rgb(0.1, 0.1, 0.15);

fn lane_x(lane: i32) -> f32 {
    lane as f32 * LANE_WIDTH
}

struct Obstacle {
    lane: i32,
    z: f32,
    entity: EntityId,
}

#[derive(Default)]
struct Run {
    cube: Option<MeshId>,
    player: Option<EntityId>,

    player_lane: i32,
    player_x: f32,
    player_y: f32,
    player_vel_y: f32,
    on_ground: bool,

    obstacles: Vec<Obstacle>,
    distance: f32,
    distance_since_spawn: f32,
    speed: f32,
    game_over: bool,
}

impl Run {
    fn spawn_obstacle(&mut self, engine: &mut Engine) {
        let lane = rand::rng().random_range(-(LANE_COUNT / 2)..=(LANE_COUNT / 2));
        let entity = engine.world.spawn(
            Entity::with_mesh(self.cube.unwrap())
                .with_color(OBSTACLE_COLOR)
                .with_transform(self.obstacle_transform(lane, SPAWN_Z)),
        );
        self.obstacles.push(Obstacle {
            lane,
            z: SPAWN_Z,
            entity,
        });
    }

    fn obstacle_transform(&self, lane: i32, z: f32) -> Transform {
        Transform::at([lane_x(lane), OBSTACLE_SIZE / 2.0, z]).with_scale([OBSTACLE_SIZE; 3])
    }

    fn reset(&mut self, engine: &mut Engine) {
        for o in self.obstacles.drain(..) {
            engine.world.despawn(o.entity);
        }
        self.player_lane = 0;
        self.player_x = 0.0;
        self.player_y = 0.0;
        self.player_vel_y = 0.0;
        self.on_ground = true;
        self.distance = 0.0;
        self.distance_since_spawn = 0.0;
        self.speed = BASE_SPEED;
        self.game_over = false;
    }

    fn simulate(&mut self, engine: &mut Engine, dt: f32) {
        self.speed += SPEED_RAMP_PER_SEC * dt;
        self.distance += self.speed * dt;
        self.distance_since_spawn += self.speed * dt;

        // Slide toward the target lane.
        let target_x = lane_x(self.player_lane);
        let max_step = LANE_SLIDE_SPEED * dt;
        self.player_x += (target_x - self.player_x).clamp(-max_step, max_step);

        // Jump arc.
        self.player_vel_y += GRAVITY * dt;
        self.player_y = (self.player_y + self.player_vel_y * dt).max(0.0);
        if self.player_y <= 0.0 {
            self.player_y = 0.0;
            self.player_vel_y = 0.0;
            self.on_ground = true;
        }

        if self.distance_since_spawn >= SPAWN_EVERY_DISTANCE {
            self.distance_since_spawn = 0.0;
            self.spawn_obstacle(engine);
        }

        for o in &mut self.obstacles {
            o.z += self.speed * dt;
        }
        for o in self.obstacles.extract_if(.., |o| o.z >= DESPAWN_Z) {
            engine.world.despawn(o.entity);
        }

        let clearance = PLAYER_SIZE * 0.6; // how high you must jump to clear an obstacle
        for o in &self.obstacles {
            let z_overlap = o.z.abs() < (PLAYER_SIZE + OBSTACLE_SIZE) / 2.0;
            let lane_match = o.lane == self.player_lane;
            let too_low = self.player_y < clearance;
            if z_overlap && lane_match && too_low {
                self.game_over = true;
            }
        }

        // Camera chases the player's lane.
        engine.camera.look_at(
            [self.player_x, 3.2, 6.0],
            [self.player_x, 1.0, -6.0],
            [0.0, 1.0, 0.0],
        );
    }

    /// Pushes gameplay state into the scene the engine draws.
    fn sync_scene(&self, engine: &mut Engine) {
        if let Some(player) = self.player.and_then(|p| engine.world.get_mut(p)) {
            player.transform =
                Transform::at([self.player_x, PLAYER_SIZE / 2.0 + self.player_y, 0.0])
                    .with_scale([PLAYER_SIZE; 3]);
        }
        for o in &self.obstacles {
            let transform = self.obstacle_transform(o.lane, o.z);
            if let Some(entity) = engine.world.get_mut(o.entity) {
                entity.transform = transform;
            }
        }
    }

    fn draw_hud(&self, engine: &mut Engine) {
        let shadowed = |engine: &mut Engine, x: f32, y: f32, size: f32, text: &str| {
            let offset = size / 8.0;
            engine
                .hud
                .text(x + offset, y + offset, size, SHADOW_COLOR, text);
            engine.hud.text(x, y, size, TEXT_COLOR, text);
        };
        shadowed(
            engine,
            24.0,
            24.0,
            32.0,
            &format!("DISTANCE {}", self.distance as i32),
        );
        if self.game_over {
            let (w, h) = engine.screen_size();
            for (text, size, dy) in [
                ("GAME OVER", 64.0, -48.0),
                ("PRESS SPACE TO RUN AGAIN", 24.0, 40.0),
            ] {
                let x = (w - pulga_engine::hud::Hud::text_width(text, size)) / 2.0;
                shadowed(engine, x, h / 2.0 + dy, size, text);
            }
        }
    }
}

impl Game for Run {
    fn init(&mut self, engine: &mut Engine) {
        let cube = engine.renderer.add_mesh(&primitives::cube(1.0));
        let ground = engine.renderer.add_mesh(&primitives::ground_plane(
            LANE_WIDTH * (LANE_COUNT as f32 + 1.0),
            400.0,
        ));
        engine.world.spawn(
            Entity::with_mesh(ground)
                .with_color(GROUND_COLOR)
                .with_transform(Transform::at([0.0, 0.0, -180.0])),
        );
        self.player = Some(
            engine
                .world
                .spawn(Entity::with_mesh(cube).with_color(PLAYER_COLOR)),
        );
        self.cube = Some(cube);
        self.reset(engine);
    }

    fn event(&mut self, engine: &mut Engine, event: Event) {
        match event {
            Event::KeyDown(Key::Escape) => engine.quit(),
            Event::KeyDown(Key::Left) if !self.game_over => {
                self.player_lane = (self.player_lane - 1).max(-(LANE_COUNT / 2));
            }
            Event::KeyDown(Key::Right) if !self.game_over => {
                self.player_lane = (self.player_lane + 1).min(LANE_COUNT / 2);
            }
            Event::KeyDown(Key::Space) if !self.game_over && self.on_ground => {
                self.player_vel_y = JUMP_VELOCITY;
                self.on_ground = false;
            }
            Event::KeyDown(Key::Space) if self.game_over => self.reset(engine),
            _ => {}
        }
    }

    fn update(&mut self, engine: &mut Engine, dt: f32) {
        if !self.game_over {
            self.simulate(engine, dt);
        }
        self.sync_scene(engine);
        self.draw_hud(engine);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pulga_engine::run(
        WindowConfig {
            title: "42run".into(),
            width: 1280,
            height: 720,
        },
        Run::default(),
    )
}
