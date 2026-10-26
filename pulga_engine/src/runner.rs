//! The runner: a character that runs along a [`Track`] on its own, Temple
//! Run style.
//!
//! The runner always moves forward along the current track centerline, in
//! one of [`RunnerConfig::lanes`] lanes. [`Runner::steer`] does what the
//! player expects from the same two keys: at a junction (see
//! [`Track::junction`]) it turns onto the exit, anywhere else it changes
//! lane. [`Runner::jump`] jumps.
//!
//! Collisions come from mass, not from special obstacle types: solid props
//! (see [`crate::zone::Prop::solid`]) hold the runner up, can be stepped onto
//! if low enough, and end the run when run into ([`Death::Crash`]). Where
//! there's no solid floor the runner falls, and the run ends below
//! [`RunnerConfig::kill_depth`] ([`Death::Fall`]): that's also what happens
//! when a turn is missed, since turn zones have no floor straight ahead.

use crate::physics::{Aabb, Vec3};
use crate::track::{Track, rotate_quarters};

/// Tuning knobs, in world units and seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunnerConfig {
    /// Number of lanes (odd, centered on the track).
    pub lanes: i32,
    /// Distance between lane centers.
    pub lane_width: f32,
    /// Hitbox width and depth.
    pub width: f32,
    /// Hitbox height.
    pub height: f32,
    /// Downward acceleration.
    pub gravity: f32,
    /// Upward speed at the start of a jump.
    pub jump_velocity: f32,
    /// How fast the runner slides sideways to its lane.
    pub lateral_speed: f32,
    /// Highest ledge walked onto without crashing.
    pub step_height: f32,
    /// Falling below this height ends the run.
    pub kill_depth: f32,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            lanes: 3,
            lane_width: 2.2,
            width: 0.8,
            height: 1.2,
            gravity: -24.0,
            jump_velocity: 9.0,
            lateral_speed: 12.0,
            step_height: 0.3,
            kill_depth: -6.0,
        }
    }
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Death {
    /// Ran into something solid.
    Crash,
    /// Fell off the track.
    Fall,
}

/// What [`Runner::steer`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Steer {
    /// Turned onto a junction's exit.
    Turned,
    /// Moved one lane over.
    ChangedLane,
    /// Already in the outermost lane.
    Blocked,
}

/// See the [module docs](self).
#[derive(Debug, Clone, PartialEq)]
pub struct Runner {
    /// Tuning.
    pub config: RunnerConfig,
    /// Feet position (bottom center of the hitbox).
    pub position: Vec3,
    /// Heading in quarter turns (0 = -z, 1 = -x, -1 = +x).
    pub heading: i32,
    /// A point on the current centerline.
    line_origin: Vec3,
    /// Lane index, 0 = center, negative = left.
    pub lane: i32,
    /// Vertical speed.
    pub velocity_y: f32,
    /// Standing on something solid.
    pub grounded: bool,
    /// Set once the run is over.
    pub death: Option<Death>,
    /// Distance run, in world units.
    pub distance: f32,
}

impl Runner {
    /// A runner standing at the start of a track (just inside its first
    /// zone), center lane, heading -z.
    pub fn new(config: RunnerConfig) -> Self {
        Self {
            config,
            position: [0.0, 0.0, -0.5],
            heading: 0,
            line_origin: [0.0; 3],
            lane: 0,
            velocity_y: 0.0,
            grounded: true,
            death: None,
            distance: 0.0,
        }
    }

    /// Unit vector the runner runs along.
    pub fn forward(&self) -> Vec3 {
        rotate_quarters([0.0, 0.0, -1.0], self.heading)
    }

    /// Unit vector to the runner's right.
    pub fn right(&self) -> Vec3 {
        rotate_quarters([1.0, 0.0, 0.0], self.heading)
    }

    /// The runner's collision box.
    pub fn hitbox(&self) -> Aabb {
        let [x, y, z] = self.position;
        let c = &self.config;
        Aabb::from_center_size([x, y + c.height / 2.0, z], [c.width, c.height, c.width])
    }

    /// Signed distance from the centerline, positive to the right.
    fn lateral(&self) -> f32 {
        let r = self.right();
        (0..3)
            .map(|i| (self.position[i] - self.line_origin[i]) * r[i])
            .sum()
    }

    /// Jumps if standing on something.
    pub fn jump(&mut self) -> bool {
        if self.death.is_none() && self.grounded {
            self.velocity_y = self.config.jump_velocity;
            self.grounded = false;
            return true;
        }
        false
    }

    /// Left (`direction < 0`) or right: turns at a junction, otherwise
    /// changes lane.
    pub fn steer(&mut self, direction: i32, track: &Track) -> Steer {
        if self.death.is_some() {
            return Steer::Blocked;
        }
        let turn = -direction.signum(); // left = +1 quarter turn
        if let Some(j) = track.junction(self.position, turn) {
            self.heading = j.heading;
            self.line_origin = j.world_exit;
            self.lane = 0;
            return Steer::Turned;
        }
        let max = self.config.lanes / 2;
        let lane = (self.lane + direction.signum()).clamp(-max, max);
        if lane == self.lane {
            return Steer::Blocked;
        }
        self.lane = lane;
        Steer::ChangedLane
    }

    /// Advances `dt` seconds at `speed`, colliding with the track's solids.
    /// Returns the death if this step ended the run.
    pub fn update(&mut self, dt: f32, speed: f32, track: &Track) -> Option<Death> {
        if self.death.is_some() {
            return None;
        }
        let solids = track.solids_near(self.position);
        let c = self.config;

        // Vertical: gravity, then land on / bump the underside of solids.
        let feet_before = self.position[1];
        self.velocity_y += c.gravity * dt;
        self.position[1] += self.velocity_y * dt;
        for s in &solids {
            if !self.hitbox().intersects(s) {
                continue;
            }
            if self.velocity_y <= 0.0 && feet_before >= s.max[1] - c.step_height {
                self.position[1] = s.max[1];
                self.velocity_y = 0.0;
            } else if self.velocity_y > 0.0 {
                self.position[1] = s.min[1] - c.height;
                self.velocity_y = 0.0;
            }
        }

        // Horizontal: forward, plus sliding toward the lane.
        let target = self.lane as f32 * c.lane_width;
        let slide = (target - self.lateral()).clamp(-c.lateral_speed * dt, c.lateral_speed * dt);
        let (f, r) = (self.forward(), self.right());
        for i in [0, 2] {
            self.position[i] += f[i] * speed * dt + r[i] * slide;
        }
        for s in &solids {
            if !self.hitbox().intersects(s) {
                continue;
            }
            if s.max[1] - self.position[1] <= c.step_height {
                self.position[1] = s.max[1]; // a low ledge: step onto it
            } else {
                self.death = Some(Death::Crash);
            }
        }
        self.distance += speed * dt;

        let mut probe = self.hitbox();
        probe.min[1] -= 0.05;
        probe.max[1] = probe.min[1] + 0.1;
        self.grounded = solids.iter().any(|s| probe.intersects(s));
        if self.position[1] < c.kill_depth {
            self.death = Some(Death::Fall);
        }
        self.death
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Item, Project};
    use crate::render::MeshId;
    use crate::scene::World;
    use crate::track::Source;
    use crate::zone::{Prop, ZoneKind};

    /// Seconds at 8 u/s from the runner's start (z = -0.5) to the tested
    /// zone's entry at z = -20, past the runway.
    const AHEAD: f32 = 19.5 / 8.0;

    /// A track repeating one zone (after the runway), a runner at its start.
    fn setup(build: impl FnOnce(&mut Project) -> u32) -> (Track, World, Runner) {
        let mut p = Project::new("p");
        let id = build(&mut p);
        p.add_zone("Hall", ZoneKind::Straight);
        let mut world = World::default();
        let track = Track::new(
            p,
            Source::Repeat(Item::Zone(id)),
            1,
            &mut world,
            MeshId::test_id(),
        );
        (track, world, Runner::new(RunnerConfig::default()))
    }

    /// Runs for `seconds` at `speed`, letting the track follow.
    fn run(
        r: &mut Runner,
        t: &mut Track,
        w: &mut World,
        seconds: f32,
        speed: f32,
    ) -> Option<Death> {
        let dt = 1.0 / 120.0;
        for _ in 0..(seconds / dt) as usize {
            if let Some(d) = r.update(dt, speed, t) {
                return Some(d);
            }
            t.update(w, r.position);
        }
        None
    }

    #[test]
    fn runs_down_an_empty_hall_forever() {
        let (mut t, mut w, mut r) = setup(|p| p.add_zone("Plain", ZoneKind::Straight));
        assert_eq!(run(&mut r, &mut t, &mut w, 10.0, 8.0), None);
        assert!(r.grounded);
        assert!((r.distance - 80.0).abs() < 0.5);
        assert!(r.position[2] < -75.0);
    }

    #[test]
    fn crashes_into_a_tall_solid_and_jumps_a_low_one() {
        let (mut t, mut w, mut r) = setup(|p| {
            let id = p.add_zone("Wall", ZoneKind::Straight);
            p.zone_mut(id).unwrap().props.push(Prop::solid(
                [0.0, 1.5, -5.0],
                [7.0, 3.0, 0.5],
                [1.0; 3],
            ));
            id
        });
        assert_eq!(
            run(&mut r, &mut t, &mut w, AHEAD + 2.0, 8.0),
            Some(Death::Crash)
        );

        let (mut t, mut w, mut r) = setup(|p| {
            let id = p.add_zone("Low", ZoneKind::Straight);
            p.zone_mut(id).unwrap().props.push(Prop::solid(
                [0.0, 0.35, -5.0],
                [7.0, 0.7, 0.5],
                [1.0; 3],
            ));
            id
        });
        // At 8 u/s the wall's near face (4.75 into the zone) comes ~0.6 s
        // after the zone's entry; jumping 0.25 s after the entry clears it.
        assert_eq!(run(&mut r, &mut t, &mut w, AHEAD + 0.25, 8.0), None);
        assert!(r.jump());
        assert_eq!(run(&mut r, &mut t, &mut w, 0.5, 8.0), None);
    }

    #[test]
    fn non_solid_props_are_passed_through() {
        let (mut t, mut w, mut r) = setup(|p| {
            let id = p.add_zone("Ghost", ZoneKind::Straight);
            let mut prop = Prop::solid([0.0, 1.5, -5.0], [7.0, 3.0, 0.5], [1.0; 3]);
            prop.solid = false;
            p.zone_mut(id).unwrap().props.push(prop);
            id
        });
        assert_eq!(run(&mut r, &mut t, &mut w, 3.0, 8.0), None);
    }

    #[test]
    fn steps_onto_a_low_ledge_and_runs_across_a_box_after_landing() {
        let (mut t, mut w, mut r) = setup(|p| {
            let id = p.add_zone("Step", ZoneKind::Straight);
            p.zone_mut(id).unwrap().props.push(Prop::solid(
                [0.0, 0.1, -5.0],
                [7.0, 0.2, 2.0],
                [1.0; 3],
            ));
            id
        });
        run(&mut r, &mut t, &mut w, AHEAD + 0.62, 8.0); // 5 into the zone
        assert!(
            (r.position[1] - 0.2).abs() < 1e-4,
            "on the step: {:?}",
            r.position
        );
        assert_eq!(run(&mut r, &mut t, &mut w, 2.0, 8.0), None);
    }

    #[test]
    fn lanes_change_and_stop_at_the_edge() {
        let (mut t, mut w, mut r) = setup(|p| p.add_zone("Plain", ZoneKind::Straight));
        assert_eq!(r.steer(-1, &t), Steer::ChangedLane);
        assert_eq!(r.steer(-1, &t), Steer::Blocked);
        run(&mut r, &mut t, &mut w, 0.5, 8.0);
        assert!((r.position[0] + 2.2).abs() < 1e-3, "{:?}", r.position);
    }

    #[test]
    fn turning_at_the_junction_survives_and_missing_it_falls() {
        // Repeating left turns recover with right turns, so the run continues.
        let left = |p: &mut Project| {
            p.add_zone("Right", ZoneKind::Right);
            p.add_zone("Left", ZoneKind::Left)
        };
        let (mut t, mut w, mut r) = setup(left);
        run(&mut r, &mut t, &mut w, AHEAD + 0.55, 8.0); // into the crossing
        assert_eq!(r.steer(-1, &t), Steer::Turned);
        assert_eq!(r.heading, 1);
        assert_eq!(run(&mut r, &mut t, &mut w, 0.8, 8.0), None);
        assert!(r.position[0] < -5.0, "went left: {:?}", r.position);

        let (mut t, mut w, mut r) = setup(left);
        assert_eq!(
            run(&mut r, &mut t, &mut w, AHEAD + 4.0, 8.0),
            Some(Death::Fall)
        );
    }

    #[test]
    fn steering_early_changes_lane_instead_of_turning() {
        let (t, _w, mut r) = setup(|p| p.add_zone("Left", ZoneKind::Left));
        assert_eq!(r.steer(-1, &t), Steer::ChangedLane);
        assert_eq!(r.heading, 0);
    }
}
