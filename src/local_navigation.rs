use nalgebra_glm::Vec3;

#[derive(Clone, Copy)]
pub struct Reference {
    pub name: &'static str,
    pub center: Vec3,
    pub radius: f32,
    pub motion: crate::orbit::Motion,
}

#[derive(Clone, Copy)]
pub struct LocalFlight {
    pub reference: Reference,
    pub velocity: Vec3,
}

pub struct Step {
    pub position: Vec3,
    axis: Vec3,
    angle: f32,
}

impl Reference {
    pub fn center_at(self, seconds: f32) -> Vec3 {
        self.center + self.motion.displacement(seconds)
    }

    pub fn altitude(self, position: Vec3, seconds: f32) -> f32 {
        (position - self.center_at(seconds)).norm() - self.radius - crate::collision::SHIP_RADIUS
    }

    pub fn available(self, position: Vec3, seconds: f32) -> bool {
        let altitude = self.altitude(position, seconds);
        altitude >= 0.0 && altitude <= self.radius.max(30.0) * 1.5
    }

    pub fn speed(self, position: Vec3, seconds: f32, boost: bool) -> f32 {
        let speed = (6.0 + self.altitude(position, seconds).max(0.0) * 0.22).clamp(6.0, 26.0);
        speed * if boost { 1.6 } else { 1.0 }
    }

    pub fn turn_scale(self, position: Vec3, seconds: f32) -> f32 {
        (0.45 + self.altitude(position, seconds).max(0.0) / (self.radius * 2.0)).clamp(0.45, 1.0)
    }

    /// Proyectar el rumbo en el horizonte local; también funciona en ambos polos.
    pub fn direction(
        self,
        position: Vec3,
        forward: Vec3,
        movement: (f32, f32),
        climb: f32,
        seconds: f32,
    ) -> Vec3 {
        let up = (position - self.center_at(seconds)).normalize();
        let mut tangent = forward - up * forward.dot(&up);
        if tangent.norm_squared() < 0.0001 {
            let axis = if up.y.abs() < 0.9 {
                Vec3::y()
            } else {
                Vec3::x()
            };
            tangent = axis.cross(&up);
        }
        let forward = tangent.normalize();
        let right = forward.cross(&up).normalize();
        let direction = forward * movement.0 + right * movement.1 + up * climb;
        direction / direction.norm().max(1.0)
    }

    /// Arco sobre la esfera, no una cuerda que se hunda en el terreno.
    /// La traslación orbital se añade exactamente sin depender de la cantidad de cuadros.
    pub fn step(self, position: Vec3, delta: Vec3, seconds: f32, dt: f32) -> Step {
        let radial = position - self.center_at(seconds);
        let radius = radial.norm().max(0.001);
        let up = radial / radius;
        let climb = delta.dot(&up);
        let tangent = delta - up * climb;
        let distance = tangent.norm();
        let angle = distance / radius;
        let (axis, direction) = if distance > 0.000001 {
            let direction = tangent / distance;
            (up.cross(&direction).normalize(), direction)
        } else {
            (Vec3::zeros(), Vec3::zeros())
        };
        Step {
            position: self.center_at(seconds + dt)
                + (up * angle.cos() + direction * angle.sin()) * (radius + climb).max(0.001),
            axis,
            angle,
        }
    }
}

impl Step {
    /// Transporte paralelo de rumbo y velocidad mientras cambia el horizonte.
    pub fn transport(&self, direction: Vec3) -> Vec3 {
        rotate(direction, self.axis, self.angle)
    }
}

pub fn rotate(direction: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let (sin, cos) = angle.sin_cos();
    direction * cos + axis.cross(&direction) * sin + axis * axis.dot(&direction) * (1.0 - cos)
}

pub fn blend_up(from: Vec3, target: Vec3, forward: Vec3, response: f32) -> Vec3 {
    let dot = from.dot(&target).clamp(-1.0, 1.0);
    let cross = from.cross(&target);
    if dot > 0.9999 {
        return target;
    }
    let axis = if cross.norm_squared() > 1e-8 {
        cross.normalize()
    } else {
        forward.normalize()
    };
    rotate(from, axis, dot.acos() * response).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planet() -> Reference {
        Reference {
            name: "Prueba",
            center: Vec3::zeros(),
            radius: 90.0,
            motion: crate::orbit::Motion::default(),
        }
    }

    #[test]
    fn tangential_travel_circles_a_planet_without_losing_height_or_heading() {
        let reference = planet();
        let mut position = Vec3::z() * 110.0;
        let mut forward = Vec3::x();
        let dt = 1.0 / 60.0;
        let duration = std::f32::consts::TAU * 110.0 / 12.0;
        for _ in 0..(duration / dt).round() as usize {
            let step = reference.step(position, forward * 12.0 * dt, 0.0, dt);
            forward = step.transport(forward);
            position = step.position;
            assert!((position.norm() - 110.0).abs() < 0.02);
            assert!(forward.dot(&position.normalize()).abs() < 0.001);
        }
        assert!((position - Vec3::z() * 110.0).norm() < 0.3);
    }

    #[test]
    fn local_frame_follows_orbits_and_climb_is_radial() {
        let reference = Reference {
            motion: crate::orbit::aurora_motion(6),
            ..planet()
        };
        let start = reference.center_at(0.0) + Vec3::z() * 110.0;
        let step = reference.step(start, Vec3::zeros(), 0.0, 1.0);
        assert!((step.position - reference.center_at(1.0) - Vec3::z() * 110.0).norm() < 0.001);
        let up = reference.step(start, Vec3::z() * 3.0, 0.0, 1.0);
        assert!(((up.position - reference.center_at(1.0)).norm() - 113.0).abs() < 0.001);
    }

    #[test]
    fn controls_work_at_poles_and_speed_increases_with_altitude() {
        let reference = planet();
        for normal in [Vec3::x(), Vec3::y(), -Vec3::y(), Vec3::z()] {
            let direction = reference.direction(normal * 110.0, normal, (1.0, 1.0), 0.0, 0.0);
            assert!(direction.iter().all(|v| v.is_finite()));
            assert!(direction.dot(&normal).abs() < 0.001);
            assert!((direction.norm() - 1.0).abs() < 0.001);
        }
        assert!(
            reference.speed(Vec3::z() * 94.0, 0.0, false)
                < reference.speed(Vec3::z() * 140.0, 0.0, false)
        );
    }

    #[test]
    fn local_flight_tracks_kepler_with_real_collisions_and_keeps_relative_height() {
        let mut space = crate::space::Space::load().unwrap();
        space.configure(1);
        let simulate = |fps| {
            let mut flight = space.new_flight();
            space.approach(&mut flight, "Kepler-186f").unwrap();
            flight.heading.yaw = std::f32::consts::PI;
            let initial = flight.position;
            assert!(flight.toggle_local_navigation());
            assert_eq!(initial, flight.position);
            let reference = flight.navigation_reference().unwrap();
            assert_eq!(reference.name, "Kepler-186f");
            let altitude = reference.altitude(flight.position, flight.seconds);
            let mut previous_up = (flight.position - reference.center_at(0.0)).normalize();
            let mut covered = 0.0;
            for _ in 0..fps * 75 {
                flight.advance(1.0 / fps as f32, (1.0, 0.0), false, false);
                assert!(flight.crash.is_none());
                assert!(
                    (reference.altitude(flight.position, flight.seconds) - altitude).abs() < 0.15
                );
                let up = (flight.position - reference.center_at(flight.seconds)).normalize();
                covered += previous_up.dot(&up).clamp(-1.0, 1.0).acos();
                previous_up = up;
            }
            assert!(
                covered > std::f32::consts::TAU,
                "Debe completar una vuelta al planeta"
            );
            assert!(
                flight.camera.basis().1.dot(&previous_up) > 0.9,
                "La cámara debe seguir el horizonte local"
            );
            assert!(
                (reference.center_at(flight.seconds) - reference.center_at(0.0)).norm() > 100.0
            );
            flight
        };
        assert!((simulate(10).position - simulate(60).position).norm() < 0.5);
    }

    #[test]
    fn local_descent_respects_real_surface_collisions() {
        let mut space = crate::space::Space::load().unwrap();
        space.configure(1);
        let mut flight = space.new_flight();
        space.approach(&mut flight, "Kepler-186f").unwrap();
        assert!(flight.toggle_local_navigation());
        let reference = flight.navigation_reference().unwrap();
        flight.set_local_climb(-1.0);
        let mut touched = false;
        for _ in 0..600 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
            assert!(flight.crash.is_none());
            assert!(reference.altitude(flight.position, flight.seconds) > -0.3);
            touched |= flight.contact_notice > 0.0;
        }
        assert!(touched, "El modo local no debe omitir las colisiones");
    }
}
