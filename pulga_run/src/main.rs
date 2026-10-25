//! `42run` — a Temple Run–style endless runner (see `../42Run/subject.en.pdf`).
//! First playable slice: lateral movement, jump, scrolling obstacles,
//! collision, distance meter. No campus-themed set or textures yet — see
//! the README for what's still missing.

use pulga_win_lib::time::Instant;
use pulga_win_lib::{
    App, Camera, Event, Key, WindowConfig, gl,
    gl_mesh::GlMesh,
    obj::Mesh,
    primitives::{cube, ground_plane},
    run, shader, transform,
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

fn lane_x(lane: i32) -> f32 {
    lane as f32 * LANE_WIDTH
}

struct Obstacle {
    lane: i32,
    z: f32,
}

struct Game {
    camera: Camera,
    program: gl::types::GLuint,
    proj_loc: gl::types::GLint,
    view_loc: gl::types::GLint,
    model_loc: gl::types::GLint,
    cube_mesh: Option<GlMesh>,
    ground_mesh: Option<GlMesh>,

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

    last_frame: Instant,
    shown_distance: i32,
    pending_title: Option<String>,
}

impl Game {
    fn new() -> Self {
        let mut camera = Camera::new(60f32.to_radians(), 16.0 / 9.0, 0.1, 100.0);
        camera.look_at([0.0, 3.2, 6.0], [0.0, 1.0, -6.0], [0.0, 1.0, 0.0]);
        Self {
            camera,
            program: 0,
            proj_loc: -1,
            view_loc: -1,
            model_loc: -1,
            cube_mesh: None,
            ground_mesh: None,
            player_lane: 0,
            player_x: 0.0,
            player_y: 0.0,
            player_vel_y: 0.0,
            on_ground: true,
            obstacles: Vec::new(),
            distance: 0.0,
            distance_since_spawn: 0.0,
            speed: BASE_SPEED,
            game_over: false,
            last_frame: Instant::now(),
            shown_distance: -1,
            pending_title: None,
        }
    }

    fn spawn_obstacle(&mut self) {
        let lane = rand::rng().random_range(-(LANE_COUNT / 2)..=(LANE_COUNT / 2));
        self.obstacles.push(Obstacle { lane, z: SPAWN_Z });
    }

    fn reset(&mut self) {
        self.player_lane = 0;
        self.player_x = 0.0;
        self.player_y = 0.0;
        self.player_vel_y = 0.0;
        self.on_ground = true;
        self.obstacles.clear();
        self.distance = 0.0;
        self.distance_since_spawn = 0.0;
        self.speed = BASE_SPEED;
        self.game_over = false;
        self.shown_distance = -1;
    }

    fn draw_cube(&self, model: &transform::Mat4) {
        unsafe {
            gl::UniformMatrix4fv(
                self.model_loc,
                1,
                gl::TRUE,
                transform::to_gl(model).as_ptr(),
            );
            self.cube_mesh.as_ref().unwrap().draw();
        }
    }
}

impl App for Game {
    fn init(&mut self) {
        let vert_src = include_str!("../../pulga_win_lib/assets/shaders/mesh.vert");
        let frag_src = include_str!("../../pulga_win_lib/assets/shaders/mesh.frag");
        unsafe {
            self.program =
                shader::link_program(vert_src, frag_src).expect("shader compile/link failed");
            self.proj_loc = shader::uniform_location(self.program, "proj");
            self.view_loc = shader::uniform_location(self.program, "view");
            self.model_loc = shader::uniform_location(self.program, "model");

            let cube_mesh: Mesh = cube(1.0);
            self.cube_mesh = Some(GlMesh::upload(&cube_mesh));
            let ground: Mesh = ground_plane(LANE_WIDTH * (LANE_COUNT as f32 + 1.0), 400.0);
            self.ground_mesh = Some(GlMesh::upload(&ground));

            gl::Enable(gl::DEPTH_TEST);
        }
        self.last_frame = Instant::now();
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::KeyDown(Key::Escape) => std::process::exit(0),
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
            Event::KeyDown(Key::Space) if self.game_over => self.reset(),
            Event::Resized { width, height } if height > 0 => {
                self.camera.set_aspect(width as f32 / height as f32);
            }
            _ => {}
        }
    }

    fn title(&self) -> Option<String> {
        self.pending_title.clone()
    }

    fn draw(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05); // clamp hitches
        self.last_frame = now;

        if !self.game_over {
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
                self.spawn_obstacle();
            }

            for o in &mut self.obstacles {
                o.z += self.speed * dt;
            }
            self.obstacles.retain(|o| o.z < DESPAWN_Z);

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
            self.camera.look_at(
                [self.player_x, 3.2, 6.0],
                [self.player_x, 1.0, -6.0],
                [0.0, 1.0, 0.0],
            );
        }

        self.pending_title = if self.game_over {
            (self.shown_distance >= 0).then(|| {
                self.shown_distance = -1; // force one more refresh if the player restarts
                format!("42run — GAME OVER — distance: {}", self.distance as i32)
            })
        } else {
            let whole = self.distance as i32;
            (whole != self.shown_distance).then(|| {
                self.shown_distance = whole;
                format!("42run — distance: {whole}")
            })
        };

        unsafe {
            gl::ClearColor(0.53, 0.75, 0.92, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            gl::UseProgram(self.program);
            gl::UniformMatrix4fv(
                self.proj_loc,
                1,
                gl::TRUE,
                transform::to_gl(&self.camera.proj()).as_ptr(),
            );
            gl::UniformMatrix4fv(
                self.view_loc,
                1,
                gl::TRUE,
                transform::to_gl(&self.camera.view()).as_ptr(),
            );

            // Ground.
            let ground_model = transform::translate([0.0, 0.0, -180.0]);
            gl::UniformMatrix4fv(
                self.model_loc,
                1,
                gl::TRUE,
                transform::to_gl(&ground_model).as_ptr(),
            );
            self.ground_mesh.as_ref().unwrap().draw();

            // Player.
            let player_model =
                transform::translate([self.player_x, PLAYER_SIZE / 2.0 + self.player_y, 0.0])
                    .mul_mat(&transform::scale([PLAYER_SIZE, PLAYER_SIZE, PLAYER_SIZE]));
            self.draw_cube(&player_model);

            // Obstacles.
            for o in &self.obstacles {
                let model =
                    transform::translate([lane_x(o.lane), OBSTACLE_SIZE / 2.0, o.z]).mul_mat(
                        &transform::scale([OBSTACLE_SIZE, OBSTACLE_SIZE, OBSTACLE_SIZE]),
                    );
                self.draw_cube(&model);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run(
        WindowConfig {
            title: "42run".into(),
            width: 1280,
            height: 720,
        },
        Game::new(),
    )
}
