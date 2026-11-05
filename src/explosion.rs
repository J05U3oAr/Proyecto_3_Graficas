//! Deterministic world-space fire, sparks and smoke; opaque planets hide debris.
use crate::{
    flight::Crash,
    framebuffer::Framebuffer,
    renderer::{FLIGHT_FOCAL, FlightView},
};
use nalgebra_glm::Vec3;
use std::f32::consts::TAU;

struct Particle {
    position: Vec3,
    radius: f32,
    color: u32,
    alpha: f32,
}

fn random(index: usize, salt: f32) -> f32 {
    ((index as f32 * 127.1 + salt * 311.7).sin() * 43_758.547)
        .fract()
        .abs()
}

fn sprite(fb: &mut Framebuffer, view: &FlightView, particle: &Particle) {
    let Some(center) = view.project(particle.position) else {
        return;
    };
    let radius = (particle.radius * FLIGHT_FOCAL * center.z).max(0.8);
    let left = (center.x - radius).max(0.0).min(fb.width as f32) as usize;
    let right = (center.x + radius).ceil().max(0.0).min(fb.width as f32) as usize;
    let top = (center.y - radius).max(0.0).min(fb.height as f32) as usize;
    let bottom = (center.y + radius).ceil().max(0.0).min(fb.height as f32) as usize;
    for y in top..bottom {
        for x in left..right {
            let r2 = ((x as f32 + 0.5 - center.x).powi(2) + (y as f32 + 0.5 - center.y).powi(2))
                / radius.powi(2);
            if r2 < 1.0 {
                fb.blend_depth(
                    x,
                    y,
                    center.z,
                    particle.color,
                    (1.0 - r2).powi(2) * particle.alpha,
                );
            }
        }
    }
}

pub fn draw(fb: &mut Framebuffer, view: &FlightView, crash: &Crash) {
    let normal = crash.normal;
    let axis = if normal.y.abs() < 0.9 {
        Vec3::y()
    } else {
        Vec3::x()
    };
    let right = normal.cross(&axis).normalize();
    let up = normal.cross(&right);
    // Hard-edged hull fragments remain distinct from the soft fire and smoke.
    for i in 0..26 {
        let angle = i as f32 * 2.399_963;
        let direction =
            normal * (0.8 + random(i, 7.0)) + (right * angle.cos() + up * angle.sin()) * 1.3;
        let center = crash.position + direction * (2.0 + random(i, 8.0) * 5.0) * crash.age;
        let spin = angle + crash.age * (3.0 + random(i, 9.0) * 8.0);
        let size = (0.12 + random(i, 10.0) * 0.18) * (1.0 - crash.age / 2.0).max(0.0);
        let a = (right * spin.cos() + up * spin.sin()) * size;
        let b = (normal * (spin * 1.7).cos() + up * (spin * 1.7).sin()) * size;
        if let (Some(a), Some(b), Some(c)) = (
            view.project(center + a),
            view.project(center - a + b),
            view.project(center - b),
        ) {
            let color = if crash.age < 0.18 {
                0xffdf8a
            } else {
                [0x776b60, 0xb28b62, 0x403f43][i % 3]
            };
            let color =
                crate::color::Color::new((color >> 16) as u8, (color >> 8) as u8, color as u8);
            crate::triangle::fill(fb, &a, &b, &c, color);
        }
    }
    let mut particles = Vec::with_capacity(140);
    for i in 0..96 {
        let angle = i as f32 * 2.399_963;
        let height = random(i, 1.0);
        let radial = (1.0 - height * height).sqrt();
        let direction =
            normal * (0.2 + height * 0.8) + (right * angle.cos() + up * angle.sin()) * radial;
        let speed = 4.0 + random(i, 2.0) * 18.0;
        let lifetime = 0.45 + random(i, 3.0) * 0.95;
        let age = crash.age;
        if age < lifetime {
            let fade = 1.0 - age / lifetime;
            let position = crash.position + direction * speed * age;
            particles.push(Particle {
                position,
                radius: (0.30 + random(i, 4.0) * 0.5) * (0.5 + age),
                color: if age < 0.15 {
                    0xffedb0
                } else if fade > 0.45 {
                    0xff9c2f
                } else {
                    0xcc461c
                },
                alpha: fade * 0.95,
            });
            // Short incandescent trails make the outward fragments readable.
            for tail in 1..=2 {
                particles.push(Particle {
                    position: position - direction * tail as f32 * 0.22,
                    radius: 0.09,
                    color: 0xffdc86,
                    alpha: fade * 0.55,
                });
            }
        }
    }
    for i in 0..18 {
        let angle = i as f32 * TAU / 18.0;
        let direction = normal * 2.5 + right * angle.cos() * 2.0 + up * angle.sin() * 2.0;
        particles.push(Particle {
            position: crash.position + direction * crash.age,
            radius: 0.45 + crash.age * (0.8 + random(i, 5.0)),
            color: 0x363c48,
            alpha: (crash.age * 2.0).min(1.0) * (1.0 - crash.age / 2.0).max(0.0) * 0.55,
        });
    }
    if crash.age < 0.42 {
        particles.push(Particle {
            position: crash.position + normal * 0.4,
            radius: 1.2 + crash.age * 9.0,
            color: 0xffd07a,
            alpha: (1.0 - crash.age / 0.42) * 0.95,
        });
    }
    if crash.age < 0.25 {
        particles.push(Particle {
            position: crash.position + normal * 0.7,
            radius: 1.0 + crash.age * 4.0,
            color: 0xfff6ce,
            alpha: 1.0 - crash.age / 0.25,
        });
    }
    particles.sort_by(|a, b| {
        view.view_point(a.position)
            .z
            .total_cmp(&view.view_point(b.position).z)
    });
    for particle in &particles {
        sprite(fb, view, particle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::Flight;

    #[test]
    fn explosion_animates_without_drawing_through_foreground_geometry() {
        let flight = Flight::new();
        let view = FlightView::new(&flight, 320, 240);
        let mut crash = Crash {
            position: Vec3::new(0.0, 0.0, -12.0),
            normal: Vec3::z(),
            age: 0.12,
            planet: "Test",
        };
        let mut fb = Framebuffer::new(320, 240);
        draw(&mut fb, &view, &crash);
        let first = fb.pixels.clone();
        assert!(first.iter().any(|p| ((p >> 16) & 255) > 150));
        fb.clear();
        crash.age = 0.7;
        draw(&mut fb, &view, &crash);
        assert_ne!(first, fb.pixels);
        for y in 0..240 {
            for x in 0..320 {
                fb.point(x, y, 1.0, 0x123456, true);
            }
        }
        draw(&mut fb, &view, &crash);
        assert!(fb.pixels.iter().all(|p| *p == 0x123456));
    }
}
