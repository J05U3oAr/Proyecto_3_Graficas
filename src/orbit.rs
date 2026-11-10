use nalgebra_glm::Vec3;

pub const ORBIT_SPEED_MULTIPLIER: f32 = 3.0;

#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    pub radius: f32,
    pub speed: f32,
    pub phase: f32,
    pub inclination: f32,
}

impl Orbit {
    pub fn offset(self, seconds: f32) -> Vec3 {
        let (sin, cos) = (self.phase + seconds * self.speed).sin_cos();
        let (tilt_sin, tilt_cos) = self.inclination.sin_cos();
        Vec3::new(cos, sin * tilt_sin, sin * tilt_cos) * self.radius
    }

    pub fn velocity(self, seconds: f32) -> Vec3 {
        let (sin, cos) = (self.phase + seconds * self.speed).sin_cos();
        let (tilt_sin, tilt_cos) = self.inclination.sin_cos();
        Vec3::new(-sin, cos * tilt_sin, cos * tilt_cos) * self.radius * self.speed
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Motion {
    pub primary: Option<Orbit>,
    pub satellite: Option<Orbit>,
}

impl Motion {
    pub fn offset(self, seconds: f32) -> Vec3 {
        [self.primary, self.satellite]
            .into_iter()
            .flatten()
            .map(|orbit| orbit.offset(seconds))
            .sum()
    }

    pub fn displacement(self, seconds: f32) -> Vec3 {
        self.offset(seconds) - self.offset(0.0)
    }

    pub fn velocity(self, seconds: f32) -> Vec3 {
        [self.primary, self.satellite]
            .into_iter()
            .flatten()
            .map(|orbit| orbit.velocity(seconds))
            .sum()
    }
}

pub const FLAREON: Orbit = Orbit {
    radius: 450.0,
    speed: 0.008 * ORBIT_SPEED_MULTIPLIER,
    phase: -2.4,
    inclination: 0.025,
};
pub const SIROCO: Orbit = Orbit {
    radius: 720.0,
    speed: 0.0058 * ORBIT_SPEED_MULTIPLIER,
    phase: -0.2,
    inclination: 0.04,
};
pub const KEPLER: Orbit = Orbit {
    radius: 1160.0,
    speed: 0.0035 * ORBIT_SPEED_MULTIPLIER,
    phase: -1.35,
    inclination: -0.025,
};
pub const JOVARA: Orbit = Orbit {
    radius: 2100.0,
    speed: 0.0022 * ORBIT_SPEED_MULTIPLIER,
    phase: 3.95,
    inclination: -0.045,
};
pub const METIS: Orbit = Orbit {
    radius: 190.0,
    speed: 0.024 * ORBIT_SPEED_MULTIPLIER,
    phase: 1.0,
    inclination: 0.18,
};

pub const NIVALIS: Orbit = Orbit {
    radius: 860.0,
    speed: 0.0035 * ORBIT_SPEED_MULTIPLIER,
    phase: -1.15,
    inclination: -0.025,
};

pub const ONICE: Orbit = Orbit {
    radius: 620.0,
    speed: 0.0055 * ORBIT_SPEED_MULTIPLIER,
    phase: 0.0,
    inclination: 0.075,
};

pub const VESPERA: Orbit = Orbit {
    radius: 1120.0,
    speed: 0.0027 * ORBIT_SPEED_MULTIPLIER,
    phase: 0.8,
    inclination: -0.10,
};

pub const PRISMA: Orbit = Orbit {
    radius: 1390.0,
    speed: 0.0021 * ORBIT_SPEED_MULTIPLIER,
    phase: -0.25,
    inclination: -0.065,
};

pub const NACAR: Orbit = Orbit {
    radius: 1690.0,
    speed: 0.0017 * ORBIT_SPEED_MULTIPLIER,
    phase: 1.8,
    inclination: 0.055,
};

pub fn umbria_motion(body: usize) -> Motion {
    Motion {
        primary: match body {
            3 => Some(NIVALIS),
            9 => Some(ONICE),
            10 => Some(VESPERA),
            11 => Some(PRISMA),
            12 => Some(NACAR),
            _ => None,
        },
        satellite: None,
    }
}

pub fn aurora_motion(body: usize) -> Motion {
    Motion {
        primary: match body {
            0 => Some(SIROCO),
            2 => Some(JOVARA),
            5 => Some(FLAREON),
            6 | 7 => Some(KEPLER),
            _ => None,
        },
        satellite: (body == 7).then_some(METIS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn umbria_is_compact_spread_around_departure_and_keeps_clear_flight_gaps() {
        let bodies = [(9, 72.0), (3, 65.0), (10, 84.0), (11, 78.0), (12, 96.0)];
        let spawn = crate::flight::SPAWN_POSITION;
        let center = crate::galaxy::UMBRIA_CENTER;
        let mut front = 0;
        let mut back = 0;
        let mut bearings = Vec::new();
        for &(body, radius) in &bodies {
            let orbit = umbria_motion(body).primary.unwrap();
            assert!(
                orbit.radius <= 1750.0,
                "Las orbitas deben permitir recorridos cortos"
            );
            assert!(orbit.radius - radius - crate::galaxy::ASTERION_RADIUS > 150.0);
            let planet = center + orbit.offset(0.0);
            let direction = planet - spawn;
            assert!(
                direction.norm() < 1800.0,
                "El planeta debe quedar accesible desde la salida"
            );
            front += usize::from(direction.z < 0.0);
            back += usize::from(direction.z > 0.0);
            bearings.push(direction.x.atan2(-direction.z));
            let t = ((center - spawn).dot(&direction) / direction.norm_squared()).clamp(0.0, 1.0);
            assert!(
                (spawn + direction * t - center).norm() > crate::galaxy::ASTERION_RADIUS + 15.0,
                "El sol no debe bloquear el recorrido inicial al planeta"
            );
        }
        assert!(
            front >= 2 && back >= 2,
            "Los planetas deben rodear la salida"
        );
        for a in 0..bodies.len() {
            for b in a + 1..bodies.len() {
                let gap = (umbria_motion(bodies[a].0).primary.unwrap().radius
                    - umbria_motion(bodies[b].0).primary.unwrap().radius)
                    .abs();
                assert!(gap - bodies[a].1 - bodies[b].1 > 70.0);
                let angle = (bearings[a] - bearings[b] + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                assert!(
                    angle.abs() > 20_f32.to_radians(),
                    "No deben aparecer alineados"
                );
            }
        }
    }

    #[test]
    fn planets_keep_their_orbits_and_metis_follows_kepler() {
        for seconds in [0.0, 20.0, 120.0, 600.0, 3600.0] {
            for body in [0, 2, 5, 6] {
                let motion = aurora_motion(body);
                assert!(
                    (motion.offset(seconds).norm() - motion.primary.unwrap().radius).abs() < 0.001
                );
                let measured =
                    (motion.offset(seconds + 0.01) - motion.offset(seconds - 0.01)) / 0.02;
                assert!((measured - motion.velocity(seconds)).norm() < 0.15);
            }
            let moon = aurora_motion(7).offset(seconds);
            let planet = aurora_motion(6).offset(seconds);
            assert!(((moon - planet).norm() - METIS.radius).abs() < 0.001);
        }
        assert_eq!(aurora_motion(4).offset(100.0), Vec3::zeros());
        for (inner, outer, inner_extent, outer_extent) in [
            (0.0, FLAREON.radius, crate::galaxy::SUN_RADIUS, 48.0),
            (FLAREON.radius, SIROCO.radius, 48.0, 36.0),
            (SIROCO.radius, KEPLER.radius, 36.0, METIS.radius + 17.0),
            (KEPLER.radius, JOVARA.radius, METIS.radius + 17.0, 95.0),
        ] {
            assert!(outer - inner - inner_extent - outer_extent > 180.0);
        }
    }
}
