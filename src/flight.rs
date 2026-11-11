use crate::camera::Camera;
use crate::collision::CollisionWorld;
use crate::local_navigation::{LocalFlight, Reference};
use nalgebra_glm::Vec3;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

pub const CRASH_SPEED: f32 = 28.0;
pub const CRUISE_SPEED: f32 = 36.0;
pub const BOOST_SPEED: f32 = 155.0;
pub const SPAWN_POSITION: Vec3 = Vec3::new(88.0, -139.0, 579.0);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SteeringMode {
    #[default]
    Mouse,
    Arrows,
}

impl SteeringMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mouse => "MOUSE",
            Self::Arrows => "FLECHAS",
        }
    }
}

fn orient(camera: &mut Camera, forward: Vec3) {
    let backward = -forward.normalize();
    camera.yaw = backward.z.atan2(backward.x).rem_euclid(TAU);
    camera.pitch = backward.y.clamp(-0.999999, 0.999999).asin();
}

pub struct Crash {
    pub position: Vec3,
    pub normal: Vec3,
    pub age: f32,
    pub planet: &'static str,
}

/// Arcade flight with a mouse steering target and a camera following from behind.
pub struct Flight {
    pub position: Vec3,
    pub black_hole: crate::black_hole::BlackHole,
    spawn_position: Vec3,
    pub velocity: Vec3,
    pub camera: Camera,
    pub heading: Camera,
    pub bank: f32,
    pub seconds: f32,
    pub steering_mode: SteeringMode,
    gravity_velocity: Vec3,
    captured_for: Option<f32>,
    pub respawn_notice: f32,
    pub crash: Option<Crash>,
    pub contact_notice: f32,
    pub portals: crate::portal::Pair,
    collisions: Option<Arc<CollisionWorld>>,
    steering: (f32, f32),
    mouse_target: (f32, f32),
    navigation_targets: Arc<[Reference]>,
    local_navigation: Option<LocalFlight>,
    local_climb: f32,
}

impl Flight {
    pub fn new() -> Self {
        let heading = Camera {
            yaw: FRAC_PI_2,
            pitch: 0.0,
            zoom: 1.0,
            up_axis: None,
        };
        Self {
            position: Vec3::zeros(),
            black_hole: crate::black_hole::BLACK_HOLE,
            spawn_position: Vec3::zeros(),
            velocity: Vec3::zeros(),
            camera: Camera {
                pitch: 0.14,
                ..heading
            },
            heading,
            bank: 0.0,
            seconds: 0.0,
            steering_mode: SteeringMode::Mouse,
            gravity_velocity: Vec3::zeros(),
            captured_for: None,
            respawn_notice: 0.0,
            crash: None,
            contact_notice: 0.0,
            portals: crate::portal::Pair::default(),
            collisions: None,
            steering: (0.0, 0.0),
            mouse_target: (0.0, 0.0),
            navigation_targets: Arc::from([]),
            local_navigation: None,
            local_climb: 0.0,
        }
    }

    pub fn with_collisions(world: Arc<CollisionWorld>) -> Self {
        Self {
            position: SPAWN_POSITION,
            spawn_position: SPAWN_POSITION,
            collisions: Some(world),
            ..Self::new()
        }
    }

    fn respawn(&mut self) {
        let seconds = self.seconds;
        let spawn_position = self.spawn_position;
        let black_hole = self.black_hole;
        let collisions = self.collisions.take();
        let navigation_targets = Arc::clone(&self.navigation_targets);
        let steering_mode = self.steering_mode;
        let portals_enabled = self.portals.enabled();
        *self = Self::new();
        self.position = spawn_position;
        self.spawn_position = spawn_position;
        self.black_hole = black_hole;
        self.seconds = seconds;
        self.collisions = collisions;
        self.navigation_targets = navigation_targets;
        self.steering_mode = steering_mode;
        self.portals.set_enabled(portals_enabled);
        self.respawn_notice = 3.5;
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

    pub fn toggle_steering_mode(&mut self) {
        self.steering_mode = match self.steering_mode {
            SteeringMode::Mouse => SteeringMode::Arrows,
            SteeringMode::Arrows => SteeringMode::Mouse,
        };
        self.reset_steering();
    }

    pub fn steer_input(&mut self, mouse: Option<(f32, f32)>, arrows: (f32, f32)) {
        let target = match self.steering_mode {
            SteeringMode::Mouse => mouse,
            SteeringMode::Arrows => Some(arrows),
        };
        self.move_with_mouse(target);
    }

    pub fn reset_steering(&mut self) {
        self.mouse_target = (0.0, 0.0);
        self.steering = (0.0, 0.0);
        self.local_climb = 0.0;
    }

    pub fn set_navigation_targets(&mut self, targets: Vec<Reference>) {
        self.navigation_targets = targets.into();
        self.local_navigation = None;
    }

    pub fn is_local(&self) -> bool {
        self.local_navigation.is_some()
    }

    pub fn navigation_reference(&self) -> Option<Reference> {
        self.local_navigation
            .map(|local| local.reference)
            .or_else(|| {
                self.navigation_targets.iter().copied().min_by(|a, b| {
                    b.available(self.position, self.seconds)
                        .cmp(&a.available(self.position, self.seconds))
                        .then_with(|| {
                            a.altitude(self.position, self.seconds)
                                .total_cmp(&b.altitude(self.position, self.seconds))
                        })
                })
            })
    }

    pub fn relative_speed(&self) -> f32 {
        self.local_navigation
            .map_or_else(|| self.velocity.norm(), |local| local.velocity.norm())
    }

    pub fn set_local_climb(&mut self, climb: f32) {
        self.local_climb = climb.clamp(-1.0, 1.0);
    }

    pub fn toggle_local_navigation(&mut self) -> bool {
        if self.is_local() {
            self.leave_local_navigation();
            return true;
        }
        if self.crash.is_some() || self.is_captured() {
            return false;
        }
        let Some(reference) = self
            .navigation_reference()
            .filter(|reference| reference.available(self.position, self.seconds))
        else {
            return false;
        };
        let up = (self.position - reference.center_at(self.seconds)).normalize();
        let relative =
            self.velocity - reference.motion.velocity(self.seconds) - self.gravity_velocity;
        self.local_navigation = Some(LocalFlight {
            reference,
            velocity: relative - up * relative.dot(&up),
        });
        self.heading.up_axis = Some(up);
        let forward = reference.direction(
            self.position,
            self.heading.basis().2,
            (1.0, 0.0),
            0.0,
            self.seconds,
        );
        orient(&mut self.heading, forward);
        self.reset_steering();
        true
    }

    fn leave_local_navigation(&mut self) {
        self.local_navigation = None;
        self.heading.up_axis = None;
        self.local_climb = 0.0;
    }

    fn portal_space_clear(&self, center: Vec3) -> bool {
        self.portal_space_clear_for(center, crate::portal::RADIUS)
    }

    fn portal_space_clear_for(&self, center: Vec3, radius: f32) -> bool {
        let clearance = radius + crate::collision::SHIP_RADIUS + 0.65;
        (center - self.black_hole.position).norm() > self.black_hole.horizon + clearance
            && self
                .collisions
                .as_ref()
                .is_none_or(|world| world.sphere_clear(center, clearance, self.seconds))
    }

    pub fn can_open_galactic_gate(&self, center: Vec3, radius: f32) -> bool {
        self.crash.is_none()
            && !self.is_captured()
            && radius.is_finite()
            && radius > 0.0
            && self.portal_space_clear_for(center, radius)
            && self.portal_path_clear(self.position, center)
    }

    fn portal_path_clear(&self, start: Vec3, end: Vec3) -> bool {
        !self.black_hole.captures(start, end)
            && self.collisions.as_ref().is_none_or(|world| {
                world
                    .move_ship(start, end - start, self.seconds, 0.0)
                    .contacts
                    .is_empty()
            })
    }

    pub fn place_portal(&mut self, kind: crate::portal::Kind) -> bool {
        if !self.portals.enabled() || self.crash.is_some() || self.is_captured() {
            return false;
        }
        let forward = self.heading.basis().2;
        let center = self.position + forward * crate::portal::PLACE_DISTANCE;
        if !self.portal_space_clear(center) || !self.portal_path_clear(self.position, center) {
            self.portals
                .notify("No hay espacio libre. Apunta lejos de los planetas.");
            return false;
        }
        let Some(portal) = crate::portal::Portal::new(center, -forward) else {
            return false;
        };
        self.portals.place(kind, portal)
    }

    pub fn capture_progress(&self) -> f32 {
        self.captured_for
            .map_or(0.0, |time| (time / 1.4).clamp(0.0, 1.0))
    }

    pub fn is_captured(&self) -> bool {
        self.captured_for.is_some()
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
        self.respawn_notice = (self.respawn_notice - dt).max(0.0);
        self.contact_notice = (self.contact_notice - dt).max(0.0);
        self.portals.tick(dt);
        if let Some(crash) = self.crash.as_mut() {
            crash.age += dt;
            if crash.age >= 2.0 {
                self.respawn();
            }
            return;
        }
        if let Some(time) = self.captured_for.as_mut() {
            *time += dt;
            if *time >= 1.4 {
                self.respawn();
            }
            return;
        }
        let steps = (dt * 120.0).ceil() as usize;
        let dt = dt / steps as f32;
        for step in 0..steps {
            let time = self.seconds - (steps - step) as f32 * dt;
            if self
                .local_navigation
                .is_some_and(|local| local.reference.outside_local_zone(self.position, time))
            {
                self.leave_local_navigation();
            }
            let reference = self.local_navigation.map(|local| local.reference);
            let turn = reference.map_or(1.0, |body| body.turn_scale(self.position, time));
            let response = 1.0 - (-8.0 * dt).exp();
            self.steering.0 += (self.mouse_target.0 - self.steering.0) * response;
            self.steering.1 += (self.mouse_target.1 - self.steering.1) * response;
            if let Some(body) = reference {
                let up = (self.position - body.center_at(time)).normalize();
                self.heading.up_axis = Some(up);
                let forward = crate::local_navigation::rotate(
                    self.heading.basis().2,
                    up,
                    -self.steering.0 * 1.35 * dt * turn,
                );
                let right = forward.cross(&up).normalize();
                let forward = crate::local_navigation::rotate(
                    forward,
                    right,
                    -self.steering.1 * 1.05 * dt * turn,
                );
                let vertical = forward.dot(&up).clamp(-0.97, 0.97);
                let tangent = (forward - up * forward.dot(&up)).normalize();
                orient(
                    &mut self.heading,
                    tangent * (1.0 - vertical * vertical).sqrt() + up * vertical,
                );
            } else {
                self.heading.up_axis = None;
                self.heading.yaw = (self.heading.yaw + self.steering.0 * 1.35 * dt).rem_euclid(TAU);
                self.heading.pitch =
                    (self.heading.pitch + self.steering.1 * 1.05 * dt + PI).rem_euclid(TAU) - PI;
            }
            self.bank += (-self.steering.0 * 0.55 - self.bank) * response;

            let (right, _, forward) = self.heading.basis();
            let direction = reference.map_or_else(
                || forward * movement.0 + right * movement.1,
                |body| body.direction(self.position, forward, movement, self.local_climb, time),
            );
            // Normalize diagonals so W+D is as fast as W alone.
            let direction = direction / direction.norm().max(1.0);
            let speed = reference.map_or_else(
                || if boost { BOOST_SPEED } else { CRUISE_SPEED },
                |body| body.speed(self.position, time, boost),
            );
            let mut target_velocity = if brake {
                Vec3::zeros()
            } else {
                direction * speed
            };
            if let Some(body) = reference {
                // Faster tangential cruise must not turn Q/E into dangerous terrain impacts.
                let up = (self.position - body.center_at(time)).normalize();
                let climb = target_velocity.dot(&up);
                let limit = if boost { 18.0 } else { 12.0 };
                target_velocity += up * (climb.clamp(-limit, limit) - climb);
            }
            let acceleration = if brake || direction.norm_squared() == 0.0 {
                6.0
            } else {
                3.0
            };
            // Thrusters damp their own motion; releasing WASD does not erase gravity.
            let mut controlled = self
                .local_navigation
                .map_or(self.velocity - self.gravity_velocity, |local| {
                    local.velocity
                });
            controlled += (target_velocity - controlled) * (1.0 - (-acceleration * dt).exp());
            self.gravity_velocity += self.black_hole.acceleration(self.position) * dt;
            // Brakes can help in the outer field but cannot cancel the attraction.
            if brake {
                self.gravity_velocity *= (-0.25 * dt).exp();
            }
            let drift_speed = self.gravity_velocity.norm();
            if drift_speed > 320.0 {
                self.gravity_velocity *= 320.0 / drift_speed;
            }
            self.velocity = controlled + self.gravity_velocity;
            let local_step =
                reference.map(|body| body.step(self.position, self.velocity * dt, time, dt));
            let mut next = local_step
                .as_ref()
                .map_or(self.position + self.velocity * dt, |step| step.position);
            if local_step.is_some() {
                self.velocity = (next - self.position) / dt;
            }
            if let Some(world) = &self.collisions {
                let motion = world.move_ship(self.position, self.velocity * dt, time, dt);
                next = motion.position;
                for contact in &motion.contacts {
                    let impact = -(self.velocity - contact.surface_velocity).dot(&contact.normal);
                    if impact >= CRASH_SPEED {
                        self.crash = Some(Crash {
                            position: next,
                            normal: contact.normal,
                            age: 0.0,
                            planet: contact.planet,
                        });
                        self.velocity = Vec3::zeros();
                        self.gravity_velocity = Vec3::zeros();
                        self.position = next;
                        self.local_navigation = None;
                        break;
                    }
                    let inward = (self.velocity - contact.surface_velocity)
                        .dot(&contact.normal)
                        .min(0.0);
                    self.velocity -= contact.normal * inward;
                    controlled -= contact.normal * inward;
                    let gravity_inward = self.gravity_velocity.dot(&contact.normal).min(0.0);
                    self.gravity_velocity -= contact.normal * gravity_inward;
                    self.contact_notice = 0.7;
                }
                if self.crash.is_some() {
                    break;
                }
            }
            let mut segment_start = self.position;
            if let Some(transit) = self.portals.crossing(self.position, next) {
                if self.portal_space_clear(transit.exit_center())
                    && self.portal_path_clear(transit.exit_start, transit.position)
                    && !self.black_hole.captures(self.position, transit.entry_point)
                {
                    let rotated_right = transit.rotate(self.ship_basis().0);
                    self.velocity = transit.rotate(self.velocity);
                    self.gravity_velocity = transit.rotate(self.gravity_velocity);
                    let rotate_camera = |camera: &mut Camera| {
                        let backward = -transit.rotate(camera.basis().2);
                        camera.yaw = backward.z.atan2(backward.x).rem_euclid(TAU);
                        camera.pitch = backward.y.clamp(-1.0, 1.0).asin();
                        camera.up_axis = camera.up_axis.map(|up| transit.rotate(up));
                    };
                    rotate_camera(&mut self.heading);
                    rotate_camera(&mut self.camera);
                    let (right, up, _) = self.heading.basis();
                    self.bank = rotated_right.dot(&up).atan2(rotated_right.dot(&right));
                    self.reset_steering();
                    next = transit.position;
                    segment_start = transit.exit_start;
                    self.portals.complete_transit();
                    self.local_navigation = None;
                } else {
                    self.portals
                        .notify("Salida bloqueada. Recoloca ese portal.");
                }
            }
            if self.black_hole.captures(segment_start, next) {
                self.captured_for = Some(0.0);
                self.velocity = Vec3::zeros();
                self.gravity_velocity = Vec3::zeros();
                self.local_navigation = None;
                break;
            }
            self.position = next;

            if let (Some(local), Some(step)) = (self.local_navigation.as_mut(), local_step.as_ref())
            {
                local.velocity = step.transport(controlled);
                let forward = step.transport(self.heading.basis().2);
                let view = step.transport(self.camera.basis().2);
                orient(&mut self.heading, forward);
                orient(&mut self.camera, view);
                self.heading.up_axis =
                    Some((self.position - local.reference.center_at(time + dt)).normalize());
                self.camera.up_axis = self.camera.up_axis.map(|up| step.transport(up));
            }

            let follow = 1.0 - (-7.0 * dt).exp();
            if let Some(local) = self.local_navigation {
                let up = (self.position - local.reference.center_at(time + dt)).normalize();
                let target = self.heading.basis().2 * 0.40_f32.cos() - up * 0.40_f32.sin();
                let forward = self.camera.basis().2;
                orient(&mut self.camera, forward + (target - forward) * follow);
                self.camera.up_axis = Some(crate::local_navigation::blend_up(
                    self.camera.up_axis.unwrap_or_else(Vec3::y),
                    up,
                    self.camera.basis().2,
                    follow,
                ));
            } else {
                let yaw_error = (self.heading.yaw - self.camera.yaw + PI).rem_euclid(TAU) - PI;
                self.camera.yaw = (self.camera.yaw + yaw_error * follow).rem_euclid(TAU);
                let pitch_error =
                    (self.heading.pitch + 0.14 - self.camera.pitch + PI).rem_euclid(TAU) - PI;
                self.camera.pitch =
                    (self.camera.pitch + pitch_error * follow + PI).rem_euclid(TAU) - PI;
                if let Some(up) = self.camera.up_axis {
                    let up = crate::local_navigation::blend_up(
                        up,
                        Vec3::y(),
                        self.camera.basis().2,
                        follow,
                    );
                    self.camera.up_axis = ((up - Vec3::y()).norm() > 0.001).then_some(up);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_flight() -> Flight {
        let mut flight = Flight::new();
        flight.set_navigation_targets(vec![crate::local_navigation::Reference {
            name: "Prueba",
            center: Vec3::zeros(),
            radius: 90.0,
            surface_radius: 90.0,
            motion: crate::orbit::Motion::default(),
        }]);
        flight.position = Vec3::z() * 114.0;
        flight.heading.yaw = PI;
        assert!(flight.toggle_local_navigation());
        flight
    }

    #[test]
    fn local_entry_prefers_a_planet_in_range_over_a_closer_unavailable_reference() {
        use crate::local_navigation::Reference;
        let mut flight = Flight::new();
        let small = Reference {
            name: "Fuera de alcance",
            center: Vec3::zeros(),
            radius: 10.0,
            surface_radius: 10.0,
            motion: crate::orbit::Motion::default(),
        };
        let large = Reference {
            name: "Disponible",
            center: -Vec3::z() * 215.0,
            radius: 200.0,
            surface_radius: 200.0,
            motion: crate::orbit::Motion::default(),
        };
        flight.position = Vec3::z() * 66.7;
        assert!(!small.available(flight.position, 0.0));
        assert!(large.available(flight.position, 0.0));
        flight.set_navigation_targets(vec![small, large]);
        assert!(flight.toggle_local_navigation());
        assert_eq!(flight.navigation_reference().unwrap().name, "Disponible");
    }

    #[test]
    fn arrows_keep_pitching_in_both_directions_after_surface_contact() {
        let mut space = crate::space::Space::load().unwrap();
        space.configure(1);
        for sign in [-1.0, 1.0] {
            for fps in [10, 60] {
                let mut flight = space.new_flight();
                space.approach(&mut flight, "Kepler-186f").unwrap();
                assert!(flight.toggle_local_navigation());
                flight.set_local_climb(-1.0);
                for _ in 0..fps * 10 {
                    flight.advance(1.0 / fps as f32, (0.0, 0.0), false, false);
                }
                assert!(flight.contact_notice > 0.0);
                assert!(flight.toggle_local_navigation());
                flight.toggle_steering_mode();
                flight.heading.pitch = sign * 1.1;
                let mut swept = 0.0;
                let mut previous = flight.heading.basis().2;
                for _ in 0..fps * 8 {
                    let right = flight.heading.basis().0;
                    flight.steer_input(None, (0.0, sign));
                    flight.advance(1.0 / fps as f32, (0.0, 0.0), false, false);
                    let forward = flight.heading.basis().2;
                    let angle = previous
                        .cross(&forward)
                        .dot(&right)
                        .atan2(previous.dot(&forward));
                    assert!(
                        angle * -sign > 0.0,
                        "La flecha no debe bloquearse ni invertir el giro"
                    );
                    swept += angle.abs();
                    assert!(flight.camera.basis().2.iter().all(|v| v.is_finite()));
                    assert!(flight.crash.is_none());
                    previous = forward;
                }
                assert!(swept > TAU, "Debe permitir un giro completo: {swept}");
            }
        }
    }

    #[test]
    fn arrow_steering_ignores_mouse_and_switching_back_clears_the_old_turn() {
        let mut flight = Flight::new();
        flight.toggle_steering_mode();
        assert_eq!(flight.steering_mode, SteeringMode::Arrows);
        let start = flight.heading.yaw;
        for _ in 0..60 {
            flight.steer_input(Some((1.0, 1.0)), (0.0, 0.0));
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        }
        assert_eq!(start, flight.heading.yaw);
        for _ in 0..60 {
            flight.steer_input(Some((-1.0, -1.0)), (1.0, 0.6));
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        }
        assert!(flight.heading.yaw > start + 0.5);
        assert!(flight.heading.pitch > 0.2);
        flight.respawn();
        assert_eq!(flight.steering_mode, SteeringMode::Arrows);
        flight.toggle_steering_mode();
        let heading = flight.heading.yaw;
        for _ in 0..30 {
            flight.steer_input(Some((0.0, 0.0)), (-1.0, -1.0));
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        }
        assert_eq!(heading, flight.heading.yaw);
    }

    #[test]
    fn local_navigation_preserves_height_is_frame_rate_independent_and_can_be_disabled() {
        let simulate = |fps| {
            let mut flight = local_flight();
            for _ in 0..fps * 20 {
                flight.advance(1.0 / fps as f32, (1.0, 0.0), false, false);
            }
            assert!((flight.position.norm() - 114.0).abs() < 0.03);
            assert!(flight.is_local());
            flight
        };
        let mut fast = simulate(60);
        let slow = simulate(10);
        assert!((fast.position - slow.position).norm() < 0.3);
        let position = fast.position;
        let velocity = fast.velocity;
        assert!(fast.toggle_local_navigation());
        assert!(!fast.is_local());
        assert_eq!(position, fast.position);
        assert_eq!(velocity, fast.velocity);
        fast.advance(0.1, (1.0, 0.0), true, false);
        assert!(fast.velocity.norm() > velocity.norm());
    }

    #[test]
    fn local_navigation_climbs_brakes_and_rejects_distant_or_destroyed_ships() {
        let mut flight = local_flight();
        flight.set_local_climb(1.0);
        for _ in 0..60 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        }
        assert!(flight.position.norm() > 117.0);
        flight.set_local_climb(0.0);
        for _ in 0..120 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, true);
        }
        assert!(flight.velocity.norm() < 0.01);
        flight.respawn();
        assert!(!flight.is_local());
        flight.position = Vec3::repeat(10000.0);
        assert!(!flight.toggle_local_navigation());
        flight.position = Vec3::z() * 114.0;
        flight.crash = Some(Crash {
            position: flight.position,
            normal: Vec3::z(),
            age: 0.0,
            planet: "Prueba",
        });
        assert!(!flight.toggle_local_navigation());
    }

    fn portal_flight() -> Flight {
        use crate::portal::{Kind, Portal};
        let mut flight = Flight::new();
        flight.portals.set_enabled(true);
        flight.portals.place(
            Kind::Yellow,
            Portal::new(Vec3::new(0.0, 0.0, -12.0), Vec3::z()).unwrap(),
        );
        flight.portals.place(
            Kind::Green,
            Portal::new(Vec3::new(100.0, 0.0, 0.0), Vec3::x()).unwrap(),
        );
        flight
    }

    #[test]
    fn boost_crosses_portals_at_low_fps_without_losing_speed_or_camera_alignment() {
        let simulate = |fps| {
            let mut flight = portal_flight();
            for _ in 0..fps * 2 {
                flight.advance(1.0 / fps as f32, (1.0, 0.0), true, false);
            }
            flight
        };
        let fast = simulate(60);
        let slow = simulate(10);
        assert_eq!(fast.portals.traversals, 1);
        assert_eq!(slow.portals.traversals, 1);
        assert!(fast.position.x > 250.0 && fast.position.z.abs() < 0.1);
        assert!((fast.position - slow.position).norm() < 0.1);
        assert!(fast.velocity.norm() > 150.0);
        assert!(fast.heading.basis().2.dot(&Vec3::x()) > 0.99);
        assert!(fast.camera.basis().2.dot(&Vec3::x()) > 0.98);
    }

    #[test]
    fn teleport_does_not_sweep_the_empty_space_between_portals_through_the_black_hole() {
        let mut flight = portal_flight();
        flight.black_hole.position = Vec3::new(50.0, 0.0, -6.0);
        flight.black_hole.horizon = 8.0;
        flight.black_hole.influence = 12.0;
        for _ in 0..120 {
            flight.advance(1.0 / 60.0, (1.0, 0.0), false, false);
        }
        assert_eq!(flight.portals.traversals, 1);
        assert!(!flight.is_captured());
    }

    #[test]
    fn placement_requires_capability_and_rejects_the_black_hole_without_erasing_old_portal() {
        use crate::portal::Kind;
        let mut flight = Flight::new();
        assert!(!flight.place_portal(Kind::Yellow));
        flight.portals.set_enabled(true);
        assert!(flight.place_portal(Kind::Yellow));
        let old = flight.portals.get(Kind::Yellow).unwrap().center;
        flight.black_hole.position = old;
        assert!(!flight.place_portal(Kind::Yellow));
        assert_eq!(flight.portals.get(Kind::Yellow).unwrap().center, old);
        assert!(!flight.place_portal(Kind::Green));
    }

    #[test]
    fn a_planet_at_the_exit_prevents_teleport_and_respawning_clears_the_pair() {
        let model = crate::model::Model::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/uv-checker.obj"
        ))
        .unwrap();
        let collider = crate::collision::PlanetCollider::new(
            "Blocked exit",
            &model,
            crate::renderer::WorldTransform {
                position: Vec3::new(100.0, 0.0, 0.0),
                radius: 10.0,
                yaw: 0.0,
            },
            0.0,
        );
        let mut flight = portal_flight();
        flight.collisions = Some(Arc::new(CollisionWorld::new(vec![collider])));
        for _ in 0..120 {
            flight.advance(1.0 / 60.0, (1.0, 0.0), false, false);
        }
        assert_eq!(flight.portals.traversals, 0);
        assert!(flight.position.x.abs() < 0.1);
        flight.respawn();
        assert!(flight.portals.enabled());
        assert!(!flight.portals.linked());
    }

    #[test]
    fn gravity_pulls_an_idle_ship_and_remains_consistent_between_frame_rates() {
        let simulate = |fps: usize| {
            let mut flight = Flight::new();
            flight.position = crate::black_hole::BLACK_HOLE.position + Vec3::z() * 200.0;
            for _ in 0..fps * 2 {
                flight.advance(1.0 / fps as f32, (0.0, 0.0), false, false);
            }
            flight
        };
        let flight = simulate(60);
        assert!(flight.velocity.z < -5.0);
        assert!((flight.position - crate::black_hole::BLACK_HOLE.position).norm() < 195.0);
        assert!((flight.position - simulate(30).position).norm() < 0.05);
    }

    #[test]
    fn boost_can_escape_the_outer_field() {
        let mut flight = Flight::new();
        flight.position = crate::black_hole::BLACK_HOLE.position + Vec3::z() * 250.0;
        flight.heading.yaw = 3.0 * FRAC_PI_2;
        flight.camera = flight.heading;
        for _ in 0..360 {
            flight.advance(1.0 / 60.0, (1.0, 0.0), true, false);
        }
        assert!(!flight.is_captured());
        assert!((flight.position - crate::black_hole::BLACK_HOLE.position).norm() > 500.0);
    }

    #[test]
    fn capture_respawns_without_invalid_coordinates_or_residual_gravity() {
        let mut flight = Flight::new();
        flight.position = crate::black_hole::BLACK_HOLE.position + Vec3::z() * 75.0;
        let mut captured = false;
        for _ in 0..600 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
            captured |= flight.is_captured();
            assert!(flight.position.iter().all(|v| v.is_finite()));
            if flight.respawn_notice > 0.0 {
                break;
            }
        }
        assert!(captured && flight.respawn_notice > 0.0);
        assert_eq!(flight.position, Vec3::zeros());
        assert_eq!(flight.velocity, Vec3::zeros());
        assert_eq!(flight.gravity_velocity, Vec3::zeros());
        assert!(!flight.is_captured());
    }

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
            assert!(flight.heading.pitch.abs() <= PI);
        }
    }

    #[test]
    fn boost_brake_and_frame_rate_preserve_control() {
        let simulate = |fps: usize, boost: bool| {
            let mut flight = Flight::new();
            flight.move_with_mouse(Some((-0.5, 0.25)));
            for _ in 0..fps * 2 {
                flight.advance(1.0 / fps as f32, (1.0, 0.0), boost, false);
            }
            flight
        };
        let mut flight = simulate(60, true);
        let slower_frames = simulate(30, true);
        assert!((flight.position - slower_frames.position).norm() < 0.05);
        assert!(
            flight.velocity.norm() > BOOST_SPEED * 0.88 && flight.velocity.norm() <= BOOST_SPEED
        );
        assert!(flight.velocity.norm() > simulate(60, false).velocity.norm() * 2.5);
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
