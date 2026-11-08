//! Perspective-correct portal disks using the supplied yellow art and green GIF atlas.
use crate::{
    flight::Flight,
    framebuffer::Framebuffer,
    portal::{Kind, RADIUS},
    renderer::FlightView,
};
use image::RgbaImage;
use nalgebra_glm::Vec2;
use std::{error::Error, path::PathBuf, sync::OnceLock};

pub struct Art {
    yellow: RgbaImage,
    green: RgbaImage,
}

struct Frame<'a> {
    image: &'a RgbaImage,
    x: u32,
    y: u32,
}

impl Frame<'_> {
    fn sample(&self, point: Vec2) -> (u32, f32) {
        if point.x.abs() > 1.0 || point.y.abs() > 1.0 {
            return (0, 0.0);
        }
        let x = (point.x + 1.0) * 0.5 * 255.0;
        let y = (1.0 - point.y) * 0.5 * 255.0;
        let (left, top) = (x.floor() as u32, y.floor() as u32);
        let (right, bottom) = ((left + 1).min(255), (top + 1).min(255));
        let (u, v) = (x - left as f32, y - top as f32);
        let mut result = [0.0f32; 4];
        // Premultiplied interpolation prevents black/white fringes at the cutout.
        for (x, y, weight) in [
            (left, top, (1.0 - u) * (1.0 - v)),
            (right, top, u * (1.0 - v)),
            (left, bottom, (1.0 - u) * v),
            (right, bottom, u * v),
        ] {
            let pixel = self.image.get_pixel(self.x + x, self.y + y);
            let alpha = pixel[3] as f32 / 255.0;
            for channel in 0..3 {
                result[channel] += pixel[channel] as f32 * alpha * weight;
            }
            result[3] += alpha * weight;
        }
        if result[3] < 1e-6 {
            return (0, 0.0);
        }
        let channel = |i: usize| (result[i] / result[3]).round().clamp(0.0, 255.0) as u32;
        (channel(0) << 16 | channel(1) << 8 | channel(2), result[3])
    }
}

impl Art {
    pub fn load() -> Result<&'static Self, Box<dyn Error>> {
        static ART: OnceLock<Result<Art, String>> = OnceLock::new();
        let result = ART.get_or_init(|| {
            let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/portals");
            let read = |name| image::open(directory.join(name)).map(|image| image.to_rgba8());
            let yellow = read("yellow.png").map_err(|e| e.to_string())?;
            let green = read("green-atlas.png").map_err(|e| e.to_string())?;
            if yellow.dimensions() != (256, 256) || green.dimensions() != (2048, 1792) {
                return Err("Dimensiones invalidas en las texturas de los portales".into());
            }
            Ok(Art { yellow, green })
        });
        match result {
            Ok(art) => Ok(art),
            Err(error) => Err(error.clone().into()),
        }
    }

    fn frame(&self, kind: Kind, seconds: f32) -> Frame<'_> {
        let index = ((seconds.rem_euclid(2.8) / 0.05).floor() as u32).min(55);
        match kind {
            Kind::Yellow => Frame {
                image: &self.yellow,
                x: 0,
                y: 0,
            },
            Kind::Green => Frame {
                image: &self.green,
                x: (index % 8) * 256,
                y: (index / 8) * 256,
            },
        }
    }

    pub fn draw(&self, fb: &mut Framebuffer, flight: &Flight) {
        let camera = FlightView::new(flight, fb.width, fb.height);
        let eye = flight.camera_position();
        let (right, up, forward) = flight.camera.basis();
        let mut portals: Vec<_> = Kind::ALL
            .into_iter()
            .filter_map(|kind| flight.portals.get(kind).map(|portal| (kind, portal)))
            .collect();
        portals.sort_by(|(_, a), (_, b)| {
            (b.center - eye)
                .norm_squared()
                .total_cmp(&(a.center - eye).norm_squared())
        });
        for (kind, portal) in portals {
            let reach = RADIUS * 1.22;
            if !camera.sphere_visible(portal.center, reach) {
                continue;
            }
            let to_center = portal.center - eye;
            let plane_distance = to_center.dot(&portal.normal);
            if plane_distance.abs() < 0.02 {
                continue;
            }
            let mut bounds = (fb.width, fb.height, 0, 0);
            let mut clipped = false;
            for (x, y) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                if let Some(p) =
                    camera.project(portal.center + (portal.right * x + portal.up * y) * reach)
                {
                    bounds.0 = bounds.0.min((p.x - 2.0).floor().max(0.0) as usize);
                    bounds.1 = bounds.1.min((p.y - 2.0).floor().max(0.0) as usize);
                    bounds.2 = bounds
                        .2
                        .max((p.x + 3.0).ceil().max(0.0).min(fb.width as f32) as usize);
                    bounds.3 = bounds
                        .3
                        .max((p.y + 3.0).ceil().max(0.0).min(fb.height as f32) as usize);
                } else {
                    clipped = true;
                }
            }
            if clipped {
                bounds = (0, 0, fb.width, fb.height);
            }
            let frame = self.frame(kind, flight.seconds);
            let angle = if kind == Kind::Yellow {
                flight.seconds * 0.32
            } else {
                0.0
            };
            let (sin, cos) = angle.sin_cos();
            let width = fb.width as f32;
            let height = fb.height as f32;
            fb.shade_rect(bounds, |x, y, pixel, depth| {
                let ray = forward
                    + right * ((x as f32 + 0.5 - width * 0.5) / crate::renderer::FLIGHT_FOCAL)
                    + up * ((height * 0.5 - y as f32 - 0.5) / crate::renderer::FLIGHT_FOCAL);
                let denominator = ray.dot(&portal.normal);
                if denominator.abs() < 1e-6 {
                    return;
                }
                let distance = plane_distance / denominator;
                if distance < 0.25 || 1.0 / distance <= *depth {
                    return;
                }
                let offset = ray * distance - to_center;
                let local = Vec2::new(offset.dot(&portal.right), offset.dot(&portal.up)) / RADIUS;
                let radius = local.norm();
                if radius > 1.22 {
                    return;
                }
                let glow = (-((radius - 0.94) / 0.13).powi(2)).exp() * 0.32;
                *pixel = crate::framebuffer::emission_color(*pixel, kind.color(), glow, 0.0);
                let uv = Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos);
                let (color, alpha) = frame.sample(uv);
                if alpha > 0.0 {
                    *pixel = crate::ui::mix(*pixel, color, alpha);
                    if alpha > 0.98 {
                        *depth = 1.0 / distance;
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::Portal;
    use nalgebra_glm::Vec3;

    #[test]
    fn reference_atlas_keeps_transparent_corners_and_animated_green_frames() {
        let art = Art::load().unwrap();
        assert_eq!(
            art.frame(Kind::Green, 0.0).sample(Vec2::new(-1.0, 1.0)).1,
            0.0
        );
        assert_eq!(
            art.frame(Kind::Yellow, 0.0).sample(Vec2::new(-1.0, 1.0)).1,
            0.0
        );
        assert!(art.frame(Kind::Green, 0.0).sample(Vec2::zeros()).1 > 0.98);
        let changed = (0..20)
            .flat_map(|y| {
                (0..20).map(move |x| Vec2::new(x as f32 / 10.0 - 1.0, y as f32 / 10.0 - 1.0))
            })
            .filter(|&p| {
                art.frame(Kind::Green, 0.0).sample(p) != art.frame(Kind::Green, 0.35).sample(p)
            })
            .count();
        assert!(changed > 100);
    }

    #[test]
    fn world_portals_animate_respect_foreground_and_render_from_either_side() {
        let art = Art::load().unwrap();
        let mut flight = Flight::new();
        flight.portals.set_enabled(true);
        flight.portals.place(
            Kind::Green,
            Portal::new(Vec3::new(0.0, 0.0, -22.0), Vec3::z()).unwrap(),
        );
        let mut first = Framebuffer::new(320, 240);
        art.draw(&mut first, &flight);
        assert!(
            first
                .pixels
                .iter()
                .filter(|&&p| p != crate::framebuffer::BACKGROUND)
                .count()
                > 1000
        );
        flight.seconds = 0.35;
        let mut animated = Framebuffer::new(320, 240);
        art.draw(&mut animated, &flight);
        assert_ne!(first.pixels, animated.pixels);
        animated.pixels.fill(0x123456);
        animated.depth.fill(f32::INFINITY);
        art.draw(&mut animated, &flight);
        assert!(animated.pixels.iter().all(|&p| p == 0x123456));
        flight.position.z = -44.0;
        flight.camera.yaw += std::f32::consts::PI;
        let mut back = Framebuffer::new(320, 240);
        art.draw(&mut back, &flight);
        assert!(
            back.pixels
                .iter()
                .any(|&p| p != crate::framebuffer::BACKGROUND)
        );
    }
}
