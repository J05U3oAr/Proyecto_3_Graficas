use crate::local_navigation::Reference;
use nalgebra_glm::Vec3;

pub const MAX_ACCELERATION: f32 = 18.0;
pub const MAX_DRIFT_SPEED: f32 = 22.0;
const DRAG: f32 = 0.18;

pub fn acceleration(bodies: &[Reference], position: Vec3, seconds: f32) -> Vec3 {
    let mut total = Vec3::zeros();
    for body in bodies {
        total += attraction(body.center_at(seconds), body.surface_radius, position);
    }
    limit(total)
}

pub fn attraction(center: Vec3, radius: f32, position: Vec3) -> Vec3 {
    let offset = center - position;
    let distance = offset.norm();
    let radius = radius.max(0.1);
    let influence = radius + (radius * 3.0).max(120.0);
    if !distance.is_finite() || distance < 0.001 || distance >= influence {
        return Vec3::zeros();
    }
    // El corte suave evita saltos al entrar en el campo; la fuerza escala con el tamano del cuerpo.
    let edge = ((influence - distance) / (influence - radius)).clamp(0.0, 1.0);
    let fade = edge * edge * (3.0 - 2.0 * edge);
    let surface = (radius * 0.17).clamp(0.03, 18.0);
    offset / distance * (surface * (radius / distance.max(radius)).powi(2) * fade)
}

pub fn limit(mut total: Vec3) -> Vec3 {
    let magnitude = total.norm();
    if magnitude > MAX_ACCELERATION {
        total *= MAX_ACCELERATION / magnitude;
    }
    total
}

pub fn advance_drift(velocity: Vec3, force: Vec3, dt: f32, brake: bool) -> Vec3 {
    // La deriva planetaria queda separada de la atraccion intensa del agujero negro.
    let drag = if brake { 2.5 } else { DRAG };
    let damping = (-drag * dt).exp();
    let mut next = velocity * damping + force * ((1.0 - damping) / drag);
    let speed = next.norm();
    if speed > MAX_DRIFT_SPEED {
        next *= MAX_DRIFT_SPEED / speed;
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body() -> Reference {
        Reference {
            name: "Prueba",
            center: Vec3::zeros(),
            radius: 60.0,
            surface_radius: 60.0,
            motion: Default::default(),
        }
    }

    #[test]
    fn field_is_radial_smooth_bounded_and_follows_the_orbit() {
        let mut planet = body();
        let near = acceleration(&[planet], Vec3::x() * 80.0, 0.0);
        let far = acceleration(&[planet], Vec3::x() * 160.0, 0.0);
        assert!(near.x < far.x && far.x < 0.0 && near.y == 0.0 && near.z == 0.0);
        assert_eq!(
            acceleration(&[planet], Vec3::x() * 240.0, 0.0),
            Vec3::zeros()
        );
        assert!(acceleration(&[planet], Vec3::x() * 239.99, 0.0).norm() < 0.00001);
        assert_eq!(acceleration(&[planet], Vec3::zeros(), 0.0), Vec3::zeros());
        assert!(
            acceleration(&[planet; 30], Vec3::x() * 20.0, 0.0).norm() <= MAX_ACCELERATION + 0.00001
        );
        planet.motion = crate::orbit::aurora_motion(6);
        let point = planet.center_at(40.0) + Vec3::x() * 80.0;
        assert!((acceleration(&[planet], point, 40.0) - near).norm() < 0.0001);
        assert_eq!(acceleration(&[planet], point, 0.0), Vec3::zeros());
        let hole = crate::black_hole::BLACK_HOLE;
        assert!(
            hole.acceleration(hole.position + Vec3::x() * 80.0).norm() > MAX_ACCELERATION * 4.0
        );
    }

    #[test]
    fn drift_is_independent_of_frame_rate_brakes_and_never_overwhelms_thrusters() {
        let simulate = |fps, brake| {
            let mut velocity = Vec3::zeros();
            for _ in 0..fps * 12 {
                velocity = advance_drift(velocity, Vec3::x() * 3.0, 1.0 / fps as f32, brake);
            }
            velocity
        };
        assert!((simulate(10, false) - simulate(120, false)).norm() < 0.001);
        assert!(simulate(60, true).norm() < simulate(60, false).norm() * 0.2);
        assert!(simulate(60, false).norm() <= MAX_DRIFT_SPEED);
    }
}
