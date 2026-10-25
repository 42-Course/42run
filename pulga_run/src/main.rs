//! `42run` — a Temple Run–style endless runner (see `../subject.en.pdf`).
//!
//! This crate is a `pulga_engine` project: `project.toml` holds the tiles
//! built in `pulga_editor` and is embedded at compile time; this file is the
//! game's rules. The track is glued from the project's straight tiles in
//! random order; obstacles are the props marked as such in the editor.
//! Turn tiles are skipped until the runner can turn.

use std::collections::VecDeque;

use pulga_engine::{
    Color, Engine, Event, Game, Key, WindowConfig, font,
    physics::Aabb,
    project::Project,
    scene::{Entity, EntityId, Transform},
    tile::{PropKind, TILE_SIZE, TileKind, spawn_tile},
    track::Placement,
};
use rand::RngExt;

const PROJECT: &str = include_str!("../project.toml");

const LANE_WIDTH: f32 = 2.2;
const LANE_COUNT: i32 = 3; // lanes at x = -LANE_WIDTH, 0, +LANE_WIDTH
const PLAYER_SIZE: f32 = 1.0;
const HITBOX: f32 = 0.8; // a bit smaller than the drawn cube, to be fair
const GRAVITY: f32 = -24.0;
const JUMP_VELOCITY: f32 = 9.0;
const BASE_SPEED: f32 = 7.0; // world units/sec
const SPEED_RAMP_PER_SEC: f32 = 0.08; // the run gets gradually faster
const LANE_SLIDE_SPEED: f32 = 12.0; // how fast the player slides to a new lane
const TILES_AHEAD: usize = 12;
const SAFE_START_TILES: usize = 2; // obstacle-free tiles before the first challenge

const PLAYER_COLOR: Color = Color::rgb(0.95, 0.75, 0.2);
const TEXT_COLOR: Color = Color::rgb(1.0, 1.0, 1.0);
const SHADOW_COLOR: Color = Color::rgb(0.1, 0.1, 0.15);

fn lane_x(lane: i32) -> f32 {
    lane as f32 * LANE_WIDTH
}

struct PlacedTile {
    root: EntityId,
    origin_z: f32,
    obstacles: Vec<Aabb>, // world space
}

struct Run {
    project: Project,
    straights: Vec<usize>,
    calm: Vec<usize>, // straights without obstacles
    tiles: VecDeque<PlacedTile>,
    placed: usize,
    player: Option<EntityId>,

    player_lane: i32,
    player_x: f32,
    player_y: f32,
    player_z: f32,
    player_vel_y: f32,
    on_ground: bool,

    speed: f32,
    game_over: bool,
}

impl Run {
    fn new(project: Project) -> Result<Self, String> {
        let straights: Vec<usize> = (0..project.tiles.len())
            .filter(|&i| project.tiles[i].kind == TileKind::Straight)
            .collect();
        if straights.is_empty() {
            return Err("project.toml has no straight tiles to build a track from".into());
        }
        let calm = straights
            .iter()
            .copied()
            .filter(|&i| {
                project.tiles[i]
                    .props
                    .iter()
                    .all(|p| p.kind != PropKind::Obstacle)
            })
            .collect();
        Ok(Self {
            project,
            straights,
            calm,
            tiles: VecDeque::new(),
            placed: 0,
            player: None,
            player_lane: 0,
            player_x: 0.0,
            player_y: 0.0,
            player_z: 0.0,
            player_vel_y: 0.0,
            on_ground: true,
            speed: BASE_SPEED,
            game_over: false,
        })
    }

    /// Glues the next tile onto the end of the track.
    fn place_next_tile(&mut self, engine: &mut Engine) {
        let pool = if self.placed < SAFE_START_TILES && !self.calm.is_empty() {
            &self.calm
        } else {
            &self.straights
        };
        let tile = pool[rand::rng().random_range(0..pool.len())];
        let placement = Placement {
            tile,
            origin: [0.0, 0.0, -(self.placed as f32) * TILE_SIZE],
            heading: 0,
        };
        let def = &self.project.tiles[tile];
        let cube = engine.renderer.cube();
        let root = spawn_tile(&mut engine.world, cube, def, placement.transform());
        let obstacles = def
            .props
            .iter()
            .filter(|p| p.kind == PropKind::Obstacle)
            .map(|p| Aabb::from_center_size(placement.to_world(p.position), p.size))
            .collect();
        self.tiles.push_back(PlacedTile {
            root,
            origin_z: placement.origin[2],
            obstacles,
        });
        self.placed += 1;
    }

    fn reset(&mut self, engine: &mut Engine) {
        for t in self.tiles.drain(..) {
            engine.world.despawn(t.root);
        }
        self.placed = 0;
        self.player_lane = 0;
        self.player_x = 0.0;
        self.player_y = 0.0;
        self.player_z = 0.0;
        self.player_vel_y = 0.0;
        self.on_ground = true;
        self.speed = BASE_SPEED;
        self.game_over = false;
        for _ in 0..TILES_AHEAD {
            self.place_next_tile(engine);
        }
    }

    fn simulate(&mut self, engine: &mut Engine, dt: f32) {
        self.speed += SPEED_RAMP_PER_SEC * dt;
        // ponytail: f32 positions lose precision far from the origin; rebase
        // the track around the player if runs ever reach ~100k units.
        self.player_z -= self.speed * dt;

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

        // Keep the track ahead filled, drop what's behind the camera.
        while self.tiles.len() < TILES_AHEAD
            || self.tiles.back().unwrap().origin_z > self.player_z - TILES_AHEAD as f32 * TILE_SIZE
        {
            self.place_next_tile(engine);
        }
        while let Some(t) = self.tiles.front()
            && t.origin_z - TILE_SIZE > self.player_z + 2.0 * TILE_SIZE
        {
            let t = self.tiles.pop_front().unwrap();
            engine.world.despawn(t.root);
        }

        let hitbox = Aabb::from_center_size(
            [
                self.player_x,
                self.player_y + PLAYER_SIZE / 2.0,
                self.player_z,
            ],
            [HITBOX; 3],
        );
        if self
            .tiles
            .iter()
            .flat_map(|t| &t.obstacles)
            .any(|o| o.intersects(&hitbox))
        {
            self.game_over = true;
        }
    }

    /// Pushes gameplay state into the scene the engine draws.
    fn sync_scene(&self, engine: &mut Engine) {
        if let Some(player) = self.player.and_then(|p| engine.world.get_mut(p)) {
            player.transform = Transform::at([
                self.player_x,
                PLAYER_SIZE / 2.0 + self.player_y,
                self.player_z,
            ])
            .with_scale([PLAYER_SIZE; 3]);
        }
        engine.camera.look_at(
            [self.player_x, 3.2, self.player_z + 6.0],
            [self.player_x, 1.0, self.player_z - 6.0],
            [0.0, 1.0, 0.0],
        );
    }

    fn draw_hud(&self, engine: &mut Engine) {
        let shadowed = |engine: &mut Engine, x: f32, y: f32, size: f32, text: &str| {
            let offset = size / 8.0;
            engine.text(x + offset, y + offset, size, SHADOW_COLOR, text);
            engine.text(x, y, size, TEXT_COLOR, text);
        };
        let distance = (-self.player_z) as i32;
        shadowed(engine, 24.0, 24.0, 32.0, &format!("DISTANCE {distance}"));
        if self.game_over {
            let (w, h) = engine.screen_size();
            for (text, size, dy) in [
                ("GAME OVER", 64.0, -48.0),
                ("PRESS SPACE TO RUN AGAIN", 24.0, 40.0),
            ] {
                let x = (w - font::text_width(text, size)) / 2.0;
                shadowed(engine, x, h / 2.0 + dy, size, text);
            }
        }
    }
}

impl Game for Run {
    fn init(&mut self, engine: &mut Engine) {
        let cube = engine.renderer.cube();
        self.player = Some(
            engine
                .world
                .spawn(Entity::with_mesh(cube).with_color(PLAYER_COLOR)),
        );
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
        assert!(!run.straights.is_empty());
    }
}
