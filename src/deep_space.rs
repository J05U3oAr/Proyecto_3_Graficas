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

const IDS: [&str; 5] = ["helix", "southern-ring", "binary", "accretion", "spiral"];
static TEXTURES: OnceLock<Result<Arc<Vec<Texture>>, String>> = OnceLock::new();

struct Landmark {
    position: Vec3,
    width: f32,
}

pub struct DeepSpace {
    textures: Arc<Vec<Texture>>,
    objects: [Landmark; 5],
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
fn shadow(index: usize, u: f32, v: f32) -> f32 {
    let ellipse = |x: f32, y: f32, rx: f32, ry: f32| {
        let radius = (((u - x) / rx).powi(2) + ((v - y) / ry).powi(2)).sqrt();
        smooth((1.03 - radius) / 0.06)
    };
    match index {
        2 => ellipse(0.536, 0.440, 0.047, 0.083).max(ellipse(0.455, 0.574, 0.038, 0.067)),
        3 => ellipse(0.670, 0.530, 0.153, 0.237).max(ellipse(0.118, 0.668, 0.017, 0.030)),
        _ => 0.0,
    }
}

impl DeepSpace {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let textures = TEXTURES
            .get_or_init(|| {
                IDS.iter()
                    .map(|id| {
                        Texture::load(format!("assets/deep-space/textures/{id}.png"))
                            .map_err(|e| format!("Escenario {id}: {e}"))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(Arc::new)
            })
            .as_ref()
            .map(Arc::clone)
            .map_err(Clone::clone)?;
        Ok(Self {
            textures,
            objects: [
                Landmark {
                    position: Vec3::new(-9650.0, 920.0, -13250.0),
                    width: 6500.0,
                },
                Landmark {
                    position: Vec3::new(11150.0, -3670.0, -15630.0),
                    width: 7000.0,
                },
                Landmark {
                    position: Vec3::new(3545.0, -9664.0, -16808.0),
                    width: 9000.0,
                },
                Landmark {
                    position: Vec3::new(12420.0, 2610.0, -20560.0),
                    width: 11200.0,
                },
                Landmark {
                    position: Vec3::new(-12330.0, -9670.0, -20850.0),
                    width: 11000.0,
                },
            ],
        })
    }

    pub fn draw(&self, fb: &mut Framebuffer, flight: &Flight) {
        let view = FlightView::new(flight, fb.width, fb.height);
        let mut order = [0, 1, 2, 3, 4];
        order.sort_by(|a, b| {
            view.view_point(self.objects[*a].position)
                .z
                .total_cmp(&view.view_point(self.objects[*b].position).z)
        });
        for index in order {
            self.draw_object(fb, &view, index);
        }
    }

    fn draw_object(&self, fb: &mut Framebuffer, view: &FlightView, index: usize) {
        let object = &self.objects[index];
        let Some(center) = view.project(object.position) else {
            return;
        };
        let width = object.width * FLIGHT_FOCAL * center.z;
        let height = width / self.textures[index].aspect_ratio();
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
                let color = self.textures[index].sample(Vec2::new(u, 1.0 - v)).to_hex();
                fb.emission_depth((x, y), center.z, color, fade, shadow(index, u, v) * fade);
            }
        }
    }

    /// A close view uses the same world projection and compositor as gameplay.
    pub fn preview(&self, fb: &mut Framebuffer, id: &str) -> Result<(), Box<dyn Error>> {
        let index = IDS
            .iter()
            .position(|candidate| *candidate == id)
            .ok_or_else(|| format!("Escenario desconocido: {id}. Opciones: {}", IDS.join(", ")))?;
        let object = &self.objects[index];
        let distance = (object.width * FLIGHT_FOCAL / (fb.width as f32 * 0.90)).max(
            object.width / self.textures[index].aspect_ratio() * FLIGHT_FOCAL
                / (fb.height as f32 * 0.90),
        );
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        flight.position = object.position + Vec3::new(0.0, -1.6, distance - 8.0);
        fb.clear();
        fb.pixels.fill(0x010207);
        self.draw_object(fb, &FlightView::new(&flight, fb.width, fb.height), index);
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
        assert_eq!(shadow(2, 0.536, 0.440), 1.0);
        assert_eq!(shadow(2, 0.455, 0.574), 1.0);
        assert_eq!(shadow(3, 0.670, 0.530), 1.0);
        assert_eq!(shadow(0, 0.5, 0.5), 0.0);
    }

    #[test]
    fn landmarks_are_distant_world_objects_visible_at_launch_and_behind_foreground() {
        let space = DeepSpace::load().unwrap();
        let flight = Flight::new();
        let view = FlightView::new(&flight, 1280, 840);
        for object in &space.objects {
            assert!(object.position.norm() > 15000.0);
            let center = view.project(object.position).unwrap();
            assert!((0.0..1280.0).contains(&center.x));
            assert!((0.0..840.0).contains(&center.y));
            assert!(center.z < 0.0001);
            let mut moved = Flight::new();
            moved.position.x = 3000.0;
            let shifted = FlightView::new(&moved, 1280, 840).project(object.position).unwrap();
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
