use crate::{
    framebuffer::Framebuffer,
    renderer::{FLIGHT_FOCAL, FlightView, WorldTransform},
};
use nalgebra_glm::Vec3;
use rayon::prelude::*;
use std::f32::consts::TAU;

struct Grain {
    height: f32,
    angle: f32,
    altitude: f32,
    phase: f32,
    size: f32,
    opacity: f32,
    fine: bool,
}

pub struct DustStorm {
    grains: Vec<Grain>,
    axis: Vec3,
    right: Vec3,
    up: Vec3,
}

struct ProjectedDust {
    screen: Vec3,
    radius: f32,
    color: u32,
    opacity: f32,
}

impl DustStorm {
    pub fn new() -> Self {
        let count = 4800;
        let random = |seed: u32| {
            let mut n = seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
            n = ((n >> ((n >> 28) + 4)) ^ n).wrapping_mul(277_803_737);
            ((n >> 22) ^ n) as f32 / u32::MAX as f32
        };
        let grains = (0..count)
            .map(|index| {
                let fine = index % 6 == 0;
                let variation = random(index * 5 + 17);
                Grain {
                    height: 1.0 - 2.0 * (index as f32 + 0.5) / count as f32,
                    angle: index as f32 * 2.399_963_1,
                    altitude: random(index * 5 + 18),
                    phase: random(index * 5 + 19) * TAU,
                    size: if fine {
                        0.007 + variation * 0.006
                    } else {
                        0.055 + variation * 0.05
                    },
                    opacity: if fine {
                        0.28 + variation * 0.22
                    } else {
                        0.18 + variation * 0.18
                    },
                    fine,
                }
            })
            .collect();
        // Match the giant cyclone's axis in the Blender surface (exported with Y up).
        let axis = Vec3::new(0.16, 0.24, 1.0).normalize();
        let right = axis.cross(&Vec3::new(0.0, 1.0, 0.0)).normalize();
        let up = right.cross(&axis).normalize();
        Self {
            grains,
            axis,
            right,
            up,
        }
    }

    pub fn draw(
        &self,
        fb: &mut Framebuffer,
        view: &FlightView,
        planet: WorldTransform,
        seconds: f32,
    ) {
        if !view.sphere_visible(planet.position, planet.radius * 1.25) {
            return;
        }
        let (spin_sin, spin_cos) = planet.yaw.sin_cos();
        let light = Vec3::new(-0.4, 0.8, 1.0).normalize();
        let mut projected = Vec::with_capacity(self.grains.len());
        for grain in &self.grains {
            let height = (grain.height + (seconds * 0.28 + grain.phase).sin() * 0.012)
                .clamp(-0.9999, 0.9999);
            let latitude_radius = (1.0 - height * height).sqrt();
            let wind = 0.18 + 0.24 * latitude_radius;
            let angle = grain.angle + seconds * wind + (height * 9.0 + seconds * 0.23).sin() * 0.12;
            let (sin, cos) = angle.sin_cos();
            let direction =
                self.axis * height + (self.right * cos + self.up * sin) * latitude_radius;
            let altitude =
                1.02 + grain.altitude * 0.07 + (seconds * 0.31 + grain.phase).sin() * 0.008;
            let local = direction * (planet.radius * altitude);
            let world = planet.position
                + Vec3::new(
                    local.x * spin_cos + local.z * spin_sin,
                    local.y,
                    local.z * spin_cos - local.x * spin_sin,
                );
            let Some(screen) = view.project(world) else {
                continue;
            };
            let radius = (grain.size * planet.radius * FLIGHT_FOCAL * screen.z).min(48.0);
            if radius < 0.65
                || screen.x + radius < 0.0
                || screen.y + radius < 0.0
                || screen.x - radius >= fb.width as f32
                || screen.y - radius >= fb.height as f32
            {
                continue;
            }
            let color = if grain.fine {
                0xf4c27b
            } else if grain.altitude > 0.5 {
                0xd7ad74
            } else {
                0xc4955e
            };
            let normal = (world - planet.position) / (planet.radius * altitude);
            let lighting = 0.48 + view.view_direction(normal).dot(&light).max(0.0) * 0.52;
            let shade = |shift: u32| ((((color >> shift) & 255) as f32 * lighting).round()) as u32;
            let color = shade(16) << 16 | shade(8) << 8 | shade(0);
            let flow = if grain.fine {
                1.0
            } else {
                0.72 + (angle * 3.0 - height * 22.0).sin() * 0.28
            };
            projected.push(ProjectedDust {
                screen,
                radius,
                color,
                opacity: grain.opacity * flow,
            });
        }
        // Reciprocal depth increases toward the eye: blend the farthest billows first.
        projected.sort_unstable_by(|a, b| a.screen.z.total_cmp(&b.screen.z));
        draw_cloud(fb, &projected);
    }
}

fn draw_cloud(fb: &mut Framebuffer, particles: &[ProjectedDust]) {
    let width = fb.width;
    let (left, top, right, bottom) = fb.clip;
    let band_height = 32;
    let mut bins = vec![Vec::new(); fb.height.div_ceil(band_height)];
    for (index, dust) in particles.iter().enumerate() {
        let y0 = (dust.screen.y - dust.radius).floor().max(top as f32) as usize;
        let y1 = ((dust.screen.y + dust.radius).ceil() + 1.0)
            .max(0.0)
            .min(bottom as f32) as usize;
        if y0 < y1 {
            for bin in &mut bins[y0 / band_height..=(y1 - 1) / band_height] {
                bin.push(index);
            }
        }
    }
    // El orden de mezcla se mantiene de atras hacia delante dentro de cada franja.
    let paint = |(band, (pixels, depth)): (usize, (&mut [u32], &[f32]))| {
        let start_y = band * band_height;
        let end_y = start_y + pixels.len() / width;
        for &index in &bins[band] {
            let dust = &particles[index];
            let x0 = (dust.screen.x - dust.radius).floor().max(left as f32) as usize;
            let x1 = ((dust.screen.x + dust.radius).ceil() + 1.0)
                .max(0.0)
                .min(right as f32) as usize;
            let y0 = (dust.screen.y - dust.radius)
                .floor()
                .max(top.max(start_y) as f32) as usize;
            let y1 = ((dust.screen.y + dust.radius).ceil() + 1.0)
                .max(0.0)
                .min(bottom.min(end_y) as f32) as usize;
            let inverse_radius_squared = 1.0 / (dust.radius * dust.radius);
            for y in y0..y1 {
                for x in x0..x1 {
                    let offset = (y - start_y) * width + x;
                    if dust.screen.z <= depth[offset] {
                        continue;
                    }
                    let dx = x as f32 + 0.5 - dust.screen.x;
                    let dy = y as f32 + 0.5 - dust.screen.y;
                    let distance = (dx * dx + dy * dy) * inverse_radius_squared;
                    if distance < 1.0 {
                        let alpha = (dust.opacity * (1.0 - distance).powi(2)).clamp(0.0, 1.0);
                        let background = pixels[offset];
                        let channel = |shift: u32| {
                            let from = ((background >> shift) & 255) as f32;
                            let to = ((dust.color >> shift) & 255) as f32;
                            (from + (to - from) * alpha).round() as u32
                        };
                        pixels[offset] = channel(16) << 16 | channel(8) << 8 | channel(0);
                    }
                }
            }
        }
    };
    let chunk = width * band_height;
    if particles.len() >= 128 && rayon::current_num_threads() > 1 {
        fb.pixels
            .par_chunks_mut(chunk)
            .zip(fb.depth.par_chunks(chunk))
            .enumerate()
            .for_each(paint);
    } else {
        fb.pixels
            .chunks_mut(chunk)
            .zip(fb.depth.chunks(chunk))
            .enumerate()
            .for_each(paint);
    }
}

#[cfg(test)]
fn draw_billows(fb: &mut Framebuffer, dust: ProjectedDust) {
    let left = (dust.screen.x - dust.radius).floor().max(0.0) as usize;
    let right = (dust.screen.x + dust.radius)
        .ceil()
        .min(fb.width as f32 - 1.0) as usize;
    let top = (dust.screen.y - dust.radius).floor().max(0.0) as usize;
    let bottom = (dust.screen.y + dust.radius)
        .ceil()
        .min(fb.height as f32 - 1.0) as usize;
    let inverse_radius_squared = 1.0 / (dust.radius * dust.radius);
    for y in top..=bottom {
        for x in left..=right {
            let dx = x as f32 + 0.5 - dust.screen.x;
            let dy = y as f32 + 0.5 - dust.screen.y;
            let distance = (dx * dx + dy * dy) * inverse_radius_squared;
            if distance < 1.0 {
                let alpha = dust.opacity * (1.0 - distance).powi(2);
                fb.blend_depth(x, y, dust.screen.z, dust.color, alpha);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{flight::Flight, space::Space};

    #[test]
    fn parallel_cloud_matches_ordered_transparency_and_opaque_depth() {
        let particles: Vec<_> = (0..160)
            .map(|i| ProjectedDust {
                screen: Vec3::new(
                    8.0 + (i % 9) as f32 * 8.0,
                    5.0 + (i % 11) as f32 * 7.0,
                    0.1 + i as f32 * 0.001,
                ),
                radius: 12.0,
                color: 0xe6ad72,
                opacity: 0.2,
            })
            .collect();
        let mut reference = Framebuffer::new(96, 96);
        reference.set_clip(6, 3, 78, 83);
        reference.point(30, 30, 2.0, 0xff00ff, true);
        for dust in &particles {
            draw_billows(
                &mut reference,
                ProjectedDust {
                    screen: dust.screen,
                    radius: dust.radius,
                    color: dust.color,
                    opacity: dust.opacity,
                },
            );
        }
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut frame = Framebuffer::new(96, 96);
            frame.set_clip(6, 3, 78, 83);
            frame.point(30, 30, 2.0, 0xff00ff, true);
            pool.install(|| draw_cloud(&mut frame, &particles));
            assert_eq!(frame.pixels, reference.pixels);
            assert_eq!(frame.depth, reference.depth);
        }
    }

    #[test]
    fn storm_moves_while_the_ship_is_stationary_and_repeats_deterministically() {
        let space = Space::load().unwrap();
        let flight = Flight::new();
        let render = |seconds| {
            let mut fb = Framebuffer::new(1280, 840);
            space.draw(&mut fb, &flight, 0.0);
            let view = FlightView::new(&flight, fb.width, fb.height);
            let planet = &space.planets[0];
            planet
                .dust
                .as_ref()
                .unwrap()
                .draw(&mut fb, &view, planet.transform, seconds);
            fb
        };
        let before = render(0.0);
        let repeated = render(0.0);
        let after = render(1.0);
        assert_eq!(before.pixels, repeated.pixels);
        let changed = before
            .pixels
            .iter()
            .zip(&after.pixels)
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changed > 10000,
            "The global cloud must visibly flow: {changed}"
        );
    }

    #[test]
    fn foreground_geometry_occludes_the_entire_cloud() {
        let space = Space::load().unwrap();
        let flight = Flight::new();
        let mut fb = Framebuffer::new(640, 420);
        for y in 0..fb.height {
            for x in 0..fb.width {
                fb.point(x as i32, y as i32, 1.0, 0x123456, true);
            }
        }
        let view = FlightView::new(&flight, fb.width, fb.height);
        let planet = &space.planets[0];
        planet
            .dust
            .as_ref()
            .unwrap()
            .draw(&mut fb, &view, planet.transform, 2.0);
        assert!(fb.pixels.iter().all(|&p| p == 0x123456));
    }
}
