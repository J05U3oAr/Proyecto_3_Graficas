use crate::{
    collision::{CollisionWorld, PlanetCollider},
    dust::DustStorm,
    flight::Flight,
    framebuffer::Framebuffer,
    model::Model,
    renderer::{self, FlightView, WorldTransform},
};
use nalgebra_glm::Vec3;
use std::{error::Error, path::PathBuf, sync::Arc};

pub struct Planet {
    pub active: bool,
    pub name: &'static str,
    pub model: Model,
    pub transform: WorldTransform,
    pub rotation_speed: f32,
    pub dust: Option<DustStorm>,
}

pub struct Space {
    pub planets: Vec<Planet>,
    collisions: Arc<CollisionWorld>,
    debris_model: Model,
    debris: Vec<WorldTransform>,
}

impl Space {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/planets");
        let planets = vec![
            Planet {
                active: true,
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
                active: true,
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
                active: true,
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
            Planet {
                active: true,
                name: "Boreal",
                model: {
                    let mut model = Model::load(assets.join("boreal.obj"))?;
                    for texture in &mut model.textures {
                        texture.blend_longitude_seam(0.035);
                    }
                    model
                },
                transform: WorldTransform {
                    position: Vec3::new(-145.0, -85.0, -235.0),
                    radius: 42.0,
                    yaw: 0.35,
                },
                rotation_speed: 0.012,
                dust: None,
            },
        ];
        let collisions = Arc::new(CollisionWorld::new(
            planets
                .iter()
                .map(|p| PlanetCollider::new(p.name, &p.model, p.transform, p.rotation_speed))
                .collect(),
        ));
        Ok(Self {
            planets,
            collisions,
            debris_model: debris_model(),
            debris: Vec::new(),
        })
    }

    pub fn configure(&mut self, index: usize) {
        let galaxy = &crate::galaxy::GALAXIES[index];
        for (planet_index, planet) in self.planets.iter_mut().enumerate() {
            planet.active = galaxy.planets.contains(&planet_index);
            planet.transform = crate::galaxy::planet_transform(index, planet_index);
        }
        self.collisions = Arc::new(CollisionWorld::new(
            self.active_planets()
                .map(|p| PlanetCollider::new(p.name, &p.model, p.transform, p.rotation_speed))
                .collect(),
        ));
        let mut seed = galaxy.seed;
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed as f32 / u32::MAX as f32
        };
        self.debris = (0..galaxy.debris)
            .map(|_| {
                let angle = next() * std::f32::consts::TAU;
                let distance = 82.0 + next() * 72.0;
                WorldTransform {
                    position: Vec3::new(
                        angle.cos() * distance,
                        -43.0 + next() * 18.0,
                        -200.0 + angle.sin() * distance,
                    ),
                    radius: 0.8 + next() * 2.8,
                    yaw: next() * std::f32::consts::TAU,
                }
            })
            .collect();
    }

    pub fn active_planets(&self) -> impl Iterator<Item = &Planet> {
        self.planets.iter().filter(|planet| planet.active)
    }

    pub fn new_flight(&self) -> Flight {
        Flight::with_collisions(Arc::clone(&self.collisions))
    }

    pub fn approach(&self, flight: &mut Flight, name: &str) -> Result<(), Box<dyn Error>> {
        let planet = self
            .planets
            .iter()
            .find(|p| p.active && p.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("Planeta desconocido: {name}"))?;
        flight.position = planet.transform.position + Vec3::z() * (planet.transform.radius + 24.0);
        Ok(())
    }

    pub fn draw(&self, fb: &mut Framebuffer, flight: &Flight, seconds: f32) {
        let view = FlightView::new(flight, fb.width, fb.height);
        for planet in self.active_planets() {
            let mut transform = planet.transform;
            transform.yaw += seconds * planet.rotation_speed;
            renderer::draw_world_model(fb, &planet.model, &view, transform);
        }
        for &transform in &self.debris {
            renderer::draw_world_model(fb, &self.debris_model, &view, transform);
        }
    }

    pub fn preview(
        &self,
        fb: &mut Framebuffer,
        name: &str,
        seconds: f32,
    ) -> Result<(), Box<dyn Error>> {
        let planet = self
            .planets
            .iter()
            .find(|planet| planet.active && planet.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("Planeta desconocido: {name}"))?;
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        let target_radius = fb.width.min(fb.height) as f32 * 0.40;
        let distance = planet.transform.radius
            * (1.0 + (renderer::FLIGHT_FOCAL / target_radius).powi(2)).sqrt();
        flight.position = planet.transform.position + Vec3::new(0.0, -1.6, distance - 8.0);
        let mut transform = planet.transform;
        transform.yaw += seconds * planet.rotation_speed;
        fb.clear();
        fb.pixels.fill(0x03060c);
        renderer::draw_world_model(
            fb,
            &planet.model,
            &FlightView::new(&flight, fb.width, fb.height),
            transform,
        );
        Ok(())
    }

    pub fn draw_atmospheres(&self, fb: &mut Framebuffer, flight: &Flight, seconds: f32) {
        let view = FlightView::new(flight, fb.width, fb.height);
        for planet in self.active_planets() {
            if let Some(dust) = &planet.dust {
                let mut transform = planet.transform;
                transform.yaw += seconds * planet.rotation_speed;
                dust.draw(fb, &view, transform, seconds);
            }
        }
    }
}

fn debris_model() -> Model {
    let vertices = vec![
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-0.8, 0.0, 0.0),
        Vec3::new(0.0, 0.7, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.0, 0.0, 0.9),
        Vec3::new(0.0, 0.0, -0.7),
    ];
    let faces = [
        [0, 2, 4],
        [4, 2, 1],
        [1, 2, 5],
        [5, 2, 0],
        [0, 4, 3],
        [4, 1, 3],
        [1, 5, 3],
        [5, 0, 3],
    ]
    .into_iter()
    .map(|indices| crate::model::Face {
        indices,
        color: crate::color::Color::new(112, 122, 142),
        uv: None,
        texture: None,
        unlit: false,
    })
    .collect();
    Model {
        vertices,
        faces,
        center: Vec3::zeros(),
        radius: 1.0,
        textures: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destinations_restore_layouts_reproduce_debris_and_keep_active_collisions() {
        let mut space = Space::load().unwrap();
        for galaxy in [1, 2, 0, 2, 1, 0] {
            space.configure(galaxy);
            let definition = &crate::galaxy::GALAXIES[galaxy];
            assert_eq!(space.active_planets().count(), definition.planets.len());
            assert_eq!(space.debris.len(), definition.debris);
            let first_debris: Vec<_> = space.debris.iter().map(|t| t.position).collect();
            space.configure(galaxy);
            assert_eq!(
                first_debris,
                space.debris.iter().map(|t| t.position).collect::<Vec<_>>()
            );
            let solid = space
                .active_planets()
                .find(|p| p.name != "Oquedad")
                .unwrap();
            let start = solid.transform.position + Vec3::z() * (solid.transform.radius + 20.0);
            let motion = space
                .collisions
                .move_ship(start, -Vec3::z() * 80.0, 0.0, 0.1);
            assert!(
                motion
                    .contacts
                    .iter()
                    .any(|contact| contact.planet == solid.name)
            );
            for planet in space.planets.iter().filter(|p| !p.active) {
                assert!(space.approach(&mut Flight::new(), planet.name).is_err());
            }
        }
    }
    use crate::framebuffer::BACKGROUND;

    #[test]
    fn all_solid_planets_block_a_ship_swept_across_the_whole_world() {
        let space = Space::load().unwrap();
        for planet in space.planets.iter().filter(|p| p.name != "Oquedad") {
            let world = CollisionWorld::new(vec![PlanetCollider::new(
                planet.name,
                &planet.model,
                planet.transform,
                planet.rotation_speed,
            )]);
            let r = planet.transform.radius;
            let result = world.move_ship(
                planet.transform.position + Vec3::z() * r * 3.0,
                -Vec3::z() * r * 6.0,
                12.0,
                0.0,
            );
            assert!(
                !result.contacts.is_empty(),
                "{} must block high speed motion",
                planet.name
            );
            assert!(result.position.z > planet.transform.position.z);
        }
    }

    #[test]
    fn oquedad_preserves_its_real_tunnel_and_collides_with_its_walls_after_rotation() {
        let space = Space::load().unwrap();
        let p = space.planets.iter().find(|p| p.name == "Oquedad").unwrap();
        let world = CollisionWorld::new(vec![PlanetCollider::new(
            p.name,
            &p.model,
            p.transform,
            p.rotation_speed,
        )]);
        for time in [0.0, 15.0, 80.0] {
            let angle = p.transform.yaw + time * p.rotation_speed;
            let rotate = |v: Vec3| {
                Vec3::new(
                    v.x * angle.cos() + v.z * angle.sin(),
                    v.y,
                    -v.x * angle.sin() + v.z * angle.cos(),
                )
            };
            let center = p.transform.position
                + rotate(
                    (Vec3::new(0.0, 0.1, 0.0) - p.model.center)
                        * (p.transform.radius / p.model.radius),
                );
            let axis = rotate(Vec3::new(0.16, 0.20, 1.0).normalize());
            let through = world.move_ship(center + axis * 45.0, -axis * 90.0, time, 0.0);
            assert!(
                through.contacts.is_empty(),
                "The central passage must stay open at time {time}"
            );
            assert!((through.position - (center - axis * 45.0)).norm() < 0.001);
            let wall = world.move_ship(
                center,
                rotate(Vec3::new(1.0, 1.0, 0.0).normalize()) * 35.0,
                time,
                0.0,
            );
            assert!(
                !wall.contacts.is_empty(),
                "Tunnel walls must collide at time {time}"
            );
        }
    }

    #[test]
    fn slow_contact_stops_inward_motion_but_fast_impact_explodes_and_respawns() {
        let space = Space::load().unwrap();
        let p = &space.planets[0];
        let probe = space.collisions.move_ship(
            p.transform.position + Vec3::z() * 60.0,
            -Vec3::z() * 60.0,
            0.0,
            0.0,
        );
        let normal = probe.contacts[0].normal;
        let start = probe.position + normal * 0.05;
        let mut slow = space.new_flight();
        slow.position = start;
        slow.velocity = -normal * 10.0;
        slow.advance(1.0 / 30.0, (0.0, 0.0), false, false);
        assert!(slow.crash.is_none());
        assert!(slow.contact_notice > 0.0);
        assert!(slow.velocity.dot(&normal) > -0.5);
        let mut fast = space.new_flight();
        fast.position = start;
        fast.velocity = -normal * 62.0;
        fast.advance(1.0 / 60.0, (0.0, 0.0), true, false);
        assert_eq!(fast.crash.as_ref().unwrap().planet, "Siroco");
        assert_eq!(fast.velocity, Vec3::zeros());
        fast.advance(0.0, (1.0, 0.0), true, false);
        assert_eq!(fast.crash.as_ref().unwrap().age, 0.0);
        for _ in 0..125 {
            fast.advance(1.0 / 60.0, (0.0, 0.0), false, false);
        }
        assert!(fast.crash.is_none() && fast.respawn_notice > 0.0);
        assert_eq!(fast.position, Vec3::zeros());
        fast.position = start;
        fast.velocity = -normal * 62.0;
        fast.advance(1.0 / 60.0, (0.0, 0.0), true, false);
        assert!(
            fast.crash.is_some(),
            "Respawning must retain the collision world"
        );
    }

    #[test]
    fn fast_planet_impacts_are_consistent_at_ten_and_sixty_frames_per_second() {
        let space = Space::load().unwrap();
        let simulate = |fps: usize| {
            let mut flight = space.new_flight();
            space.approach(&mut flight, "Boreal").unwrap();
            for _ in 0..fps * 3 {
                flight.advance(1.0 / fps as f32, (1.0, 0.0), true, false);
                if flight.crash.is_some() {
                    break;
                }
            }
            assert_eq!(flight.crash.as_ref().unwrap().planet, "Boreal");
            flight.position
        };
        assert!((simulate(10) - simulate(60)).norm() < 0.05);
    }

    #[test]
    fn boreal_has_a_lit_cloud_map_and_is_visible_below_the_launch_planets() {
        let space = Space::load().unwrap();
        let planet = space.planets.iter().find(|p| p.name == "Boreal").unwrap();
        assert_eq!(planet.model.textures.len(), 1);
        assert!(
            planet
                .model
                .faces
                .iter()
                .all(|face| face.uv.is_some() && !face.unlit)
        );
        let flight = Flight::new();
        let mut fb = Framebuffer::new(1280, 840);
        let view = FlightView::new(&flight, fb.width, fb.height);
        let center = view.project(planet.transform.position).unwrap();
        assert!((180.0..350.0).contains(&center.x));
        assert!((500.0..630.0).contains(&center.y));
        renderer::draw_world_model(&mut fb, &planet.model, &view, planet.transform);
        let cyan = fb
            .pixels
            .iter()
            .filter(|pixel| {
                let (r, g, b) = ((**pixel >> 16) & 255, (**pixel >> 8) & 255, **pixel & 255);
                g > r * 2 && b > r * 2 && g > 50
            })
            .count();
        assert!(
            cyan > 15_000,
            "The ice planet should retain its turquoise cloud bands"
        );
        let before = fb.pixels.clone();
        fb.clear();
        let mut rotated = planet.transform;
        rotated.yaw += 0.7;
        renderer::draw_world_model(&mut fb, &planet.model, &view, rotated);
        assert!(
            fb.pixels
                .iter()
                .zip(&before)
                .filter(|(a, b)| a != b)
                .count()
                > 15_000
        );
    }

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
