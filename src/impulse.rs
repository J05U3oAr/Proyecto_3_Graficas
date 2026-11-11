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
    }

    pub fn step(&mut self, dt: f32) -> Vec3 {
        let active = self.remaining.min(dt);
        self.remaining = (self.remaining - dt).max(0.0);
        // La fraccion del ultimo paso conserva la distancia aunque cambie la tasa de cuadros.
        self.direction * self.speed * (active / dt)
    }

    pub fn cancel(&mut self) {
        self.remaining = 0.0;
    }

    pub fn visual_strength(&self) -> f32 {
        if self.remaining <= 0.0 {
            return 0.0;
        }
        let p = (1.0 - self.remaining / DURATION).clamp(0.0, 1.0);
        (p * std::f32::consts::PI).sin().max(0.0).sqrt()
            * if self.speed <= LOCAL_SPEED { 0.45 } else { 1.0 }
    }

    pub fn draw_background(
        &self,
        fb: &mut crate::framebuffer::Framebuffer,
        camera: &crate::camera::Camera,
        seconds: f32,
    ) {
        let strength = self.visual_strength();
        if strength <= 0.0 {
            return;
        }
        let (right, up, forward) = camera.basis();
        let (dx, dy, dz) = (
            self.direction.dot(&right),
            self.direction.dot(&up),
            self.direction.dot(&forward),
        );
        let (width, height) = (fb.width as f32, fb.height as f32);
        let p = (1.0 - self.remaining / DURATION).clamp(0.0, 1.0);
        let flash = (-((p - 0.08) / 0.07).powi(2)).exp() * 0.10
            + (-((p - 0.92) / 0.07).powi(2)).exp() * 0.07;
        // La luz y las estelas se dibujan detras de la geometria para conservar la visibilidad del terreno.
        fb.shade_rect((0, 0, fb.width, fb.height), |x, y, pixel, _| {
            let edge = ((x as f32 / width - 0.5).powi(2) + (y as f32 / height - 0.5).powi(2)) * 2.0;
            *pixel = crate::framebuffer::emission_color(
                *pixel,
                0x99dfff,
                flash + edge * strength * 0.055,
                0.0,
            );
        });
        for index in 0..180 {
            let x =
                (crate::noise::hash(index, 41, 91) - seconds * dx * 0.9).rem_euclid(1.0) * width;
            let y =
                (crate::noise::hash(index, 73, 13) + seconds * dy * 0.9).rem_euclid(1.0) * height;
            let motion = Vec3::new(
                (x - width * 0.5) * dz - dx * 650.0,
                (y - height * 0.5) * dz + dy * 650.0,
                0.0,
            );
            let Some(direction) = motion.try_normalize(0.01) else {
                continue;
            };
            let length = (25.0 + crate::noise::hash(index, 19, 7) * 100.0) * strength;
            let t = (seconds * (1.8 + crate::noise::hash(index, 61, 5))
                + crate::noise::hash(index, 11, 97))
            .fract();
            let head = Vec3::new(x, y, 0.0) + direction * (t - 0.5) * 130.0;
            let tail = head - direction * length;
            let color = crate::ui::mix(
                0x0e2940,
                0xd6f6ff,
                strength * (0.35 + crate::noise::hash(index, 3, 47) * 0.60),
            );
            crate::ui::rule(
                fb,
                (tail.x as i32, tail.y as i32),
                (head.x as i32, head.y as i32),
                color,
            );
        }
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
