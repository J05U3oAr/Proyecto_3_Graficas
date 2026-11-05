use crate::camera::Camera;
use nalgebra_glm::Vec3;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Arcade flight with a mouse steering target and a camera following from behind.
pub struct Flight {
    pub position: Vec3,
    pub velocity: Vec3,
    pub camera: Camera,
    pub heading: Camera,
    pub bank: f32,
    pub seconds: f32,
    steering: (f32, f32),
    mouse_target: (f32, f32),
}

impl Flight {
    pub fn new() -> Self {
        let heading = Camera {
            yaw: FRAC_PI_2,
            pitch: 0.0,
            zoom: 1.0,
        };
        Self {
            position: Vec3::zeros(),
            velocity: Vec3::zeros(),
            camera: Camera {
                pitch: 0.14,
                ..heading
            },
            heading,
            bank: 0.0,
            seconds: 0.0,
            steering: (0.0, 0.0),
            mouse_target: (0.0, 0.0),
        }
    }

    /// Normalized canvas coordinates: center = (0, 0), right/bottom = (1, 1).
    /// Position is a sustained steering command, so turns continue at the window edge.
    pub fn move_with_mouse(&mut self, position: Option<(f32, f32)>) {
        let dead_zone = |value: f32| {
            let value = value.clamp(-1.0, 1.0);
            value.signum() * ((value.abs() - 0.08) / 0.92).max(0.0)
        };
        self.mouse_target = position
            .map(|(x, y)| (dead_zone(x), dead_zone(y)))
            .unwrap_or((0.0, 0.0));
    }

    pub fn reset_steering(&mut self) {
        self.mouse_target = (0.0, 0.0);
        self.steering = (0.0, 0.0);
    }

    pub fn camera_position(&self) -> Vec3 {
        let (_, up, forward) = self.camera.basis();
        self.position - forward * 8.0 + up * 1.6
    }

    pub fn ship_basis(&self) -> (Vec3, Vec3, Vec3) {
        let (right, up, forward) = self.heading.basis();
        let (sin, cos) = self.bank.sin_cos();
        (right * cos + up * sin, up * cos - right * sin, forward)
    }

    /// Movement axes are forward/backward and right/left, relative to the nose.
    pub fn advance(&mut self, dt: f32, movement: (f32, f32), boost: bool, brake: bool) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        // Small physics steps keep steering and camera response consistent at low FPS.
        let dt = dt.min(0.1);
        self.seconds += dt;
        let steps = (dt * 120.0).ceil() as usize;
        let dt = dt / steps as f32;
        for _ in 0..steps {
            let response = 1.0 - (-8.0 * dt).exp();
            self.steering.0 += (self.mouse_target.0 - self.steering.0) * response;
            self.steering.1 += (self.mouse_target.1 - self.steering.1) * response;
            self.heading.yaw = (self.heading.yaw + self.steering.0 * 1.35 * dt).rem_euclid(TAU);
            self.heading.pitch =
                (self.heading.pitch + self.steering.1 * 1.05 * dt).clamp(-1.15, 1.15);
            self.bank += (-self.steering.0 * 0.55 - self.bank) * response;

            let (right, _, forward) = self.heading.basis();
            let direction = forward * movement.0 + right * movement.1;
            // Normalize diagonals so W+D is as fast as W alone.
            let direction = direction / direction.norm().max(1.0);
            let speed = if boost { 62.0 } else { 36.0 };
            let target_velocity = if brake {
                Vec3::zeros()
            } else {
                direction * speed
            };
            let acceleration = if brake || direction.norm_squared() == 0.0 {
                6.0
            } else {
                3.0
            };
            self.velocity += (target_velocity - self.velocity) * (1.0 - (-acceleration * dt).exp());
            self.position += self.velocity * dt;

            let follow = 1.0 - (-7.0 * dt).exp();
            let yaw_error = (self.heading.yaw - self.camera.yaw + PI).rem_euclid(TAU) - PI;
            self.camera.yaw = (self.camera.yaw + yaw_error * follow).rem_euclid(TAU);
            self.camera.pitch += (self.heading.pitch + 0.14 - self.camera.pitch) * follow;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ship_waits_for_movement_keys_even_when_the_mouse_turns() {
        let mut flight = Flight::new();
        flight.move_with_mouse(Some((0.6, -0.3)));
        for _ in 0..120 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), true, false);
        }
        assert_eq!(flight.position, Vec3::zeros());
        assert_eq!(flight.velocity, Vec3::zeros());
    }

    #[test]
    fn holding_mouse_off_center_steers_the_flight_direction() {
        let mut flight = Flight::new();
        let initial_forward = flight.camera.basis().2;
        for _ in 0..60 {
            flight.move_with_mouse(Some((0.65, -0.35)));
            flight.advance(1.0 / 60.0, (1.0, 0.0), false, false);
        }
        assert!(
            flight.camera.basis().2.dot(&initial_forward) < 0.98,
            "Holding the mouse away from center must steer, not just slide the model"
        );
        assert!(flight.position.x > 0.0, "Mouse right must fly right");
        assert!(flight.position.y > 0.0, "Mouse up must fly up");
        assert!(flight.bank < 0.0, "Right turns bank the right wing down");
    }

    #[test]
    fn mouse_input_does_not_teleport_the_ship_and_center_stops_turning() {
        let mut flight = Flight::new();
        flight.move_with_mouse(Some((1.0, -1.0)));
        assert_eq!(flight.position, Vec3::zeros());
        for _ in 0..60 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        }
        flight.move_with_mouse(Some((0.0, 0.0)));
        for _ in 0..120 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        }
        let heading = flight.heading.basis().2;
        for _ in 0..60 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        }
        assert!(heading.dot(&flight.heading.basis().2) > 0.99999);
        assert!(flight.bank.abs() < 0.001);
        assert_eq!(
            flight.position,
            Vec3::zeros(),
            "Mouse steering alone must not translate the ship"
        );
    }

    #[test]
    fn camera_stays_behind_the_ship_even_through_full_turns() {
        let mut flight = Flight::new();
        flight.move_with_mouse(Some((1.0, -1.0)));
        for _ in 0..1800 {
            flight.advance(1.0 / 60.0, (1.0, 0.0), false, false);
            let camera_to_ship = flight.position - flight.camera_position();
            assert!(camera_to_ship.dot(&flight.heading.basis().2) > 6.0);
            assert!((camera_to_ship.norm() - (64.0f32 + 2.56).sqrt()).abs() < 0.001);
            assert!(flight.heading.pitch.abs() <= 1.15);
        }
    }

    #[test]
    fn boost_brake_and_frame_rate_preserve_control() {
        let simulate = |fps: usize| {
            let mut flight = Flight::new();
            flight.move_with_mouse(Some((-0.5, 0.25)));
            for _ in 0..fps * 2 {
                flight.advance(1.0 / fps as f32, (1.0, 0.0), true, false);
            }
            flight
        };
        let mut flight = simulate(60);
        let slower_frames = simulate(30);
        assert!((flight.position - slower_frames.position).norm() < 0.05);
        assert!(flight.velocity.norm() > 55.0 && flight.velocity.norm() <= 62.0);
        flight.move_with_mouse(None);
        for _ in 0..120 {
            flight.advance(1.0 / 60.0, (1.0, 0.0), true, true);
        }
        assert!(
            flight.velocity.norm() < 0.01,
            "Brake takes priority over boost"
        );
    }

    #[test]
    fn wasd_moves_in_each_local_direction_without_a_diagonal_speed_bonus() {
        let initial = Flight::new();
        let (right, _, forward) = initial.heading.basis();
        let simulate = |movement| {
            let mut flight = Flight::new();
            for _ in 0..120 {
                flight.advance(1.0 / 60.0, movement, false, false);
            }
            flight
        };
        for (movement, expected) in [
            ((1.0, 0.0), forward),
            ((-1.0, 0.0), -forward),
            ((0.0, 1.0), right),
            ((0.0, -1.0), -right),
        ] {
            let flight = simulate(movement);
            assert!(flight.position.normalize().dot(&expected) > 0.9999);
            assert!(flight.velocity.norm() > 35.0);
        }
        let straight = simulate((1.0, 0.0));
        let diagonal = simulate((1.0, 1.0));
        assert!((straight.velocity.norm() - diagonal.velocity.norm()).abs() < 0.001);
        assert!((straight.position.norm() - diagonal.position.norm()).abs() < 0.001);
    }

    #[test]
    fn releasing_or_cancelling_movement_keys_stops_the_ship_smoothly() {
        let mut flight = Flight::new();
        flight.move_with_mouse(Some((0.6, -0.3)));
        for _ in 0..120 {
            flight.advance(1.0 / 60.0, (1.0, 1.0), false, false);
        }
        let forward = flight.heading.basis().2;
        assert!(flight.velocity.dot(&forward) > 15.0);
        flight.move_with_mouse(None);
        let moving_speed = flight.velocity.norm();
        flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        assert!(flight.velocity.norm() > 0.0 && flight.velocity.norm() < moving_speed);
        for _ in 0..120 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        }
        assert!(flight.velocity.norm() < 0.01);
    }
}
