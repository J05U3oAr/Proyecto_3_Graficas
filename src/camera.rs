use nalgebra_glm::Vec3;
use std::f32::consts::PI;

#[derive(Clone, Copy)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

impl Camera {
    pub const fn new() -> Self {
        Self {
            yaw: -0.42,
            pitch: 0.92,
            zoom: 1.0,
        }
    }

    pub fn orbit(&mut self, yaw_delta: f32, pitch_delta: f32) {
        self.yaw = (self.yaw + yaw_delta).rem_euclid(2.0 * PI);
        self.pitch = (self.pitch + pitch_delta).clamp(-PI / 2.0 + 0.08, PI / 2.0 - 0.08);
    }

    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let backward = Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        );
        let forward = -backward;
        let right = forward.cross(&Vec3::new(0.0, 1.0, 0.0)).normalize();
        let up = right.cross(&forward).normalize();
        (right, up, forward)
    }
}
