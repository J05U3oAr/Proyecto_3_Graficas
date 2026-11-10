use nalgebra_glm::Vec3;
use std::f32::consts::PI;

#[derive(Clone, Copy)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub up_axis: Option<Vec3>,
}

impl Camera {
    pub const fn new() -> Self {
        Self {
            yaw: -0.42,
            pitch: 0.92,
            zoom: 1.0,
            up_axis: None,
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
        // The yaw defines the right axis even when pitching through the poles.
        // Crossing forward with world-up would flip this axis after half a loop.
        if self.up_axis.is_none() {
            let right = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos());
            return (right, right.cross(&forward).normalize(), forward);
        }
        let up_axis = self.up_axis.unwrap_or_else(Vec3::y);
        let mut right = forward.cross(&up_axis);
        if right.norm_squared() < 1e-8 {
            let fallback = if forward.y.abs() < 0.9 {
                Vec3::y()
            } else {
                Vec3::x()
            };
            right = forward.cross(&fallback);
        }
        let right = right.normalize();
        let up = right.cross(&forward).normalize();
        (right, up, forward)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spatial_basis_remains_continuous_through_both_vertical_directions() {
        for pitch in [-PI / 2.0, PI / 2.0] {
            for yaw in [0.0, 0.7, PI] {
                let mut previous_up = None::<Vec3>;
                for offset in [-0.001, 0.0, 0.001] {
                    let camera = Camera {
                        yaw,
                        pitch: pitch + offset,
                        ..Camera::new()
                    };
                    let (right, up, forward) = camera.basis();
                    assert!((right.norm() - 1.0).abs() < 1e-6);
                    assert!((up.norm() - 1.0).abs() < 1e-6);
                    assert!(right.dot(&forward).abs() < 1e-6);
                    assert!(up.dot(&forward).abs() < 1e-6);
                    if let Some(previous) = previous_up {
                        assert!(previous.dot(&up) > 0.999);
                    }
                    previous_up = Some(up);
                }
            }
        }
    }
}
