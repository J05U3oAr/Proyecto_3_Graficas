//! A cinematic intergalactic gate, separate from the ship's two local portals.
use crate::{
    flight::Flight, framebuffer::Framebuffer, galaxy::GALAXIES, noise, portal::Portal,
    renderer::FlightView, ui::mix,
};
use nalgebra_glm::Vec3;
use rayon::prelude::*;
use std::sync::OnceLock;

pub const RADIUS: f32 = 42.0;
pub const DISTANCE: f32 = 110.0;
pub const OPEN_TIME: f32 = 0.85;
pub const DURATION: f32 = 4.2;

pub struct Jump {
    pub origin: usize,
    pub destination: usize,
    pub portal: Portal,
    age: f32,
    entry_time: Option<f32>,
}

impl Jump {
    pub fn new(origin: usize, flight: &Flight) -> Option<Self> {
        if origin >= GALAXIES.len() {
            return None;
        }
        let forward = flight.heading.basis().2;
        let center = flight.position + forward * DISTANCE;
        if !flight.can_open_galactic_gate(center, RADIUS) {
            return None;
        }
        Some(Self {
            origin,
            destination: (origin + 1) % GALAXIES.len(),
            portal: Portal::new(center, -forward)?,
            age: 0.0,
            entry_time: None,
        })
    }

    pub fn preview(
        origin: usize,
        destination: usize,
        flight: &Flight,
        seconds: f32,
        ready: bool,
    ) -> Option<Self> {
        if destination >= GALAXIES.len() || destination == origin || !seconds.is_finite() {
            return None;
        }
        let mut jump = Self::new(origin, flight)?;
        jump.destination = destination;
        jump.age = if ready {
            seconds.max(0.0)
        } else {
            OPEN_TIME + seconds.max(0.0)
        };
        jump.entry_time = if ready {
            None
        } else {
            Some(seconds.clamp(0.0, DURATION))
        };
        Some(jump)
    }

    pub fn entering(&self) -> bool {
        self.entry_time.is_some()
    }
    pub fn ready(&self) -> bool {
        self.age >= OPEN_TIME
    }
    pub fn opening(&self) -> f32 {
        noise::smoothstep(0.0, OPEN_TIME, self.age)
    }
    pub fn progress(&self) -> f32 {
        self.entry_time.unwrap_or(0.0) / DURATION
    }
    pub fn ship_visible(&self) -> bool {
        self.progress() < 0.70
    }

    pub fn select(&mut self, step: i32) {
        if self.entering() || step == 0 {
            return;
        }
        let count = GALAXIES.len() as i32;
        let mut index = self.destination as i32;
        loop {
            index = (index + step.signum()).rem_euclid(count);
            if index as usize != self.origin {
                break;
            }
        }
        self.destination = index as usize;
    }

    pub fn begin(&mut self) -> bool {
        if !self.ready() || self.entering() {
            return false;
        }
        self.entry_time = Some(0.0);
        true
    }

    pub fn advance(&mut self, dt: f32) -> bool {
        if !dt.is_finite() || dt <= 0.0 {
            return false;
        }
        let dt = dt.min(0.1);
        self.age += dt;
        if let Some(time) = &mut self.entry_time {
            *time = (*time + dt).min(DURATION);
            return *time >= DURATION;
        }
        false
    }

    pub fn cinematic(&self, source: &Flight, width: usize, height: usize) -> (Flight, FlightView) {
        let p = self.progress();
        let forward = -self.portal.normal;
        let mut ship = Flight::new();
        ship.position =
            source.position + forward * ((DISTANCE + 12.0) * noise::smoothstep(0.0, 0.70, p));
        ship.heading = source.heading;
        ship.camera = source.camera;
        ship.bank = source.bank * (1.0 - noise::smoothstep(0.0, 0.4, p));
        ship.black_hole = source.black_hole;
        ship.seconds = source.seconds;
        ship.velocity = forward * (p * 100.0);
        let start = source.camera_position();
        let end = self.portal.center + forward * 12.0;
        let eye = start + (end - start) * noise::smoothstep(0.40, 0.94, p);
        let align = self.opening();
        let direction = source.camera.basis().2 * (1.0 - align)
            + (self.portal.center - start).normalize() * align;
        let view = FlightView::new(source, width, height).looking_at(eye, eye + direction);
        (ship, view)
    }

    pub fn stage(&self) -> &'static str {
        if !self.ready() {
            "ABRIENDO PORTAL"
        } else if !self.entering() {
            "ELIGE OTRA GALAXIA"
        } else if self.progress() < 0.65 {
            "ENTRANDO AL PORTAL"
        } else {
            "CRUZANDO ENTRE GALAXIAS"
        }
    }
}

const SIZE: usize = 512;

fn textures() -> &'static [Vec<u32>; 3] {
    static TEXTURES: OnceLock<[Vec<u32>; 3]> = OnceLock::new();
    TEXTURES.get_or_init(|| {
        // Initialize the shared art before dispatching workers that sample it.
        crate::galaxy_art::prepare();
        std::array::from_fn(|destination| {
            let mut pixels = vec![0; SIZE * SIZE];
            pixels.par_iter_mut().enumerate().for_each(|(i, pixel)| {
                let x = (i % SIZE) as f32 / (SIZE - 1) as f32 * 2.0 - 1.0;
                let y = (i / SIZE) as f32 / (SIZE - 1) as f32 * 2.0 - 1.0;
                let cloud =
                    noise::fbm(Vec3::new(x * 3.6, y * 3.6, 19.0 + destination as f32 * 7.0));
                let gas = noise::smoothstep(0.25, 0.8, cloud);
                let base = mix(0x080818, mix(0x67239e, 0x287dbc, cloud), gas * 0.72);
                let galaxy =
                    crate::galaxy_art::sample(GALAXIES[destination].artwork, x * 0.86, y * 0.86);
                let star = noise::hash(
                    (i % SIZE) as i32,
                    (i / SIZE) as i32,
                    destination as i32 + 83,
                );
                let sparkle = if star > 0.997 {
                    ((star - 0.997) / 0.003).powf(0.5) * 240.0
                } else {
                    0.0
                };
                let channel = |shift: u32| {
                    (((base >> shift) & 255) as f32
                        + ((galaxy >> shift) & 255) as f32 * 1.35
                        + sparkle)
                        .min(255.0) as u32
                };
                *pixel = channel(16) << 16 | channel(8) << 8 | channel(0);
            });
            pixels
        })
    })
}

pub fn prepare() {
    let _ = textures();
}

fn sample(image: &[u32], x: f32, y: f32) -> u32 {
    let x = ((x * 0.5 + 0.5) * (SIZE - 1) as f32).clamp(0.0, (SIZE - 2) as f32);
    let y = ((y * 0.5 + 0.5) * (SIZE - 1) as f32).clamp(0.0, (SIZE - 2) as f32);
    let (left, top) = (x.floor() as usize, y.floor() as usize);
    let (u, v) = (x - left as f32, y - top as f32);
    mix(
        mix(image[top * SIZE + left], image[top * SIZE + left + 1], u),
        mix(
            image[(top + 1) * SIZE + left],
            image[(top + 1) * SIZE + left + 1],
            u,
        ),
        v,
    )
}

pub fn draw(fb: &mut Framebuffer, jump: &Jump, view: &FlightView) {
    let texture = &textures()[jump.destination];
    let radius = RADIUS * jump.opening();
    if radius < 0.05 {
        return;
    }
    let reach = radius * 1.25;
    if !view.sphere_visible(jump.portal.center, reach) {
        return;
    }
    let mut bounds = (fb.width, fb.height, 0, 0);
    for (x, y) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        if let Some(p) =
            view.project(jump.portal.center + (jump.portal.right * x + jump.portal.up * y) * reach)
        {
            bounds.0 = bounds.0.min((p.x - 3.0).max(0.0) as usize);
            bounds.1 = bounds.1.min((p.y - 3.0).max(0.0) as usize);
            bounds.2 = bounds
                .2
                .max((p.x + 4.0).max(0.0).min(fb.width as f32) as usize);
            bounds.3 = bounds
                .3
                .max((p.y + 4.0).max(0.0).min(fb.height as f32) as usize);
        } else {
            bounds = (0, 0, fb.width, fb.height);
            break;
        }
    }
    let eye = view.eye();
    let delta = jump.portal.center - eye;
    let plane = delta.dot(&jump.portal.normal);
    let (sin, cos) = (jump.age * 0.08).sin_cos();
    fb.shade_rect(bounds, |x, y, pixel, depth| {
        let ray = view.ray(x as f32 + 0.5, y as f32 + 0.5);
        let denominator = ray.dot(&jump.portal.normal);
        if denominator.abs() < 1e-6 {
            return;
        }
        let distance = plane / denominator;
        if distance < 0.25 || 1.0 / distance <= *depth {
            return;
        }
        let offset = ray * distance - delta;
        let u = offset.dot(&jump.portal.right) / radius;
        let v = offset.dot(&jump.portal.up) / radius;
        let r = (u * u + v * v).sqrt();
        if r > 1.25 {
            return;
        }
        let angle = v.atan2(u);
        if r < 0.965 {
            *pixel = sample(texture, u * cos - v * sin, u * sin + v * cos);
            *depth = 1.0 / distance;
        }
        let inner = (-((r - 0.965) / 0.018).powi(2)).exp();
        let outer = (-((r - 1.055) / 0.008).powi(2)).exp();
        let halo = (-((r - 1.0) / 0.13).powi(2)).exp() * 0.36;
        let orbit = (angle * 32.0 - jump.age * 1.5).sin().max(0.0).powi(8);
        *pixel = crate::framebuffer::emission_color(*pixel, 0x7d45ff, halo + outer * 0.65, 0.0);
        *pixel = crate::framebuffer::emission_color(
            *pixel,
            0x93f4ff,
            inner * (0.8 + orbit * 0.2) + outer * orbit * 0.6,
            0.0,
        );
    });
}

pub fn finish(fb: &mut Framebuffer, jump: &Jump, arrival: &Framebuffer) {
    let p = jump.progress();
    let flash = (-((p - 0.88) / 0.075).powi(2)).exp() * 0.88;
    let reveal = noise::smoothstep(0.80, 1.0, p);
    fb.pixels
        .par_iter_mut()
        .zip(&arrival.pixels)
        .for_each(|(pixel, &target)| {
            *pixel = mix(mix(*pixel, 0xdbd6ff, flash), target, reveal);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_selection_excludes_origin_and_locks_while_entering() {
        for origin in 0..GALAXIES.len() {
            let mut jump = Jump::new(origin, &Flight::new()).unwrap();
            assert!(!jump.begin());
            for _ in 0..10 {
                jump.select(1);
                assert_ne!(jump.destination, origin);
            }
            jump.select(-1);
            for _ in 0..10 {
                jump.advance(0.1);
            }
            assert!(jump.begin());
            let chosen = jump.destination;
            jump.select(1);
            assert_eq!(jump.destination, chosen);
            assert!(!jump.begin());
        }
    }

    #[test]
    fn entry_has_finite_poses_at_different_frame_rates_and_ends_on_exact_arrival() {
        let source = Flight::new();
        for fps in [10, 60] {
            let mut jump = Jump::preview(0, 1, &source, 0.0, false).unwrap();
            assert!(!jump.advance(f32::NAN));
            let mut done = false;
            for _ in 0..fps * 5 {
                done = jump.advance(1.0 / fps as f32);
                let (ship, view) = jump.cinematic(&source, 128, 84);
                assert!(
                    ship.position
                        .iter()
                        .chain(view.eye().iter())
                        .all(|v| v.is_finite())
                );
                if done {
                    break;
                }
            }
            assert!(done);
            assert!(!jump.ship_visible());
            let mut image = Framebuffer::new(128, 84);
            let mut arrival = Framebuffer::new(128, 84);
            arrival.pixels.fill(0x876543);
            finish(&mut image, &jump, &arrival);
            assert_eq!(image.pixels, arrival.pixels);
        }
        assert_eq!(source.position, Vec3::zeros());
        assert_eq!(source.seconds, 0.0);
    }

    #[test]
    fn ship_crosses_the_gate_before_the_camera_and_blocked_routes_are_rejected() {
        let mut source = Flight::new();
        let jump = Jump::preview(0, 1, &source, DURATION * 0.7, false).unwrap();
        let (ship, view) = jump.cinematic(&source, 1280, 840);
        assert!((ship.position - jump.portal.center).dot(&jump.portal.normal) < 0.0);
        assert!((view.eye() - jump.portal.center).dot(&jump.portal.normal) > 0.0);
        source.black_hole.position = jump.portal.center;
        assert!(Jump::new(0, &source).is_none());
    }

    #[test]
    fn galactic_design_is_repeatable_distinct_and_occluded_by_foreground() {
        let source = Flight::new();
        let a = Jump::preview(0, 1, &source, 1.0, true).unwrap();
        let b = Jump::preview(0, 2, &source, 1.0, true).unwrap();
        let (_, view) = a.cinematic(&source, 320, 240);
        let mut first = Framebuffer::new(320, 240);
        let mut same = Framebuffer::new(320, 240);
        let mut other = Framebuffer::new(320, 240);
        draw(&mut first, &a, &view);
        draw(&mut same, &a, &view);
        draw(&mut other, &b, &view);
        assert_eq!(first.pixels, same.pixels);
        assert_ne!(first.pixels, other.pixels);
        assert!(
            first
                .pixels
                .iter()
                .filter(|&&p| p != crate::framebuffer::BACKGROUND)
                .count()
                > 20000
        );
        first.pixels.fill(0x123456);
        first.depth.fill(f32::INFINITY);
        draw(&mut first, &a, &view);
        assert!(first.pixels.iter().all(|&p| p == 0x123456));
    }
}
