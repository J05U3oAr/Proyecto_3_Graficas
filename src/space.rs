use crate::{
    dust::DustStorm,
    flight::Flight,
    framebuffer::Framebuffer,
    model::Model,
    renderer::{self, FlightView, WorldTransform},
};
use nalgebra_glm::Vec3;
use std::{error::Error, path::PathBuf};

pub struct Planet {
    pub name: &'static str,
    pub model: Model,
    pub transform: WorldTransform,
    pub rotation_speed: f32,
    pub dust: Option<DustStorm>,
}

pub struct Space {
    pub planets: Vec<Planet>,
}

impl Space {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/planets");
        Ok(Self {
            planets: vec![
                Planet {
                    name: "Siroco",
                    model: Model::load(assets.join("siroco.obj"))?,
                    transform: WorldTransform {
                        position: Vec3::new(-36.0, 7.0, -105.0),
                        radius: 22.0,
                        yaw: 0.15,
                    },
                    rotation_speed: 0.018,
                    dust: Some(DustStorm::new()),
                },
                Planet {
                    name: "Oquedad",
                    model: Model::load(assets.join("oquedad.obj"))?,
                    transform: WorldTransform {
                        position: Vec3::new(42.0, 3.0, -135.0),
                        radius: 25.0,
                        yaw: -0.45,
                    },
                    rotation_speed: 0.035,
                    dust: None,
                },
                Planet {
                    name: "Jovara",
                    model: Model::load(assets.join("jovara.obj"))?,
                    transform: WorldTransform {
                        position: Vec3::new(0.0, 57.0, -215.0),
                        radius: 35.0,
                        yaw: 0.0,
                    },
                    rotation_speed: 0.006,
                    dust: None,
                },
            ],
        })
    }

    pub fn draw(&self, fb: &mut Framebuffer, flight: &Flight, seconds: f32) {
        let view = FlightView::new(flight, fb.width, fb.height);
        for planet in &self.planets {
            let mut transform = planet.transform;
            transform.yaw += seconds * planet.rotation_speed;
            renderer::draw_world_model(fb, &planet.model, &view, transform);
        }
    }

    pub fn draw_atmospheres(&self, fb: &mut Framebuffer, flight: &Flight, seconds: f32) {
        let view = FlightView::new(flight, fb.width, fb.height);
        for planet in &self.planets {
            if let Some(dust) = &planet.dust {
                let mut transform = planet.transform;
                transform.yaw += seconds * planet.rotation_speed;
                dust.draw(fb, &view, transform, seconds);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framebuffer::BACKGROUND;

    #[test]
    fn jovara_is_visible_above_the_launch_route_with_its_blue_polar_texture() {
        let space = Space::load().unwrap();
        let planet = space.planets.iter().find(|p| p.name == "Jovara").unwrap();
        assert_eq!(planet.model.textures.len(), 1);
        let flight = Flight::new();
        let mut frame = Framebuffer::new(1280, 840);
        space.draw(&mut frame, &flight, 0.0);
        let view = FlightView::new(&flight, frame.width, frame.height);
        let point = view.project(planet.transform.position).unwrap();
        assert!((50.0..250.0).contains(&point.y));
        let color = frame.pixels[point.y as usize * frame.width + point.x as usize];
        let (r, g, b) = ((color >> 16) & 255, (color >> 8) & 255, color & 255);
        assert!(
            g > r && b > r,
            "The polar center must retain the blue/teal reference colors"
        );
    }

    #[test]
    fn both_planets_are_visible_at_launch_and_move_with_the_camera() {
        let space = Space::load().unwrap();
        let mut flight = Flight::new();
        let mut before = Framebuffer::new(1280, 840);
        let mut after = Framebuffer::new(1280, 840);
        space.draw(&mut before, &flight, 0.0);
        for (left, right) in [(260, 580), (700, 1000)] {
            let visible = (150..480)
                .flat_map(|y| (left..right).map(move |x| y * 1280 + x))
                .filter(|&index| before.pixels[index] != BACKGROUND)
                .count();
            assert!(
                visible > 10000,
                "A planet is missing from the initial view: {visible}"
            );
        }
        flight.position.x += 15.0;
        space.draw(&mut after, &flight, 0.0);
        let changed = before
            .pixels
            .iter()
            .zip(&after.pixels)
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changed > 20000,
            "Planets must occupy world space, not follow the screen"
        );
    }

    #[test]
    fn approaching_entering_and_passing_planets_keeps_projection_finite() {
        let space = Space::load().unwrap();
        let mut flight = Flight::new();
        let mut fb = Framebuffer::new(320, 210);
        for planet in &space.planets {
            for distance in [60.0, 25.0, 0.0, -40.0] {
                flight.position = planet.transform.position + Vec3::new(0.0, 0.0, distance);
                fb.clear();
                space.draw(&mut fb, &flight, 0.0);
            }
        }
    }

    #[test]
    fn overlapping_planets_occlude_each_other_regardless_of_draw_order() {
        let mut space = Space::load().unwrap();
        space.planets[0].transform.position = Vec3::new(-4.0, 0.0, -70.0);
        space.planets[1].transform.position = Vec3::new(4.0, 0.0, -85.0);
        let flight = Flight::new();
        let mut before = Framebuffer::new(640, 420);
        let mut after = Framebuffer::new(640, 420);
        space.draw(&mut before, &flight, 0.0);
        space.planets.reverse();
        space.draw(&mut after, &flight, 0.0);
        assert_eq!(before.pixels, after.pixels);
    }
}
