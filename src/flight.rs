use crate::camera::Camera;
use nalgebra_glm::Vec3;

/// State for a free-flight session in an unbounded, wraparound star field.
pub struct Flight {
    pub position: Vec3,
    pub velocity: Vec3,
    pub camera: Camera,
}

impl Flight {
    pub fn new(camera: Camera) -> Self {
        Self {
            position: Vec3::zeros(),
            velocity: Vec3::zeros(),
            camera,
        }
    }

    pub fn advance(
        &mut self,
        dt: f32,
        turn: (f32, f32),
        thrust: (f32, f32, f32),
        boost: bool,
        brake: bool,
    ) {
        self.camera.orbit(turn.0 * dt * 1.5, turn.1 * dt * 1.5);
        let (right, up, forward) = self.camera.basis();
        let acceleration = forward * thrust.0 + right * thrust.1 + up * thrust.2;
        if acceleration.norm_squared() > 0.0 {
            let acceleration = acceleration.normalize() * if boost { 26.0 } else { 13.0 };
            self.velocity += acceleration * dt;
        }

        if brake {
            self.velocity *= (-5.0 * dt).exp();
        } else if acceleration.norm_squared() == 0.0 {
            self.velocity *= (-0.38 * dt).exp();
        }

        let max_speed = if boost { 72.0 } else { 36.0 };
        let speed = self.velocity.norm();
        if speed > max_speed {
            self.velocity *= max_speed / speed;
        }
        self.position += self.velocity * dt;
    }
}
