use crate::{
    collision::{CollisionWorld, PlanetCollider},
    dust::DustStorm,
    flight::Flight,
    framebuffer::Framebuffer,
    model::Model,
    renderer::{self, FlightView, WorldTransform},
};
use nalgebra_glm::Vec3;
use std::{
    error::Error,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

pub struct Planet {
    pub active: bool,
    pub name: &'static str,
    pub model: Arc<Model>,
    pub motion: crate::orbit::Motion,
    pub is_sun: bool,
    pub transform: WorldTransform,
    pub rotation_speed: f32,
    pub dust: Option<DustStorm>,
}

impl Planet {
    fn halo_color(&self) -> u32 {
        if self.name == "Asterion" {
            0x87bbff
        } else {
            0xff9d39
        }
    }
    pub fn surface_effect(&self, seconds: f32) -> crate::shaders::SurfaceEffect {
        use crate::shaders::SurfaceEffect;
        match self.name {
            "Nivalis" => SurfaceEffect::Ice,
            "Asterion" => SurfaceEffect::Stellar { seconds },
            "Onice" => SurfaceEffect::Obsidian,
            "Vespera" => SurfaceEffect::Snow { seconds },
            "Prisma" => SurfaceEffect::Crystal,
            "Kepler-186f" => SurfaceEffect::Kepler { seconds },
            "Jovara" => SurfaceEffect::PolarClouds { seconds },
            "Helios II" => SurfaceEffect::Solar { seconds },
            "Boreal" => SurfaceEffect::Clouds {
                seconds,
                seed: 37.0,
                flow: 0.018,
            },
            "Flareon" => SurfaceEffect::Lava { seconds },
            "Metis" => SurfaceEffect::Rock {
                seconds,
                seed: 19.0,
            },
            "Oquedad" => SurfaceEffect::Rock {
                seconds,
                seed: 43.0,
            },
            _ => SurfaceEffect::None,
        }
    }
    pub fn world_transform(&self, seconds: f32) -> WorldTransform {
        let mut transform = self.transform;
        transform.position += self.motion.displacement(seconds);
        transform.yaw += seconds * self.rotation_speed;
        transform
    }
}

fn load_models() -> Result<&'static [Arc<Model>], Box<dyn Error>> {
    static MODELS: OnceLock<Result<Vec<Arc<Model>>, String>> = OnceLock::new();
    let result = MODELS.get_or_init(|| {
        let load = || -> Result<Vec<Arc<Model>>, Box<dyn Error>> {
            let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/planets");
            let paths = [
                "siroco.obj",
                "oquedad.obj",
                "jovara.obj",
                "boreal.obj",
                "runtime/helios-ii.obj",
                "runtime/flareon.obj",
                "runtime/kepler-186f.obj",
                "runtime/metis.obj",
                "nivalis/nivalis.obj",
                "asterion/asterion.obj",
                "onice/onice.obj",
                "vespera/vespera.obj",
                "prisma/prisma.obj",
            ];
            paths
                .iter()
                .enumerate()
                .map(|(index, path)| {
                    let mut model = if index == 8 {
                        crate::ice_planet::load(assets.join(path))?
                    } else {
                        Model::load(assets.join(path))?
                    };
                    if matches!(index, 10..=12) {
                        // Asymmetric terrain must rotate around its geographical origin.
                        model.center = Vec3::zeros();
                        model.radius = model.vertices.iter().map(Vec3::norm).fold(0.0, f32::max);
                    }
                    if index >= 3 {
                        for texture in &mut model.textures {
                            texture.blend_longitude_seam(if index == 3 { 0.035 } else { 0.015 });
                        }
                    }
                    Ok(Arc::new(model))
                })
                .collect()
        };
        load().map_err(|error| error.to_string())
    });
    match result {
        Ok(models) => Ok(models),
        Err(message) => Err(message.clone().into()),
    }
}

fn opacity_layer(model: &Model, transparent: bool) -> Model {
    debug_assert!(
        model.textures.is_empty(),
        "Las capas de Prisma usan materiales sin texturas"
    );
    Model {
        vertices: model.vertices.clone(),
        faces: model
            .faces
            .iter()
            .filter(|f| (f.opacity < 1.0) == transparent)
            .copied()
            .collect(),
        center: model.center,
        radius: model.radius,
        textures: Vec::new(),
    }
}

fn draw_solar_halo(fb: &mut Framebuffer, view: &FlightView, transform: WorldTransform, tint: u32) {
    let Some(center) = view.project(transform.position) else {
        return;
    };
    let radius = (transform.radius * renderer::FLIGHT_FOCAL * center.z).max(1.0);
    let reach = radius * 1.8;
    let bounds = (
        (center.x - reach).max(0.0) as usize,
        (center.y - reach).max(0.0) as usize,
        (center.x + reach).ceil().max(0.0).min(fb.width as f32) as usize,
        (center.y + reach).ceil().max(0.0).min(fb.height as f32) as usize,
    );
    fb.shade_rect(bounds, |x, y, pixel, depth| {
        if center.z <= *depth {
            return;
        }
        let distance = (((x as f32 + 0.5 - center.x).powi(2)
            + (y as f32 + 0.5 - center.y).powi(2))
            / radius.powi(2))
        .sqrt();
        let strength = (-((distance - 1.0) / 0.25).powi(2)).exp() * 0.4;
        if distance > 0.9 && distance < 1.8 {
            *pixel = crate::framebuffer::emission_color(*pixel, tint, strength, 0.0);
        }
    });
}

pub struct Space {
    pub planets: Vec<Planet>,
    collisions: Arc<CollisionWorld>,
    black_hole: crate::black_hole::BlackHole,
    debris_model: Model,
    debris: Vec<WorldTransform>,
    nivalis: Arc<Model>,
    boreal: Arc<Model>,
    prisma_opaque: Model,
    prisma_glass: Model,
}

impl Space {
    pub fn solar_lighting(&self, seconds: f32) -> Option<crate::lighting::SolarLight> {
        let sun = self.planets.iter().find(|p| p.active && p.is_sun)?;
        let transform = sun.world_transform(seconds);
        Some(crate::lighting::SolarLight {
            position: transform.position,
            radius: transform.radius,
            occluders: self
                .planets
                .iter()
                .filter(|p| p.active && !p.is_sun)
                .map(|p| {
                    let transform = p.world_transform(seconds);
                    crate::lighting::Occluder {
                        center: transform.position,
                        radius: transform.radius,
                    }
                })
                .collect(),
        })
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        let models = load_models()?;
        let planets = vec![
            Planet {
                active: true,
                name: "Siroco",
                model: Arc::clone(&models[0]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
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
                model: Arc::clone(&models[1]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
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
                model: Arc::clone(&models[2]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
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
                model: Arc::clone(&models[3]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: WorldTransform {
                    position: Vec3::new(-145.0, -85.0, -235.0),
                    radius: 42.0,
                    yaw: 0.35,
                },
                rotation_speed: 0.012,
                dust: None,
            },
            Planet {
                active: false,
                name: "Helios II",
                model: Arc::clone(&models[4]),
                motion: crate::orbit::Motion::default(),
                is_sun: true,
                transform: crate::galaxy::planet_transform(1, 4),
                rotation_speed: 0.045,
                dust: None,
            },
            Planet {
                active: false,
                name: "Flareon",
                model: Arc::clone(&models[5]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(1, 5),
                rotation_speed: 0.036,
                dust: None,
            },
            Planet {
                active: false,
                name: "Kepler-186f",
                model: Arc::clone(&models[6]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(1, 6),
                rotation_speed: 0.018,
                dust: None,
            },
            Planet {
                active: false,
                name: "Metis",
                model: Arc::clone(&models[7]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(1, 7),
                rotation_speed: 0.024,
                dust: None,
            },
            Planet {
                active: false,
                name: "Asterion",
                model: Arc::clone(&models[9]),
                motion: crate::orbit::Motion::default(),
                is_sun: true,
                transform: crate::galaxy::planet_transform(2, 8),
                rotation_speed: 0.009,
                dust: None,
            },
            Planet {
                active: false,
                name: "Onice",
                model: Arc::clone(&models[10]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(2, 9),
                rotation_speed: 0.013,
                dust: None,
            },
            Planet {
                active: false,
                name: "Vespera",
                model: Arc::clone(&models[11]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(2, 10),
                rotation_speed: 0.01,
                dust: Some(DustStorm::snow()),
            },
            Planet {
                active: false,
                name: "Prisma",
                model: Arc::clone(&models[12]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(2, 11),
                rotation_speed: 0.011,
                dust: None,
            },
        ];
        let collisions = Arc::new(CollisionWorld::new(
            planets
                .iter()
                .filter(|p| p.active)
                .map(|p| {
                    PlanetCollider::new(p.name, &p.model, p.transform, p.rotation_speed)
                        .with_motion(p.motion)
                })
                .collect(),
        ));
        Ok(Self {
            planets,
            collisions,
            black_hole: crate::galaxy::black_hole(0),
            debris_model: debris_model(),
            debris: Vec::new(),
            nivalis: Arc::clone(&models[8]),
            boreal: Arc::clone(&models[3]),
            prisma_opaque: opacity_layer(&models[12], false),
            prisma_glass: opacity_layer(&models[12], true),
        })
    }

    pub fn configure(&mut self, index: usize) {
        self.black_hole = crate::galaxy::black_hole(index);
        let galaxy = &crate::galaxy::GALAXIES[index];
        for (planet_index, planet) in self.planets.iter_mut().enumerate() {
            if planet_index == 3 {
                planet.name = if index == 2 { "Nivalis" } else { "Boreal" };
                planet.model = Arc::clone(if index == 2 {
                    &self.nivalis
                } else {
                    &self.boreal
                });
            }
            planet.active = galaxy.planets.contains(&planet_index);
            planet.transform = crate::galaxy::planet_transform(index, planet_index);
            planet.motion = if index == 1 {
                crate::orbit::aurora_motion(planet_index)
            } else if index == 2 {
                crate::orbit::umbria_motion(planet_index)
            } else {
                crate::orbit::Motion::default()
            };
        }
        self.collisions = Arc::new(CollisionWorld::new(
            self.active_planets()
                .map(|p| {
                    PlanetCollider::new(p.name, &p.model, p.transform, p.rotation_speed)
                        .with_motion(p.motion)
                })
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
                let distance = if index == 1 {
                    860.0 + next() * 40.0
                } else if index == 2 {
                    950.0 + next() * 110.0
                } else {
                    82.0 + next() * 72.0
                };
                WorldTransform {
                    position: (if index == 2 {
                        crate::galaxy::UMBRIA_CENTER
                    } else {
                        Vec3::zeros()
                    }) + Vec3::new(
                        angle.cos() * distance,
                        if index == 1 {
                            -15.0 + next() * 30.0
                        } else {
                            -43.0 + next() * 18.0
                        },
                        if index == 1 || index == 2 {
                            angle.sin() * distance
                        } else {
                            -200.0 + angle.sin() * distance
                        },
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

    pub fn sun_position(&self, seconds: f32) -> Option<Vec3> {
        self.active_planets()
            .find(|planet| planet.is_sun)
            .map(|sun| sun.world_transform(seconds).position)
    }

    pub fn new_flight(&self) -> Flight {
        let mut flight = Flight::with_collisions(Arc::clone(&self.collisions));
        flight.black_hole = self.black_hole;
        flight.set_navigation_targets(
            self.active_planets()
                .filter(|planet| !planet.is_sun)
                .map(|planet| crate::local_navigation::Reference {
                    name: planet.name,
                    center: planet.transform.position,
                    radius: planet.transform.radius,
                    motion: planet.motion,
                })
                .collect(),
        );
        flight
    }

    pub fn approach(&self, flight: &mut Flight, name: &str) -> Result<(), Box<dyn Error>> {
        let planet = self
            .planets
            .iter()
            .find(|p| p.active && p.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("Planeta desconocido: {name}"))?;
        let transform = planet.world_transform(flight.seconds);
        flight.position = transform.position + Vec3::z() * (transform.radius + 24.0);
        Ok(())
    }

    pub fn draw(&self, fb: &mut Framebuffer, flight: &Flight, seconds: f32) {
        let view = FlightView::new(flight, fb.width, fb.height);
        self.draw_view(fb, &view, seconds);
    }

    pub fn draw_view(&self, fb: &mut Framebuffer, view: &FlightView, seconds: f32) {
        for planet in self.active_planets() {
            let transform = planet.world_transform(seconds);
            let model = if planet.name == "Prisma" {
                &self.prisma_opaque
            } else {
                &planet.model
            };
            renderer::draw_world_model_shaded(
                fb,
                model,
                &view,
                transform,
                self.sun_position(seconds),
                planet.surface_effect(seconds),
            );
        }
        for sun in self.active_planets().filter(|planet| planet.is_sun) {
            draw_solar_halo(fb, &view, sun.world_transform(seconds), sun.halo_color());
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
        let transform = planet.world_transform(seconds);
        let target_radius = fb.width.min(fb.height) as f32 * 0.40;
        let distance = planet.transform.radius
            * (1.0 + (renderer::FLIGHT_FOCAL / target_radius).powi(2)).sqrt();
        flight.position = transform.position + Vec3::new(0.0, -1.6, distance - 8.0);
        fb.clear();
        fb.pixels.fill(0x03060c);
        let view = FlightView::new(&flight, fb.width, fb.height);
        renderer::draw_world_model_shaded(
            fb,
            &planet.model,
            &view,
            transform,
            self.sun_position(seconds),
            planet.surface_effect(seconds),
        );
        if planet.is_sun {
            draw_solar_halo(fb, &view, transform, planet.halo_color());
        }
        Ok(())
    }

    pub fn draw_atmospheres(&self, fb: &mut Framebuffer, flight: &Flight, seconds: f32) {
        let view = FlightView::new(flight, fb.width, fb.height);
        self.draw_atmospheres_view(fb, &view, seconds);
    }

    pub fn draw_atmospheres_view(&self, fb: &mut Framebuffer, view: &FlightView, seconds: f32) {
        for planet in self.active_planets() {
            if planet.name == "Prisma" {
                // All opaque planets and the ship establish depth before the glass pass.
                renderer::draw_world_model_shaded(
                    fb,
                    &self.prisma_glass,
                    view,
                    planet.world_transform(seconds),
                    self.sun_position(seconds),
                    planet.surface_effect(seconds),
                );
            }
            if let Some(dust) = &planet.dust {
                let transform = planet.world_transform(seconds);
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
        opacity: 1.0,
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
    fn prismas_ground_contains_blue_navy_and_purple_crystal_regions() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let model = &space
            .active_planets()
            .find(|p| p.name == "Prisma")
            .unwrap()
            .model;
        let ground: Vec<_> = model
            .faces
            .iter()
            .filter(|f| {
                f.opacity >= 1.0 && f.indices.iter().all(|&i| model.vertices[i].norm() < 1.06)
            })
            .collect();
        let blue = ground
            .iter()
            .filter(|f| {
                f.color.g >= 65 && f.color.b >= 120 && f.color.b as f32 > f.color.r as f32 * 1.6
            })
            .count();
        let navy = ground
            .iter()
            .filter(|f| f.color.b <= 110 && f.color.b as f32 > f.color.r as f32 * 1.7)
            .count();
        let purple = ground
            .iter()
            .filter(|f| {
                f.color.r >= 65
                    && (f.color.g as f32) < f.color.r as f32 * 0.8
                    && f.color.b as f32 > f.color.r as f32 * 1.2
            })
            .count();
        assert!(
            blue > 200 && navy > 100 && purple > 100,
            "El suelo debe tener zonas de cristal azul, azul marino y purpura: {blue}/{navy}/{purple}"
        );
    }

    #[test]
    fn prisma_glass_blends_after_opaque_geometry_and_preserves_foreground_ship_depth() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        for planet in &mut space.planets {
            planet.dust = None;
        }
        let mut flight = space.new_flight();
        space.approach(&mut flight, "Prisma").unwrap();
        flight.camera.pitch = 0.0;
        let view = FlightView::new(&flight, 640, 420);
        let mut frame = Framebuffer::new(640, 420);
        space.draw_view(&mut frame, &view, 0.0);
        let opaque = frame.pixels.clone();
        let depth = frame.depth.clone();
        space.draw_atmospheres_view(&mut frame, &view, 0.0);
        assert_eq!(frame.depth, depth);
        let changed: Vec<_> = frame
            .pixels
            .iter()
            .zip(&opaque)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert!(
            changed.len() > 200,
            "Las cubiertas de cristal deben verse sobre los nucleos"
        );
        let pixel = changed[changed.len() / 2];
        frame.pixels = opaque;
        frame.depth = depth;
        frame.point(
            (pixel % 640) as i32,
            (pixel / 640) as i32,
            2.0,
            0xff00ff,
            true,
        );
        space.draw_atmospheres_view(&mut frame, &view, 0.0);
        assert_eq!(frame.pixels[pixel], 0xff00ff);
        assert_eq!(frame.depth[pixel], 2.0);
    }

    #[test]
    fn prisma_has_translucent_crystals_with_solid_exteriors_and_its_own_orbit() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let planet = space
            .active_planets()
            .find(|p| p.name == "Prisma")
            .expect("Falta Prisma");
        assert!(planet.model.faces.len() < 23000);
        assert!(
            planet
                .model
                .faces
                .iter()
                .filter(|f| f.opacity < 1.0)
                .count()
                > 1000
        );
        assert!(
            planet
                .model
                .faces
                .iter()
                .filter(|f| f.opacity >= 1.0)
                .count()
                > 7000
        );
        assert!(planet.model.faces.iter().all(|f| !f.unlit));
        assert!(planet.model.vertices.iter().any(|v| v.norm() > 1.3));
        for seconds in [0.0, 40.0, 190.0, 600.0] {
            let transform = planet.world_transform(seconds);
            assert!(
                ((transform.position - space.sun_position(seconds).unwrap()).norm() - 2450.0).abs()
                    < 0.003
            );
            let hit = space.collisions.move_ship(
                transform.position + Vec3::z() * 110.0,
                -Vec3::z() * 90.0,
                seconds,
                0.01,
            );
            assert!(hit.contacts.iter().any(|c| c.planet == "Prisma"));
        }
        let transform = WorldTransform {
            position: Vec3::zeros(),
            radius: 78.0,
            yaw: 0.0,
        };
        let solid = CollisionWorld::new(vec![PlanetCollider::new(
            "Prisma",
            &planet.model,
            transform,
            0.0,
        )]);
        let core = CollisionWorld::new(vec![PlanetCollider::new_opaque(
            "Prisma",
            &planet.model,
            transform,
            0.0,
        )]);
        let scale = 78.0 / planet.model.radius;
        let collides_with_shell =
            planet
                .model
                .faces
                .iter()
                .filter(|f| f.opacity < 1.0)
                .any(|face| {
                    let [a, b, c] = face.indices.map(|i| planet.model.vertices[i] * scale);
                    let center = (a + b + c) / 3.0;
                    if center.norm() < 65.0 {
                        return false;
                    }
                    let normal = (b - a).cross(&(c - a)).normalize();
                    let start = center + normal * (crate::collision::SHIP_RADIUS + 2.0);
                    let delta = -normal * 3.0;
                    !solid.move_ship(start, delta, 0.0, 0.1).contacts.is_empty()
                        && core.move_ship(start, delta, 0.0, 0.1).contacts.is_empty()
                });
        assert!(
            collides_with_shell,
            "La cubierta transparente tambien debe detener la nave"
        );
        space.configure(0);
        assert!(!space.active_planets().any(|p| p.name == "Prisma"));
    }

    #[test]
    fn vespera_has_cold_snow_and_colliding_canyon_relief_in_its_own_orbit() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let planet = space
            .active_planets()
            .find(|p| p.name == "Vespera")
            .expect("Falta Vespera");
        assert!(planet.model.faces.len() < 30000);
        assert!(
            planet
                .model
                .faces
                .iter()
                .all(|f| f.color.b > f.color.r && !f.unlit)
        );
        assert!(
            planet
                .model
                .faces
                .iter()
                .filter(|f| f.color.r >= 150)
                .all(|f| f.color.b - f.color.r <= 24),
            "La nieve debe ser casi neutra, sin saturacion violeta de material cremoso"
        );
        assert!(planet.model.vertices.iter().any(|v| v.norm() < 0.87));
        assert!(planet.model.vertices.iter().any(|v| v.norm() > 1.1));
        assert!(planet.dust.is_some());
        for seconds in [0.0, 40.0, 190.0, 600.0] {
            let transform = planet.world_transform(seconds);
            let sun = space.sun_position(seconds).unwrap();
            assert!(((transform.position - sun).norm() - 1900.0).abs() < 0.003);
            let contact = space.collisions.move_ship(
                transform.position + Vec3::z() * 110.0,
                -Vec3::z() * 90.0,
                seconds,
                0.01,
            );
            assert!(contact.contacts.iter().any(|c| c.planet == "Vespera"));
        }
        let world = CollisionWorld::new(vec![PlanetCollider::new(
            "Vespera",
            &planet.model,
            WorldTransform {
                position: Vec3::zeros(),
                radius: 84.0,
                yaw: 0.0,
            },
            0.0,
        )]);
        let axis = Vec3::new(-0.77, 0.20, -0.61).normalize();
        let tangent = axis.cross(&Vec3::y()).normalize();
        let right = tangent * 0.45_f32.cos() + tangent.cross(&axis) * 0.45_f32.sin();
        let rim = (axis + right * 0.23).normalize();
        let sample =
            |direction: Vec3| world.move_ship(direction * 115.0, -direction * 75.0, 0.0, 0.1);
        let floor = sample(axis);
        let wall = sample(rim);
        assert!(!floor.contacts.is_empty() && !wall.contacts.is_empty());
        assert!(
            wall.position.norm() - floor.position.norm() > 5.0,
            "El canon debe ser una depresion real en la malla"
        );
        space.configure(1);
        assert!(!space.active_planets().any(|p| p.name == "Vespera"));
    }

    #[test]
    fn onice_orbits_asterion_with_real_craters_and_traversable_rock_arches() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let planet = space
            .active_planets()
            .find(|p| p.name == "Onice")
            .expect("Falta Onice en Umbria");
        assert!(planet.model.faces.len() < 30000);
        assert!(planet.model.vertices.iter().any(|p| p.norm() < 0.9));
        assert!(planet.model.vertices.iter().any(|p| p.norm() > 1.1));
        assert!(planet.model.faces.iter().all(|face| !face.unlit));
        assert!(
            planet
                .model
                .faces
                .iter()
                .filter(|face| face.color.r < 40)
                .count()
                > planet.model.faces.len() / 2
        );
        for seconds in [0.0, 30.0, 190.0, 600.0] {
            let transform = planet.world_transform(seconds);
            let sun = space.sun_position(seconds).unwrap();
            assert!(((transform.position - sun).norm() - 800.0).abs() < 0.002);
            let hit = space.collisions.move_ship(
                transform.position + Vec3::z() * 100.0,
                -Vec3::z() * 80.0,
                seconds,
                0.01,
            );
            assert!(hit.contacts.iter().any(|hit| hit.planet == "Onice"));
        }
        let isolated = CollisionWorld::new(vec![PlanetCollider::new(
            "Onice",
            &planet.model,
            WorldTransform {
                position: Vec3::zeros(),
                radius: 72.0,
                yaw: 0.0,
            },
            0.0,
        )]);
        let normal = Vec3::new(0.05, 0.18, 1.0).normalize();
        let tangent = normal.cross(&Vec3::y()).normalize();
        let other = tangent.cross(&normal).normalize();
        let right = tangent * 0.35_f32.cos() + other * 0.35_f32.sin();
        let forward = right.cross(&normal);
        let scale = 72.0 / planet.model.radius;
        let travel = forward * (0.36 * scale);
        let start = (normal - forward * 0.18) * scale;
        let passage = isolated.move_ship(start, travel, 0.0, 0.1);
        assert!(
            passage.contacts.is_empty(),
            "El hueco del arco debe permitir pasar"
        );
        assert!((passage.position - start - travel).norm() < 0.02);
        let arch = isolated.move_ship(start + normal * (0.12 * scale), travel, 0.0, 0.1);
        assert!(!arch.contacts.is_empty(), "La boveda debe detener la nave");
        let impact = isolated.move_ship(normal * 100.0, -normal * 80.0, 0.0, 0.1);
        assert!(!impact.contacts.is_empty());
        space.configure(0);
        assert!(!space.active_planets().any(|p| p.name == "Onice"));
    }

    #[test]
    fn umbria_has_a_massive_sun_and_nivalis_orbits_its_light_and_collision_world() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let sun = space
            .active_planets()
            .find(|p| p.is_sun)
            .expect("Umbria necesita su sol");
        let nivalis = space
            .active_planets()
            .find(|p| p.name == "Nivalis")
            .unwrap();
        assert_eq!(sun.name, "Asterion");
        assert!(sun.transform.radius >= 360.0);
        assert!(sun.transform.radius > nivalis.transform.radius * 5.0);
        assert!(sun.model.faces.iter().all(|face| face.unlit));
        for time in [0.0, 40.0, 190.0, 600.0] {
            let star = sun.world_transform(time);
            let planet = nivalis.world_transform(time);
            assert!(((planet.position - star.position).norm() - 1400.0).abs() < 0.002);
            assert_eq!(space.solar_lighting(time).unwrap().position, star.position);
            let hit = space.collisions.move_ship(
                planet.position + Vec3::z() * 90.0,
                -Vec3::z() * 60.0,
                time,
                0.01,
            );
            assert!(
                hit.contacts
                    .iter()
                    .any(|contact| contact.planet == "Nivalis")
            );
        }
        assert!(
            (nivalis.world_transform(100.0).position - nivalis.world_transform(0.0).position)
                .norm()
                > 400.0
        );
        assert!(
            (crate::flight::SPAWN_POSITION - sun.transform.position).norm()
                > sun.transform.radius + 100.0
        );
        assert!(
            space
                .debris
                .iter()
                .all(
                    |rock| (rock.position - sun.transform.position).norm() - rock.radius
                        > sun.transform.radius + 100.0
                )
        );
        let flight = space.new_flight();
        let reference = flight.navigation_reference().unwrap();
        assert!(
            space
                .active_planets()
                .any(|p| !p.is_sun && p.name == reference.name)
        );
        space.configure(0);
        assert!(!space.active_planets().any(|p| p.name == "Asterion"));
        space.configure(1);
        assert_eq!(
            space.active_planets().find(|p| p.is_sun).unwrap().name,
            "Helios II"
        );
    }

    #[test]
    fn asterion_plasma_animates_emits_its_own_light_and_keeps_depth() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let star = space.active_planets().find(|p| p.is_sun).unwrap();
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        let view = FlightView::new(&flight, 256, 192);
        let transform = WorldTransform {
            position: Vec3::new(0.0, 0.0, -1200.0),
            radius: 360.0,
            yaw: 0.0,
        };
        let render = |seconds, light| {
            let mut fb = Framebuffer::new(256, 192);
            fb.point(128, 96, 2.0, 0xff00ff, true);
            renderer::draw_world_model_shaded(
                &mut fb,
                &star.model,
                &view,
                transform,
                Some(light),
                star.surface_effect(seconds),
            );
            fb
        };
        let first = render(0.0, Vec3::new(3000.0, 0.0, 0.0));
        let opposite_light = render(0.0, Vec3::new(-3000.0, 0.0, 0.0));
        let later = render(4.0, Vec3::new(3000.0, 0.0, 0.0));
        assert_eq!(first.pixels, opposite_light.pixels);
        assert_eq!(first.depth, later.depth);
        assert_eq!(later.pixels[96 * 256 + 128], 0xff00ff);
        assert!(
            first
                .pixels
                .iter()
                .zip(&later.pixels)
                .filter(|(a, b)| a != b)
                .count()
                > 1000
        );
    }

    #[test]
    fn umbria_departure_is_clear_and_local_flight_follows_planets_at_different_frame_rates() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let mut departure = space.new_flight();
        for _ in 0..600 {
            departure.advance(1.0 / 60.0, (1.0, 0.0), true, false);
            assert!(departure.crash.is_none());
        }
        let simulate = |name, fps| {
            let mut flight = space.new_flight();
            space.approach(&mut flight, name).unwrap();
            assert!(flight.toggle_local_navigation());
            let reference = flight.navigation_reference().unwrap();
            let altitude = reference.altitude(flight.position, flight.seconds);
            for _ in 0..fps * 40 {
                flight.advance(1.0 / fps as f32, (1.0, 0.0), false, false);
                assert!(flight.crash.is_none());
                assert!(
                    (reference.altitude(flight.position, flight.seconds) - altitude).abs() < 0.1
                );
            }
            flight.position
        };
        for name in ["Nivalis", "Onice", "Vespera", "Prisma"] {
            assert!((simulate(name, 10) - simulate(name, 60)).norm() < 0.5);
        }
    }

    #[test]
    fn jovara_lava_and_sun_surfaces_move_without_orbital_or_camera_motion() {
        let space = Space::load().unwrap();
        for name in ["Jovara", "Flareon", "Helios II"] {
            let planet = space.planets.iter().find(|p| p.name == name).unwrap();
            let mut flight = Flight::new();
            flight.camera.pitch = 0.0;
            let transform = WorldTransform {
                position: Vec3::new(0.0, 0.0, -600.0),
                radius: 70.0,
                yaw: 0.0,
            };
            let view = FlightView::new(&flight, 256, 192);
            let render = |seconds| {
                let mut frame = Framebuffer::new(256, 192);
                frame.point(128, 96, 2.0, 0xff00ff, true);
                renderer::draw_world_model_shaded(
                    &mut frame,
                    &planet.model,
                    &view,
                    transform,
                    None,
                    planet.surface_effect(seconds),
                );
                frame
            };
            let initial = render(0.0);
            let animated = render(2.0);
            assert_eq!(initial.depth, animated.depth);
            let mut count = 0;
            let mut difference = 0u64;
            for ((&a, &b), &depth) in initial
                .pixels
                .iter()
                .zip(&animated.pixels)
                .zip(&initial.depth)
            {
                if !depth.is_finite() {
                    continue;
                }
                count += 1;
                difference += [0, 8, 16]
                    .into_iter()
                    .map(|shift| {
                        ((a >> shift & 255) as i32 - (b >> shift & 255) as i32).unsigned_abs()
                            as u64
                    })
                    .sum::<u64>();
            }
            assert!(count > 1000);
            assert!(
                difference as f32 / (count * 3) as f32 > 5.0,
                "{name} needs visible surface motion"
            );
            assert_eq!(animated.pixels[96 * 256 + 128], 0xff00ff);
        }
    }

    #[test]
    fn kepler_surface_animates_with_fixed_geometry_and_respects_depth() {
        let mut space = Space::load().unwrap();
        space.configure(1);
        let planet = &space.planets[6];
        let transform = planet.world_transform(0.0);
        assert_eq!(transform.radius, crate::galaxy::KEPLER_RADIUS);
        assert!(
            crate::orbit::METIS.radius - transform.radius - space.planets[7].transform.radius
                > 80.0
        );
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        flight.position = transform.position + Vec3::new(0.0, -1.6, transform.radius * 8.0 - 8.0);
        let view = FlightView::new(&flight, 256, 192);
        let render = |seconds, threads| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let mut frame = Framebuffer::new(256, 192);
            frame.point(128, 96, 2.0, 0xff00ff, true);
            pool.install(|| {
                renderer::draw_world_model_shaded(
                    &mut frame,
                    &planet.model,
                    &view,
                    transform,
                    space.sun_position(0.0),
                    planet.surface_effect(seconds),
                )
            });
            frame
        };
        let initial = render(0.0, 1);
        let parallel = render(0.0, 4);
        let animated = render(4.0, 4);
        let mut reference = Framebuffer::new(256, 192);
        reference.point(128, 96, 2.0, 0xff00ff, true);
        renderer::draw_world_model_lit(
            &mut reference,
            &planet.model,
            &view,
            transform,
            space.sun_position(0.0),
        );
        assert_eq!(initial.depth, reference.depth);
        assert_eq!(initial.pixels, parallel.pixels);
        assert_eq!(initial.depth, animated.depth);
        let changed = initial
            .pixels
            .iter()
            .zip(&animated.pixels)
            .filter(|(a, b)| a != b)
            .count();
        assert!(changed > 1000);
        assert_eq!(animated.pixels[96 * 256 + 128], 0xff00ff);
    }
    #[test]
    fn aurora_draws_and_collides_at_the_same_orbital_positions() {
        let mut space = Space::load().unwrap();
        space.configure(1);
        assert_eq!(space.active_planets().count(), 6);
        assert_eq!(space.sun_position(600.0), Some(Vec3::zeros()));
        let hole = space.new_flight().black_hole;
        for seconds in [0.0, 120.0, 600.0] {
            for body in space.active_planets() {
                let transform = body.world_transform(seconds);
                assert_eq!(hole.acceleration(transform.position), Vec3::zeros());
                let start = transform.position + Vec3::z() * (transform.radius + 15.0);
                let motion = space
                    .collisions
                    .move_ship(start, -Vec3::z() * 35.0, seconds, 0.0);
                assert!(
                    motion
                        .contacts
                        .iter()
                        .any(|contact| contact.planet == body.name),
                    "Colision ausente en {} a t={seconds}",
                    body.name
                );
            }
            let kepler = space.planets[6].world_transform(seconds);
            let metis = space.planets[7].world_transform(seconds);
            assert!(
                ((metis.position - kepler.position).norm() - crate::orbit::METIS.radius).abs()
                    < 0.001
            );
        }
        assert!(
            space
                .collisions
                .move_ship(crate::flight::SPAWN_POSITION, Vec3::zeros(), 0.0, 0.0)
                .contacts
                .is_empty()
        );
        for body in &space.planets[4..8] {
            assert!(body.model.faces.len() < 20000);
            assert_eq!(body.model.textures.len(), 1);
            assert!(
                body.model
                    .faces
                    .iter()
                    .all(|face| face.texture.is_some() && face.uv.is_some())
            );
        }
        assert!(space.planets[4].model.faces.iter().all(|face| face.unlit));
        let mut flight = space.new_flight();
        flight.position = hole.position + Vec3::z() * 75.0;
        for _ in 0..600 {
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
            if flight.respawn_notice > 0.0 {
                break;
            }
        }
        assert!(flight.respawn_notice > 0.0);
        assert_eq!(flight.position, crate::flight::SPAWN_POSITION);
        assert_eq!(flight.black_hole.position, hole.position);
    }

    #[test]
    fn an_orbiting_planet_can_contact_a_stationary_ship() {
        let mut space = Space::load().unwrap();
        space.configure(1);
        let body = &space.planets[6];
        let seconds = 120.0;
        let transform = body.world_transform(seconds);
        let direction = body.motion.velocity(seconds).normalize();
        let start = transform.position
            + direction * (transform.radius + crate::collision::SHIP_RADIUS + 0.05);
        let result = space
            .collisions
            .move_ship(start, Vec3::zeros(), seconds, 0.1);
        assert!(
            result
                .contacts
                .iter()
                .any(|contact| contact.planet == "Kepler-186f")
        );
        assert!((result.position - start).norm() > 0.01);
    }
    #[test]
    fn planet_shadows_follow_orbits_and_surface_noise_excludes_siroco() {
        use crate::shaders::SurfaceEffect;
        let mut space = Space::load().unwrap();
        for planet in &space.planets {
            assert_eq!(
                matches!(planet.surface_effect(4.0), SurfaceEffect::None),
                planet.name == "Siroco",
                "{}",
                planet.name
            );
        }
        space.configure(1);
        for time in [0.0, 60.0, 120.0] {
            let light = space.solar_lighting(time).unwrap();
            let kepler = space.planets[6].world_transform(time);
            assert!(light.occluders.iter().any(|p| p.center == kepler.position));
            let outward = (kepler.position - light.position).normalize();
            assert_eq!(
                light.visibility(kepler.position + outward * (kepler.radius + 30.0)),
                0.0
            );
        }
        space.configure(0);
        assert!(space.solar_lighting(0.0).is_none());
    }
    #[test]
    fn destinations_restore_layouts_reproduce_debris_and_keep_active_collisions() {
        let mut space = Space::load().unwrap();
        for galaxy in [1, 2, 0, 2, 1, 0] {
            space.configure(galaxy);
            let definition = &crate::galaxy::GALAXIES[galaxy];
            assert_eq!(
                space.planets[3].name,
                if galaxy == 2 { "Nivalis" } else { "Boreal" }
            );
            assert!(Arc::ptr_eq(
                &space.planets[3].model,
                if galaxy == 2 {
                    &space.nivalis
                } else {
                    &space.boreal
                }
            ));
            assert_eq!(space.active_planets().count(), definition.planets.len());
            let mut flight = space.new_flight();
            assert_eq!(flight.position, crate::flight::SPAWN_POSITION);
            flight.advance(1.0 / 60.0, (0.0, 0.0), false, false);
            assert_eq!(flight.position, crate::flight::SPAWN_POSITION);
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
        assert_eq!(fast.position, crate::flight::SPAWN_POSITION);
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
