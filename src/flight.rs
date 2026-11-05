use crate::camera::Camera;
use nalgebra_glm::Vec3;

/// State for a free-flight session in an unbounded, wraparound star field.
pub struct Flight {
    pub position: Vec3,
    pub velocity: Vec3,
    pub camera: Camera,
    pub screen_offset: (f32, f32),
    last_mouse_position: Option<(f32, f32)>,
}

impl Flight {
    pub fn new(camera: Camera) -> Self {
        Self {
            position: Vec3::zeros(),
            velocity: Vec3::zeros(),
            camera,
            screen_offset: (0.0, 0.0),
            last_mouse_position: None,
        }
    }

    pub fn move_with_mouse(&mut self, position: Option<(f32, f32)>) {
        let Some(position) = position else {
            self.last_mouse_position = None;
            return;
        };
        if let Some(previous) = self.last_mouse_position {
            let horizontal = position.0 - previous.0;
            let vertical = position.1 - previous.1;
            let next = (
                (self.screen_offset.0 + horizontal).clamp(-410.0, 410.0),
                (self.screen_offset.1 + vertical).clamp(-250.0, 250.0),
            );
            let moved = (next.0 - self.screen_offset.0, next.1 - self.screen_offset.1);
            self.screen_offset = next;

            // Mouse motion moves the ship in its camera plane while its heading stays fixed.
            let (right, up, _) = self.camera.basis();
            self.position += right * (moved.0 / 12.0) - up * (moved.1 / 12.0);
        }
        self.last_mouse_position = Some(position);
    }

    pub fn reset_mouse_anchor(&mut self) {
        self.last_mouse_position = None;
    }

    pub fn advance(&mut self, dt: f32, thrust: (f32, f32, f32), boost: bool, brake: bool) {
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
