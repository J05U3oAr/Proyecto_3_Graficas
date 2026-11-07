//! Intersecciones de rayos con el horizonte y el disco de acrecion.
use crate::{
    black_hole::{BLACK_HOLE, BlackHole},
    flight::Flight,
    framebuffer::Framebuffer,
    renderer::{FLIGHT_FOCAL, FlightView},
};
use nalgebra_glm::Vec3;
use std::error::Error;

pub struct DeepSpace {
    hole: BlackHole,
}

fn sphere_hit(origin: Vec3, direction: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let relative = origin - center;
    let b = relative.dot(&direction);
    let discriminant = b * b - relative.norm_squared() + radius * radius;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let near = -b - root;
    let far = -b + root;
    if near > 0.25 {
        Some(near)
    } else if far > 0.25 {
        Some(far)
    } else {
        None
    }
}

impl DeepSpace {
    pub fn new() -> Self {
        Self { hole: BLACK_HOLE }
    }

    pub fn draw(&self, fb: &mut Framebuffer, flight: &Flight) {
        let view = FlightView::new(flight, fb.width, fb.height);
        if !view.sphere_visible(self.hole.position, self.hole.disk_radius) {
            return;
        }
        let eye = flight.camera_position();
        let (right, up, forward) = flight.camera.basis();
        let relative = self.hole.position - eye;
        let depth = relative.dot(&forward);
        let bounds = if depth > self.hole.disk_radius * 1.5 {
            let center = view.project(self.hole.position).unwrap();
            // El limite incluye el borde cercano del disco inclinado.
            let size = self.hole.disk_radius * FLIGHT_FOCAL / (depth - self.hole.disk_radius);
            (
                (center.x - size).max(0.0).min(fb.width as f32) as usize,
                (center.y - size).max(0.0).min(fb.height as f32) as usize,
                (center.x + size).ceil().max(0.0).min(fb.width as f32) as usize,
                (center.y + size).ceil().max(0.0).min(fb.height as f32) as usize,
            )
        } else {
            (0, 0, fb.width, fb.height)
        };
        let normal = Vec3::new(0.025, 0.997, 0.072).normalize();
        let disk_axis = normal.cross(&Vec3::z()).normalize();
        let disk_up = normal.cross(&disk_axis);
        let plane_distance = relative.dot(&normal);
        let width = fb.width as f32;
        let height = fb.height as f32;
        fb.shade_rect(bounds, |x, y, pixel, depth_buffer| {
            let direction = (forward
                + right * ((x as f32 + 0.5 - width * 0.5) / FLIGHT_FOCAL)
                + up * ((height * 0.5 - y as f32 - 0.5) / FLIGHT_FOCAL))
                .normalize();
            let cosine = direction.dot(&forward);
            let horizon = sphere_hit(eye, direction, self.hole.position, self.hole.horizon);
            if let Some(t) = horizon {
                let z = 1.0 / (t * cosine);
                if z > *depth_buffer {
                    *pixel = 0x010102;
                    *depth_buffer = z;
                }
            }
            let denominator = direction.dot(&normal);
            if denominator.abs() > 0.0001 {
                let t = plane_distance / denominator;
                if t > 0.25 && horizon.is_none_or(|hit| t < hit) {
                    let point = eye + direction * t - self.hole.position;
                    let r = point.norm();
                    let inner = self.hole.horizon * 1.35;
                    if r > inner && r < self.hole.disk_radius {
                        let radial = (r - inner) / (self.hole.disk_radius - inner);
                        let angle = point.dot(&disk_up).atan2(point.dot(&disk_axis));
                        let flow = angle * 13.0
                            + radial * 140.0
                            + (angle * 5.0 + radial * 28.0).sin() * 4.0
                            - flight.seconds * 1.8;
                        let filaments = 0.55
                            + 0.18 * flow.sin()
                            + 0.14 * (radial * 390.0 + angle * 7.0 + flow.sin() * 2.0).sin()
                            + 0.13 * (angle * 31.0 + radial * 52.0).sin();
                        let edge = ((1.0 - radial) * 6.0).clamp(0.0, 1.0)
                            * (radial * 30.0).clamp(0.0, 1.0);
                        let hot = (1.0 - radial).powf(1.6);
                        let brightness = edge * filaments * (0.40 + hot * 0.60);
                        let r = (255.0 * brightness * 2.0).min(255.0).round() as u32;
                        let g = ((35.0 + hot * 220.0) * brightness * 1.7).min(255.0).round() as u32;
                        let b = ((4.0 + hot.powi(4) * 230.0) * brightness * 1.6)
                            .min(255.0)
                            .round() as u32;
                        let color = r << 16 | g << 8 | b;
                        let z = 1.0 / (t * cosine);
                        if z > *depth_buffer {
                            *pixel = color;
                            *depth_buffer = z;
                        }
                    }
                }
            }
            // El anillo luminoso sigue la silueta desde cualquier angulo.
            let along = relative.dot(&direction);
            if along > 0.25 {
                let impact = (relative - direction * along).norm() / self.hole.horizon;
                if (1.0..1.55).contains(&impact) {
                    let ring = (-((impact - 1.075) / 0.024).powi(2)).exp();
                    let halo = (-((impact - 1.11) / 0.22).powi(2)).exp() * 0.24;
                    if 1.0 / (along * cosine) > *depth_buffer {
                        let strength = (ring + halo).min(1.0);
                        *pixel =
                            crate::framebuffer::emission_color(*pixel, 0xffba69, strength, 0.0);
                    }
                }
            }
        });
    }

    pub fn preview(&self, fb: &mut Framebuffer, id: &str) -> Result<(), Box<dyn Error>> {
        if id != "accretion" {
            return Err(format!("Objeto desconocido: {id}. Opción: accretion").into());
        }
        let mut flight = Flight::new();
        flight.position = self.hole.position + Vec3::new(0.0, 30.0, 430.0);
        flight.camera.pitch = 0.0;
        fb.clear();
        fb.pixels.fill(0x03060c);
        self.draw(fb, &flight);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_fragments_match_single_thread_at_the_horizon() {
        let scene = DeepSpace::new();
        let mut flight = Flight::new();
        flight.position = BLACK_HOLE.position + Vec3::new(0.0, 0.0, 250.0);
        let render = |threads| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut frame = Framebuffer::new(320, 210);
            frame.point(160, 105, 2.0, 0xff00ff, true);
            pool.install(|| scene.draw(&mut frame, &flight));
            frame
        };
        let sequential = render(1);
        let parallel = render(4);
        assert_eq!(sequential.pixels, parallel.pixels);
        assert_eq!(sequential.depth, parallel.depth);
        assert_eq!(parallel.pixels[105 * 320 + 160], 0xff00ff);
    }

    #[test]
    fn horizon_is_a_volume_and_all_orbit_views_remain_finite() {
        let hole = DeepSpace::new();
        assert_eq!(
            sphere_hit(Vec3::new(0.0, 0.0, 100.0), -Vec3::z(), Vec3::zeros(), 44.0),
            Some(56.0)
        );
        assert_eq!(
            sphere_hit(Vec3::zeros(), Vec3::z(), Vec3::zeros(), 44.0),
            Some(44.0)
        );
        for angle in [0.0_f32, 1.0, 2.5, 4.5] {
            let mut flight = Flight::new();
            let offset = Vec3::new(angle.sin(), 0.2, angle.cos()) * 250.0;
            flight.position = BLACK_HOLE.position + offset;
            let direction = -offset.normalize();
            flight.camera.yaw = (-direction.z).atan2(-direction.x);
            flight.camera.pitch = (-direction.y).asin();
            let mut fb = Framebuffer::new(320, 240);
            hole.draw(&mut fb, &flight);
            assert!(fb.pixels.contains(&0x010102));
            assert!(fb.pixels.iter().any(|c| (c >> 16) > 80));
        }
    }

    #[test]
    fn foreground_geometry_hides_the_horizon_and_disk() {
        let mut fb = Framebuffer::new(320, 240);
        for y in 0..fb.height {
            for x in 0..fb.width {
                fb.point(x as i32, y as i32, 1.0, 0x123456, true);
            }
        }
        let mut flight = Flight::new();
        flight.position = BLACK_HOLE.position + Vec3::new(0.0, 0.0, 200.0);
        DeepSpace::new().draw(&mut fb, &flight);
        assert!(fb.pixels.iter().all(|c| *c == 0x123456));
    }
}
