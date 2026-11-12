use nalgebra_glm::Vec3;

pub const DURATION: f32 = 0.65;
pub const COOLDOWN: f32 = 2.5;
pub const SPACE_SPEED: f32 = 300.0;
const LOCAL_SPEED: f32 = 96.0;

#[derive(Default)]
pub struct Impulse {
    pub remaining: f32,
    pub cooldown: f32,
    pub direction: Vec3,
    speed: f32,
    pub wake: crate::impulse_wake::Wake,
}

impl Impulse {
    pub fn trigger(&mut self, direction: Vec3, local: bool) -> bool {
        if self.cooldown > 0.00001
            || self.remaining > 0.0
            || !direction.iter().all(|v| v.is_finite())
        {
            return false;
        }
        let Some(direction) = direction.try_normalize(0.00001) else {
            return false;
        };
        self.direction = direction;
        self.speed = if local { LOCAL_SPEED } else { SPACE_SPEED };
        self.remaining = DURATION;
        self.cooldown = COOLDOWN;
        true
    }

    pub fn tick(&mut self, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.wake.tick(dt);
    }

    pub fn step(&mut self, dt: f32) -> Vec3 {
        let active = self.remaining.min(dt);
        self.remaining = (self.remaining - dt).max(0.0);
        // La fraccion del ultimo paso conserva la distancia aunque cambie la tasa de cuadros.
        self.direction * self.speed * (active / dt)
    }

    pub fn cancel(&mut self) {
        self.remaining = 0.0;
        self.wake.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_steps_preserve_distance_and_diagonals_do_not_increase_speed() {
        for fps in [10, 60, 144] {
            let mut impulse = Impulse::default();
            assert!(impulse.trigger(Vec3::new(1.0, 0.0, 1.0), false));
            assert!(!impulse.trigger(Vec3::x(), false));
            let mut delta = Vec3::zeros();
            for _ in 0..fps {
                delta += impulse.step(1.0 / fps as f32) / fps as f32;
                impulse.tick(1.0 / fps as f32);
            }
            assert!((delta.norm() - SPACE_SPEED * DURATION).abs() < 0.001);
            assert!((impulse.cooldown - (COOLDOWN - 1.0)).abs() < 0.001);
            impulse.tick(COOLDOWN);
            assert!(impulse.trigger(-Vec3::x(), true));
            assert!(impulse.step(0.01).norm() < SPACE_SPEED);
        }
    }
}
