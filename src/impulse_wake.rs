//! Red hyperspace wake in world space. Sampling never changes flight physics.
use crate::{
    framebuffer::Framebuffer,
    renderer::{FLIGHT_FOCAL, FlightView},
};
use nalgebra_glm::{Vec2, Vec3};
use std::collections::VecDeque;

const LIFETIME: f32 = 0.9;
const MAX_SAMPLES: usize = 144;

struct Sample {
    position: Vec3,
    right: Vec3,
    age: f32,
}
struct Wave {
    center: Vec3,
    right: Vec3,
    up: Vec3,
    age: f32,
}
#[derive(Default)]
pub struct Wake {
    samples: VecDeque<Sample>,
    wave: Option<Wave>,
}

impl Wake {
    pub fn launch(&mut self, position: Vec3, right: Vec3, up: Vec3) {
        // Consecutive jumps retain the previous wake until it fades naturally.
        self.wave = Some(Wave {
            center: position,
            right,
            up,
            age: 0.0,
        });
        self.record(position, right);
    }
    pub fn clear(&mut self) {
        self.samples.clear();
        self.wave = None;
    }
    pub fn origin(&self) -> Option<Vec3> {
        self.wave
            .as_ref()
            .map(|wave| wave.center)
            .or_else(|| self.samples.front().map(|sample| sample.position))
    }
    pub fn tick(&mut self, dt: f32) {
        for sample in &mut self.samples {
            sample.age += dt;
        }
        while self.samples.front().is_some_and(|s| s.age >= LIFETIME) {
            self.samples.pop_front();
        }
        if let Some(wave) = &mut self.wave {
            wave.age += dt;
            if wave.age >= LIFETIME {
                self.wave = None;
            }
        }
    }
    pub fn record(&mut self, position: Vec3, right: Vec3) {
        if self
            .samples
            .back()
            .is_some_and(|s| (s.position - position).norm() < 0.12)
        {
            return;
        }
        if self.samples.len() == MAX_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(Sample {
            position,
            right,
            age: 0.0,
        });
    }
    pub fn draw(&self, fb: &mut Framebuffer, view: &FlightView) {
        let mut previous: Option<&Sample> = None;
        for sample in &self.samples {
            if let Some(old) = previous {
                let fade = (1.0 - old.age / LIFETIME).clamp(0.0, 1.0);
                // Two slender ribbons leave the rear of the hull; they do not bend stars.
                for side in [-1.0, 1.0] {
                    let start = old.position + old.right * (side * 1.05);
                    let end = sample.position + sample.right * (side * 1.05);
                    ribbon(fb, view, start, end, 0.065, fade * 0.45);
                }
            }
            previous = Some(sample);
        }
        if let Some(wave) = &self.wave {
            let p = (wave.age / LIFETIME).clamp(0.0, 1.0);
            let radius = 1.4 + 9.0 * p;
            for index in 0..96 {
                let point = |i: usize| {
                    let a = i as f32 * std::f32::consts::TAU / 96.0;
                    wave.center + (wave.right * a.cos() + wave.up * a.sin()) * radius
                };
                ribbon(
                    fb,
                    view,
                    point(index),
                    point(index + 1),
                    0.035,
                    (1.0 - p).powi(2) * 0.32,
                );
            }
        }
    }
}

fn projected_segment(view: &FlightView, start: Vec3, end: Vec3) -> Option<(Vec3, Vec3)> {
    let (mut a, mut b) = (view.view_point(start), view.view_point(end));
    // Clip to the view before projecting: a wake may cross behind the chase camera.
    const NEAR: f32 = -0.8;
    if a.z > NEAR && b.z > NEAR {
        return None;
    }
    if a.z > NEAR {
        a = a + (b - a) * ((NEAR - a.z) / (b.z - a.z));
    }
    if b.z > NEAR {
        b = b + (a - b) * ((NEAR - b.z) / (a.z - b.z));
    }
    Some((view.project_view(a)?, view.project_view(b)?))
}

fn ribbon(
    fb: &mut Framebuffer,
    view: &FlightView,
    start: Vec3,
    end: Vec3,
    radius: f32,
    strength: f32,
) {
    let Some((a, b)) = projected_segment(view, start, end) else {
        return;
    };
    let width = (radius * FLIGHT_FOCAL * a.z.max(b.z)).clamp(0.65, 5.0);
    let reach = width * 3.5;
    let bounds = (
        (a.x.min(b.x) - reach).max(0.0) as usize,
        (a.y.min(b.y) - reach).max(0.0) as usize,
        (a.x.max(b.x) + reach).max(0.0).ceil() as usize,
        (a.y.max(b.y) + reach).max(0.0).ceil() as usize,
    );
    let delta = Vec2::new(b.x - a.x, b.y - a.y);
    fb.shade_rect(bounds, |x, y, pixel, depth| {
        let relative = Vec2::new(x as f32 + 0.5 - a.x, y as f32 + 0.5 - a.y);
        let t = (relative.dot(&delta) / delta.norm_squared().max(0.001)).clamp(0.0, 1.0);
        let d2 = (relative - delta * t).norm_squared() / width.powi(2);
        if a.z + (b.z - a.z) * t > *depth && d2 < 12.0 {
            *pixel = crate::framebuffer::emission_color(
                *pixel,
                0xff163b,
                (-d2 * 0.4).exp() * strength,
                0.0,
            );
            *pixel = crate::framebuffer::emission_color(
                *pixel,
                0xff7974,
                (-d2 * 3.0).exp() * strength * 0.6,
                0.0,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wake_stays_in_world_space_fades_and_respects_foreground_depth() {
        let mut wake = Wake::default();
        wake.launch(Vec3::zeros(), Vec3::x(), Vec3::y());
        for i in 1..35 {
            wake.tick(1.0 / 120.0);
            wake.record(-Vec3::z() * i as f32, Vec3::x());
        }
        let oldest = wake.samples.front().unwrap().position;
        let flight = crate::flight::Flight::new();
        let view = FlightView::new(&flight, 640, 420)
            .looking_at(Vec3::new(20.0, 12.0, 15.0), -Vec3::z() * 15.0);
        let mut fb = Framebuffer::new(640, 420);
        let original = fb.pixels.clone();
        wake.draw(&mut fb, &view);
        assert!(
            fb.pixels
                .iter()
                .zip(&original)
                .filter(|(a, b)| a != b)
                .count()
                > 100
        );
        let red_gain: u64 = fb
            .pixels
            .iter()
            .zip(&original)
            .map(|(a, b)| (((a >> 16) & 255) - ((b >> 16) & 255)) as u64)
            .sum();
        let blue_gain: u64 = fb
            .pixels
            .iter()
            .zip(&original)
            .map(|(a, b)| ((a & 255) - (b & 255)) as u64)
            .sum();
        assert!(
            red_gain > blue_gain * 3,
            "The light added by the wake must be predominantly red"
        );
        assert!(fb.depth.iter().all(|d| *d == f32::NEG_INFINITY));
        assert_eq!(wake.samples.front().unwrap().position, oldest);
        fb.pixels.fill(0x123456);
        fb.depth.fill(f32::INFINITY);
        wake.draw(&mut fb, &view);
        assert!(fb.pixels.iter().all(|p| *p == 0x123456));
        wake.launch(-Vec3::z() * 34.0, Vec3::x(), Vec3::y());
        assert_eq!(
            wake.samples.front().unwrap().position,
            oldest,
            "Chained jumps must retain their previous trail"
        );
        wake.tick(LIFETIME);
        assert!(wake.samples.is_empty() && wake.wave.is_none());
    }
}
