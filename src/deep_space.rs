//! High-resolution distant scenery, in world coordinates rather than screen overlays.
//! Plates preserve photographic filaments; lensing is baked into the source art.
use crate::{
    flight::Flight,
    framebuffer::Framebuffer,
    renderer::{FLIGHT_FOCAL, FlightView},
    texture::Texture,
};
use nalgebra_glm::{Vec2, Vec3};
use std::{
    error::Error,
    sync::{Arc, OnceLock},
};

static TEXTURE: OnceLock<Result<Arc<Texture>, String>> = OnceLock::new();

struct Landmark {
    position: Vec3,
    width: f32,
}

pub struct DeepSpace {
    texture: Arc<Texture>,
    object: Landmark,
}

fn smooth(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn edge_fade(u: f32, v: f32) -> f32 {
    let edge = u.min(1.0 - u).min(v.min(1.0 - v));
    let radius = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() * 2.0;
    smooth(edge / 0.12) * smooth((1.10 - radius) / 0.32)
}

/// Coordinates use image rows, with y increasing downward.
fn shadow(u: f32, v: f32) -> f32 {
    let ellipse = |x: f32, y: f32, rx: f32, ry: f32| {
        let radius = (((u - x) / rx).powi(2) + ((v - y) / ry).powi(2)).sqrt();
        smooth((1.03 - radius) / 0.06)
    };
    ellipse(0.670, 0.530, 0.153, 0.237).max(ellipse(0.118, 0.668, 0.017, 0.030))
}

impl DeepSpace {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let texture = TEXTURE
            .get_or_init(|| {
                Texture::load("assets/deep-space/textures/accretion.png")
                    .map_err(|e| format!("Agujero negro: {e}"))
                    .map(Arc::new)
            })
            .as_ref()
            .map(Arc::clone)
            .map_err(Clone::clone)?;
        Ok(Self {
            texture,
            object: Landmark {
                position: Vec3::new(12420.0, 2610.0, -20560.0),
                width: 11200.0,
            },
        })
    }

    pub fn draw(&self, fb: &mut Framebuffer, flight: &Flight) {
        let view = FlightView::new(flight, fb.width, fb.height);
        self.draw_object(fb, &view);
    }

    fn draw_object(&self, fb: &mut Framebuffer, view: &FlightView) {
        let object = &self.object;
        let Some(center) = view.project(object.position) else {
            return;
        };
        let width = object.width * FLIGHT_FOCAL * center.z;
        let height = width / self.texture.aspect_ratio();
        let (left, top) = (center.x - width * 0.5, center.y - height * 0.5);
        let x0 = left.max(0.0).min(fb.width as f32) as usize;
        let y0 = top.max(0.0).min(fb.height as f32) as usize;
        let x1 = (left + width).ceil().max(0.0).min(fb.width as f32) as usize;
        let y1 = (top + height).ceil().max(0.0).min(fb.height as f32) as usize;
        for y in y0..y1 {
            let v = (y as f32 + 0.5 - top) / height;
            for x in x0..x1 {
                let u = (x as f32 + 0.5 - left) / width;
                let fade = edge_fade(u, v);
                if fade <= 0.0 {
                    continue;
                }
                let color = self.texture.sample(Vec2::new(u, 1.0 - v)).to_hex();
                fb.emission_depth((x, y), center.z, color, fade, shadow(u, v) * fade);
            }
        }
    }

    /// A close view uses the same world projection and compositor as gameplay.
    pub fn preview(&self, fb: &mut Framebuffer, id: &str) -> Result<(), Box<dyn Error>> {
        if id != "accretion" {
            return Err(format!("Escenario desconocido: {id}. Opcion: accretion").into());
        }
        let object = &self.object;
        let distance = (object.width * FLIGHT_FOCAL / (fb.width as f32 * 0.90)).max(
            object.width / self.texture.aspect_ratio() * FLIGHT_FOCAL / (fb.height as f32 * 0.90),
        );
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        flight.position = object.position + Vec3::new(0.0, -1.6, distance - 8.0);
        fb.clear();
        fb.pixels.fill(0x010207);
        self.draw_object(fb, &FlightView::new(&flight, fb.width, fb.height));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photographic_boundaries_fade_and_black_hole_centers_stay_opaque() {
        for (u, v) in [(0.0, 0.5), (1.0, 0.5), (0.5, 0.0), (0.5, 1.0)] {
            assert_eq!(edge_fade(u, v), 0.0);
        }
        assert_eq!(edge_fade(0.5, 0.5), 1.0);
        assert_eq!(shadow(0.670, 0.530), 1.0);
        assert_eq!(shadow(0.118, 0.668), 1.0);
        assert_eq!(shadow(0.5, 0.1), 0.0);
    }

    #[test]
    fn landmarks_are_distant_world_objects_visible_at_launch_and_behind_foreground() {
        let space = DeepSpace::load().unwrap();
        let flight = Flight::new();
        let view = FlightView::new(&flight, 1280, 840);
        {
            let object = &space.object;
            assert!(object.position.norm() > 15000.0);
            let center = view.project(object.position).unwrap();
            assert!((0.0..1280.0).contains(&center.x));
            assert!((0.0..840.0).contains(&center.y));
            assert!(center.z < 0.0001);
            let mut moved = Flight::new();
            moved.position.x = 3000.0;
            let shifted = FlightView::new(&moved, 1280, 840)
                .project(object.position)
                .unwrap();
            assert!((shifted.x - center.x).abs() > 50.0);
        }
        let mut fb = Framebuffer::new(1280, 840);
        for y in 0..fb.height {
            for x in 0..fb.width {
                fb.point(x as i32, y as i32, 0.5, 0x123456, true);
            }
        }
        space.draw(&mut fb, &flight);
        assert!(fb.pixels.iter().all(|pixel| *pixel == 0x123456));
        let mut away = Flight::new();
        away.camera.yaw += std::f32::consts::PI;
        fb.clear();
        let baseline = fb.pixels.clone();
        space.draw(&mut fb, &away);
        assert_eq!(fb.pixels, baseline);
    }
}
