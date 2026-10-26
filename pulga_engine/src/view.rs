//! Camera rigs: a free-flying camera for editing and a chase camera that
//! follows a [`Runner`].

use std::f32::consts::FRAC_PI_2;

use crate::Camera;
use crate::physics::Vec3;
use crate::runner::Runner;

/// A camera moved like in most 3D editors: look around (yaw/pitch) and fly
/// along where it looks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyCamera {
    /// Eye position.
    pub position: Vec3,
    /// Turn around +y in radians; 0 looks down -z, positive turns left.
    pub yaw: f32,
    /// Look up (positive) or down, in radians.
    pub pitch: f32,
}

/// Radians per pixel of mouse movement.
const LOOK_SENSITIVITY: f32 = 0.004;

impl FlyCamera {
    /// A camera at `eye` looking toward `target`.
    pub fn looking_at(eye: Vec3, target: Vec3) -> Self {
        let d = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
        let flat = (d[0] * d[0] + d[2] * d[2]).sqrt();
        Self {
            position: eye,
            yaw: (-d[0]).atan2(-d[2]),
            pitch: d[1].atan2(flat),
        }
    }

    /// Unit vector the camera looks along.
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [-sy * cp, sp, -cy * cp]
    }

    /// Unit vector to the camera's right (horizontal).
    pub fn right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        [cy, 0.0, -sy]
    }

    /// Turns by a mouse movement of (`dx`, `dy`) pixels.
    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * LOOK_SENSITIVITY;
        self.pitch = (self.pitch - dy * LOOK_SENSITIVITY).clamp(-1.5, 1.5);
    }

    /// Moves `forward`, `right` and `up` units (along the view, the view's
    /// right, and world up).
    pub fn fly(&mut self, forward: f32, right: f32, up: f32) {
        let (f, r) = (self.forward(), self.right());
        for i in 0..3 {
            self.position[i] += f[i] * forward + r[i] * right;
        }
        self.position[1] += up;
    }

    /// Keeps the angles and backs off so `target` sits `distance` ahead.
    pub fn focus(&mut self, target: Vec3, distance: f32) {
        let f = self.forward();
        self.position = [0, 1, 2].map(|i| target[i] - f[i] * distance);
    }

    /// Points `camera` the way this rig looks.
    pub fn apply(&self, camera: &mut Camera) {
        let f = self.forward();
        let target = [0, 1, 2].map(|i| self.position[i] + f[i]);
        camera.look_at(self.position, target, [0.0, 1.0, 0.0]);
    }
}

/// Follows a runner from behind and above, swinging smoothly through turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChaseCamera {
    yaw: f32,
    /// How far behind the runner.
    pub distance: f32,
    /// How far above the runner's feet.
    pub height: f32,
}

impl Default for ChaseCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            distance: 7.5,
            height: 4.2,
        }
    }
}

impl ChaseCamera {
    /// Snaps behind `runner` without swinging (e.g. on restart).
    pub fn reset(&mut self, runner: &Runner) {
        self.yaw = runner.heading as f32 * FRAC_PI_2;
    }

    /// Moves `camera` toward its spot behind `runner`, `dt` seconds on.
    pub fn follow(&mut self, camera: &mut Camera, runner: &Runner, dt: f32) {
        let target_yaw = runner.heading as f32 * FRAC_PI_2;
        self.yaw += (target_yaw - self.yaw) * (1.0 - (-7.0 * dt).exp());
        let (s, c) = self.yaw.sin_cos();
        let forward = [-s, 0.0, -c];
        let p = runner.position;
        let eye = [
            p[0] - forward[0] * self.distance,
            p[1] + self.height,
            p[2] - forward[2] * self.distance,
        ];
        let look = [p[0] + forward[0] * 9.0, p[1] + 0.8, p[2] + forward[2] * 9.0];
        camera.look_at(eye, look, [0.0, 1.0, 0.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5)
    }

    #[test]
    fn looking_at_round_trips_through_forward() {
        let cam = FlyCamera::looking_at([0.0, 5.0, 5.0], [0.0, 0.0, 0.0]);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!(close(cam.forward(), [0.0, -s, -s]), "{:?}", cam.forward());
        let side = FlyCamera::looking_at([0.0; 3], [-1.0, 0.0, 0.0]);
        assert!(close(side.forward(), [-1.0, 0.0, 0.0]));
        assert!(close(side.right(), [0.0, 0.0, -1.0]));
    }

    #[test]
    fn fly_and_focus() {
        let mut cam = FlyCamera::looking_at([0.0; 3], [0.0, 0.0, -1.0]);
        cam.fly(2.0, 1.0, 3.0);
        assert!(close(cam.position, [1.0, 3.0, -2.0]));
        cam.focus([0.0; 3], 10.0);
        assert!(close(cam.position, [0.0, 0.0, 10.0]));
    }

    #[test]
    fn chase_swings_to_the_new_heading() {
        let mut runner = Runner::new(Default::default());
        let mut chase = ChaseCamera::default();
        let mut camera = Camera::new(1.0, 1.0, 0.1, 100.0);
        runner.heading = 1;
        for _ in 0..200 {
            chase.follow(&mut camera, &runner, 1.0 / 60.0);
        }
        // Facing -x, the camera sits on the +x side of the runner.
        assert!(
            camera.eye()[0] > runner.position[0] + 6.0,
            "{:?}",
            camera.eye()
        );
    }
}
