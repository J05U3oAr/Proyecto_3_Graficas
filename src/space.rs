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
            "Aureon" => SurfaceEffect::GoldenStar { seconds },
            "Nivalis" => SurfaceEffect::Ice {
                seconds,
                light: Vec3::new(-400.0, 800.0, 1000.0),
                eye: Vec3::z() * 3.0,
                normal: Vec3::y(),
                detail: true,
                core_radius: 0.96 / self.model.radius,
            },
            "Asterion" => SurfaceEffect::Stellar {
                seconds,
                eye: Vec3::z() * 3.0,
                pixel_radius: 256.0,
            },
            "Onice" => SurfaceEffect::Obsidian,
            "Vespera" => SurfaceEffect::Snow { seconds },
            "Prisma" => SurfaceEffect::Crystal,
            "Nacar" => SurfaceEffect::PackIce { seconds },
            "Umbriel" => SurfaceEffect::GasGiant {
                seconds,
                light: Vec3::new(-0.4, 0.8, 1.0).normalize(),
                eye: Vec3::z(),
            },
            "Kepler-186f" => SurfaceEffect::Kepler { seconds },
            "Jovara" => SurfaceEffect::PolarClouds { seconds },
            "Helios II" => SurfaceEffect::Solar { seconds },
            "Boreal" => SurfaceEffect::Clouds {
                seconds,
                seed: 37.0,
                flow: 0.018,
            },
            "Flareon" => SurfaceEffect::Lava { seconds },
            "Volcarona" => SurfaceEffect::CrimsonLava { seconds },
            "Metis" => SurfaceEffect::Rock {
                seconds,
                seed: 19.0,
            },
            "Oquedad" => SurfaceEffect::Rock {
                seconds,
                seed: 43.0,
            },
            // Rock detail stays anchored to the sculpted surface while the body rotates.
            "Micelia" | "Azufria" | "Ferrita" => SurfaceEffect::Rock {
                seconds: 0.0,
                seed: match self.name {
                    "Micelia" => 61.0,
                    "Azufria" => 79.0,
                    _ => 97.0,
                },
            },
            _ => SurfaceEffect::None,
        }
    }
    fn navigation_surface_radius(&self) -> f32 {
        match self.name {
            // These Blender meshes have unit-radius ground and separate raised terrain.
            "Nivalis" | "Onice" | "Vespera" | "Prisma" | "Nacar" | "Micelia" | "Azufria"
            | "Ferrita" | "Volcarona" => self.transform.radius / self.model.radius,
            _ => self.transform.radius,
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
                "nacar/nacar.obj",
                "umbriel/umbriel.obj",
                "umbriel/anillos.obj",
                "helix/micelia/micelia.obj",
                "helix/azufria/azufria.obj",
                "helix/ferrita/ferrita.obj",
                "aureon/aureon.obj",
                "volcarona/volcarona.obj",
                "volcarona/volcarona.obj",
            ];
            paths
                .iter()
                .enumerate()
                .map(|(index, path)| {
                    let mut model = if index >= 20 {
                        crate::volcarona::load(
                            assets.join(path),
                            if index == 20 {
                                crate::volcarona::SUBDIVISIONS
                            } else {
                                0
                            },
                        )?
                    } else if index == 8 {
                        crate::ice_planet::load(assets.join(path))?
                    } else {
                        Model::load(assets.join(path))?
                    };
                    if index >= 10 {
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
        "Las capas de opacidad usan materiales sin texturas"
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

fn ring_transform(planet: &Planet, model: &Model, seconds: f32) -> WorldTransform {
    let mut transform = planet.world_transform(seconds);
    transform.yaw = planet.transform.yaw;
    transform.radius *= model.radius / planet.model.radius;
    transform
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
    debris_models: &'static [crate::asteroids::Meshes],
    debris: Vec<crate::asteroids::Instance>,
    debris_motion: Arc<crate::debris_physics::Field>,
    nivalis: Arc<Model>,
    boreal: Arc<Model>,
    prisma_opaque: Model,
    prisma_glass: Model,
    umbriel_rings: Arc<Model>,
    ring_fragments: &'static [crate::ring_debris::Fragment],
    ring_motion: Option<Arc<crate::debris_physics::Field>>,
    volcarona_distant: Arc<Model>,
    umbriel_rings_glass: Model,
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
            Planet {
                active: false,
                name: "Nacar",
                model: Arc::clone(&models[13]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(2, 12),
                rotation_speed: 0.008,
                dust: None,
            },
            Planet {
                active: false,
                name: "Umbriel",
                model: Arc::clone(&models[14]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(2, 13),
                rotation_speed: 0.016,
                dust: None,
            },
            Planet {
                active: true,
                name: "Micelia",
                model: Arc::clone(&models[16]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(0, 14),
                rotation_speed: 0.009,
                dust: None,
            },
            Planet {
                active: true,
                name: "Azufria",
                model: Arc::clone(&models[17]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(0, 15),
                rotation_speed: 0.012,
                dust: None,
            },
            Planet {
                active: true,
                name: "Ferrita",
                model: Arc::clone(&models[18]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(0, 16),
                rotation_speed: 0.01,
                dust: None,
            },
            Planet {
                active: true,
                name: "Aureon",
                model: Arc::clone(&models[19]),
                motion: crate::orbit::Motion::default(),
                is_sun: true,
                transform: crate::galaxy::planet_transform(0, 17),
                rotation_speed: 0.014,
                dust: None,
            },
            Planet {
                active: false,
                name: "Volcarona",
                model: Arc::clone(&models[20]),
                motion: crate::orbit::Motion::default(),
                is_sun: false,
                transform: crate::galaxy::planet_transform(1, 18),
                rotation_speed: 0.025,
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
        let mut space = Self {
            planets,
            collisions,
            black_hole: crate::galaxy::black_hole(0),
            debris_models: crate::asteroids::load()?,
            debris: Vec::new(),
            debris_motion: Arc::new(crate::debris_physics::Field::new(
                crate::galaxy::black_hole(0),
                Vec::new(),
            )),
            nivalis: Arc::clone(&models[8]),
            boreal: Arc::clone(&models[3]),
            volcarona_distant: Arc::clone(&models[21]),
            prisma_opaque: opacity_layer(&models[12], false),
            prisma_glass: opacity_layer(&models[12], true),
            umbriel_rings: Arc::clone(&models[15]),
            ring_fragments: crate::ring_debris::load()?,
            ring_motion: None,
            umbriel_rings_glass: opacity_layer(&models[15], true),
        };
        space.configure(0);
        Ok(space)
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
                crate::orbit::helix_motion(planet_index)
            };
        }
        let mut colliders: Vec<_> = self
            .active_planets()
            .map(|p| {
                PlanetCollider::new(p.name, &p.model, p.transform, p.rotation_speed)
                    .with_motion(p.motion)
            })
            .collect();
        self.ring_motion = None;
        let ring_planet = self
            .active_planets()
            .find(|p| p.name == "Umbriel")
            .map(|p| (p.transform, p.motion, p.model.radius));
        if let Some((planet, motion, body_radius)) = ring_planet {
            let transforms: Vec<_> = self
                .ring_fragments
                .iter()
                .map(|f| f.transform(planet, body_radius))
                .collect();
            let bodies = transforms
                .iter()
                .map(|t| crate::debris_physics::Body {
                    position: t.position,
                    radius: t.radius,
                    velocity: Vec3::zeros(),
                })
                .collect();
            let field = Arc::new(
                crate::debris_physics::Field::new(self.black_hole, bodies).with_carrier(motion),
            );
            colliders.extend(self.ring_fragments.iter().zip(&transforms).enumerate().map(
                |(index, (fragment, transform))| {
                    fragment
                        .collider
                        .instance(*transform, 0.0)
                        .with_debris(Arc::clone(&field), index)
                },
            ));
            self.ring_motion = Some(field);
        }
        self.debris = crate::asteroids::distribute(index, |rock| {
            self.active_planets().all(|planet| {
                let radius = if planet.name == "Umbriel" {
                    275.0
                } else {
                    planet.transform.radius
                };
                let margin = if planet.is_sun { 100.0 } else { 20.0 };
                if let Some(orbit) = planet.motion.primary {
                    let center = planet.transform.position - planet.motion.offset(0.0);
                    let satellite = planet.motion.satellite.map_or(0.0, |o| o.radius);
                    ((rock.position - center).norm() - orbit.radius).abs()
                        > radius + rock.radius + margin + satellite
                } else {
                    (rock.position - planet.transform.position).norm()
                        > radius + rock.radius + margin
                }
            })
        });
        self.debris_motion = Arc::new(crate::debris_physics::Field::new(
            self.black_hole,
            self.debris
                .iter()
                .map(|rock| crate::debris_physics::Body {
                    position: rock.transform.position,
                    velocity: rock.velocity,
                    radius: rock.transform.radius,
                })
                .collect(),
        ));
        colliders.extend(self.debris.iter().enumerate().map(|(index, rock)| {
            self.debris_models[rock.variant]
                .collision
                .instance(rock.transform, rock.rotation_speed())
                .with_debris(Arc::clone(&self.debris_motion), index)
        }));
        self.collisions = Arc::new(CollisionWorld::new(colliders));
    }

    pub fn active_planets(&self) -> impl Iterator<Item = &Planet> {
        self.planets.iter().filter(|planet| planet.active)
    }

    fn debris_at(&self, seconds: f32) -> Vec<crate::asteroids::Instance> {
        let states = self.debris_motion.sample(seconds);
        self.debris
            .iter()
            .zip(states.iter())
            .filter_map(|(rock, state)| {
                if state.captured {
                    return None;
                }
                let mut instance = *rock;
                instance.transform.position = state.position;
                instance.velocity = state.velocity;
                Some(instance)
            })
            .collect()
    }

    pub fn debris_captured(&self, seconds: f32) -> usize {
        let free = self
            .debris_motion
            .sample(seconds)
            .iter()
            .filter(|state| state.captured)
            .count();
        free + self.ring_motion.as_ref().map_or(0, |field| {
            field.sample(seconds).iter().filter(|s| s.captured).count()
        })
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
                    surface_radius: planet.navigation_surface_radius(),
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
            let model = if planet.name == "Volcarona"
                && fb.render_options.automatic_lod
                && transform.radius * renderer::FLIGHT_FOCAL
                    / (view.eye() - transform.position).norm().max(0.1)
                    < 60.0
            {
                &self.volcarona_distant
            } else if planet.name == "Prisma" {
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
            if planet.name == "Umbriel" {
                self.draw_rings(fb, view, planet, seconds, false);
            }
        }
        for sun in self.active_planets().filter(|planet| planet.is_sun) {
            if sun.name == "Asterion" {
                crate::stellar::draw_corona(fb, view, sun.world_transform(seconds), seconds);
            } else {
                draw_solar_halo(fb, &view, sun.world_transform(seconds), sun.halo_color());
            }
        }
        crate::asteroids::draw(
            fb,
            view,
            self.debris_models,
            &self.debris_at(seconds),
            self.sun_position(seconds),
            seconds,
        );
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
        let framing_radius = if planet.name == "Umbriel" {
            ring_transform(planet, &self.umbriel_rings, seconds).radius
        } else {
            planet.transform.radius
        };
        let distance =
            framing_radius * (1.0 + (renderer::FLIGHT_FOCAL / target_radius).powi(2)).sqrt();
        flight.position = transform.position
            + Vec3::new(
                0.0,
                -crate::flight::CHASE_HEIGHT,
                distance - crate::flight::CHASE_DISTANCE,
            );
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
        if planet.name == "Asterion" {
            crate::stellar::draw_corona(fb, &view, transform, seconds);
        } else if planet.is_sun {
            draw_solar_halo(fb, &view, transform, planet.halo_color());
        }
        if planet.name == "Umbriel" {
            self.draw_rings(fb, &view, planet, seconds, false);
            self.draw_rings(fb, &view, planet, seconds, true);
        }
        Ok(())
    }

    fn draw_rings(
        &self,
        fb: &mut Framebuffer,
        view: &FlightView,
        planet: &Planet,
        seconds: f32,
        glass: bool,
    ) {
        if !glass {
            if let Some(field) = &self.ring_motion {
                let states = field.sample(seconds);
                for (fragment, state) in self.ring_fragments.iter().zip(states.iter()) {
                    if state.captured {
                        continue;
                    }
                    let mut transform = fragment.transform(planet.transform, planet.model.radius);
                    transform.position = state.position;
                    renderer::draw_world_model_shaded(
                        fb,
                        &fragment.model,
                        view,
                        transform,
                        self.sun_position(seconds),
                        crate::shaders::SurfaceEffect::Asteroid,
                    );
                }
            }
            return;
        }
        let model = &self.umbriel_rings_glass;
        renderer::draw_world_model_shaded(
            fb,
            model,
            view,
            ring_transform(planet, model, seconds),
            self.sun_position(seconds),
            crate::shaders::SurfaceEffect::RingIce,
        );
    }

    pub fn draw_atmospheres(&self, fb: &mut Framebuffer, flight: &Flight, seconds: f32) {
        let view = FlightView::new(flight, fb.width, fb.height);
        self.draw_atmospheres_view(fb, &view, seconds);
    }

    pub fn draw_atmospheres_view(&self, fb: &mut Framebuffer, view: &FlightView, seconds: f32) {
        for planet in self.active_planets() {
            if planet.name == "Umbriel" {
                self.draw_rings(fb, view, planet, seconds, true);
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn volcarona_is_an_independent_orbital_world_in_aurora_with_real_volcanoes() {
        let mut space = Space::load().unwrap();
        space.configure(1);
        let body = &space.planets[18];
        assert_eq!(body.name, "Volcarona");
        assert!(body.active && !body.is_sun);
        assert!(!Arc::ptr_eq(&body.model, &space.planets[5].model));
        assert!(body.model.faces.len() > space.planets[5].model.faces.len() * 4);
        assert!(body.model.faces.iter().any(|f| f.unlit));
        assert!(
            (body.world_transform(30.0).position - body.world_transform(0.0).position).norm()
                > 100.0
        );
        assert!(body.world_transform(30.0).yaw > body.world_transform(0.0).yaw);
        for galaxy in [0, 2] {
            space.configure(galaxy);
            assert!(!space.planets[18].active);
        }
        let kepler = &space.planets[6].model;
        let warm = (1..20)
            .flat_map(|x| {
                (1..20).map(move |y| nalgebra_glm::Vec2::new(x as f32 / 20.0, y as f32 / 20.0))
            })
            .filter(|&uv| {
                let c = kepler.textures[0].sample(uv);
                c.r >= c.g && c.g >= c.b
            })
            .count();
        assert!(warm > 340);
    }
    #[test]
    fn planetary_gravity_uses_the_current_galaxy_and_tracks_its_moving_worlds() {
        let mut space = Space::load().unwrap();
        for galaxy in 0..3 {
            space.configure(galaxy);
            let mut flight = space.new_flight();
            for planet in space.active_planets() {
                for time in [0.0, 40.0, 120.0] {
                    let transform = planet.world_transform(time);
                    flight.seconds = time;
                    flight.position = transform.position
                        + Vec3::y() * (planet.navigation_surface_radius() + 15.0);
                    let pull = flight.planetary_acceleration();
                    assert!(
                        pull.y < -0.05,
                        "{} en galaxia {galaxy}, t={time}: {pull:?}",
                        planet.name
                    );
                    assert!(pull.norm() <= crate::planet_gravity::MAX_ACCELERATION + 0.0001);
                }
            }
        }
    }

    #[test]
    fn all_asteroid_variants_stop_slow_ships_and_destroy_fast_ships_in_every_galaxy() {
        let mut space = Space::load().unwrap();
        for galaxy in 0..3 {
            space.configure(galaxy);
            for variant in 0..6 {
                let (rock_index, &rock) = space
                    .debris
                    .iter()
                    .enumerate()
                    .find(|(_, rock)| rock.variant == variant)
                    .unwrap();
                for time in [0.0, 45.0] {
                    let state = space.debris_motion.sample(time)[rock_index];
                    assert!(!state.captured);
                    let reach = rock.transform.radius + crate::collision::SHIP_RADIUS + 4.0;
                    let probe = space.collisions.move_ship(
                        state.position + Vec3::z() * reach,
                        -Vec3::z() * reach * 2.0,
                        time,
                        0.0,
                    );
                    let contact = probe
                        .contacts
                        .first()
                        .expect("Falta colision contra el asteroide");
                    assert!(contact.planet.starts_with("Asteroide"));
                    // A swept movement can slide beyond the initial hit on an irregular rock.
                    // Start instead at its rendered +Z support vertex, outside the ship sphere.
                    let model = &space.debris_models[variant].detailed;
                    let normal = Vec3::z();
                    let start_at = |seconds: f32| {
                        let (sin, cos) =
                            (rock.transform.yaw + seconds * rock.rotation_speed()).sin_cos();
                        model
                            .vertices
                            .iter()
                            .map(|vertex| {
                                let p = (vertex - model.center)
                                    * (rock.transform.radius / model.radius);
                                space.debris_motion.sample(seconds)[rock_index].position
                                    + Vec3::new(p.x * cos + p.z * sin, p.y, p.z * cos - p.x * sin)
                            })
                            .max_by(|a, b| a.z.total_cmp(&b.z))
                            .unwrap()
                            + normal * (crate::collision::SHIP_RADIUS + 0.05)
                    };
                    let start = start_at(time);
                    let mut slow = space.new_flight();
                    slow.seconds = time;
                    slow.position = start;
                    slow.velocity = state.velocity - normal * 10.0;
                    slow.advance(1.0 / 30.0, (0.0, 0.0), false, false);
                    assert!(slow.crash.is_none());
                    assert!(slow.contact_notice > 0.0);
                    assert!((slow.velocity - state.velocity).dot(&normal) > -1.0);
                    for fps in [10, 60] {
                        let mut fast = space.new_flight();
                        fast.seconds = time;
                        fast.position = start;
                        fast.velocity = state.velocity - normal * 62.0;
                        fast.advance(1.0 / fps as f32, (0.0, 0.0), true, false);
                        let crash = fast
                            .crash
                            .as_ref()
                            .expect("El impacto rapido debe destruir la nave");
                        assert!(crash.planet.starts_with("Asteroide"));
                        assert_eq!(fast.velocity, Vec3::zeros());
                        if galaxy == 2 && variant == 4 && time == 45.0 && fps == 60 {
                            for _ in 0..125 {
                                fast.advance(1.0 / 60.0, (0.0, 0.0), false, false);
                                if fast.respawn_notice > 0.0 {
                                    break;
                                }
                            }
                            assert!(fast.crash.is_none() && fast.respawn_notice > 0.0);
                            assert_eq!(fast.position, crate::flight::SPAWN_POSITION);
                            fast.position = start_at(fast.seconds);
                            fast.velocity = space.debris_motion.sample(fast.seconds)[rock_index]
                                .velocity
                                - normal * 62.0;
                            fast.advance(1.0 / 60.0, (0.0, 0.0), true, false);
                            assert!(
                                fast.crash.as_ref().unwrap().planet.starts_with("Asteroide"),
                                "La reaparicion debe conservar las colisiones de asteroides"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn umbria_has_a_dense_belt_of_blender_rocks_separate_from_its_scattered_field() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let center = crate::galaxy::UMBRIA_CENTER;
        let belt: Vec<_> = space
            .debris
            .iter()
            .filter(|rock| {
                let p = rock.transform.position - center;
                let radial = (p.x * p.x + p.z * p.z).sqrt();
                (735.0..775.0).contains(&radial) && p.y.abs() <= 45.0
            })
            .collect();
        assert!(
            belt.len() >= 320,
            "El cinturon debe tener al menos 320 modelos, no depender del reparto disperso: {}",
            belt.len()
        );
        assert!(space.debris.len() - belt.len() >= 120);
        for variant in 0..6 {
            assert!(belt.iter().any(|rock| rock.variant == variant));
        }
        let mut sectors = [0; 32];
        for rock in belt {
            let p = rock.transform.position - center;
            let angle = p.z.atan2(p.x).rem_euclid(std::f32::consts::TAU);
            sectors[(angle / std::f32::consts::TAU * 32.0) as usize % 32] += 1;
        }
        assert!(sectors.into_iter().all(|count| count >= 8));
        let flight = space.new_flight();
        let eye = center + Vec3::new(754.0, 44.0, 50.0);
        let target = center + Vec3::new(730.0, 0.0, -250.0);
        let view = FlightView::new(&flight, 1280, 840).looking_at(eye, target);
        let mut frame = Framebuffer::new(1280, 840);
        frame.pixels.fill(0);
        crate::asteroids::draw(
            &mut frame,
            &view,
            space.debris_models,
            &space.debris,
            space.sun_position(0.0),
            0.0,
        );
        assert!(
            frame.pixels.iter().filter(|&&p| p != 0).count() > 5000,
            "El cinturon debe dibujar roca visible al recorrer su interior"
        );
    }

    #[test]
    fn asteroids_populate_all_galaxies_without_blocking_spawns_or_orbital_paths() {
        let mut space = Space::load().unwrap();
        assert_eq!(space.debris.len(), crate::galaxy::GALAXIES[0].debris);
        for galaxy in 0..3 {
            space.configure(galaxy);
            assert_eq!(space.debris.len(), crate::galaxy::GALAXIES[galaxy].debris);
            for variant in 0..6 {
                assert!(space.debris.iter().any(|rock| rock.variant == variant));
            }
            for rock in &space.debris {
                let t = rock.transform;
                assert!((t.position - crate::flight::SPAWN_POSITION).norm() > t.radius + 110.0);
                for seconds in [0.0, 100.0, 400.0, 1000.0] {
                    for planet in space.active_planets() {
                        let planet_t = planet.world_transform(seconds);
                        let radius = if planet.name == "Umbriel" {
                            275.0
                        } else {
                            planet_t.radius
                        };
                        assert!((t.position - planet_t.position).norm() > radius + t.radius + 19.0);
                    }
                }
            }
            let mut frame = Framebuffer::new(640, 480);
            frame.pixels.fill(0);
            let flight = space.new_flight();
            let view = FlightView::new(&flight, frame.width, frame.height);
            crate::asteroids::draw(
                &mut frame,
                &view,
                space.debris_models,
                &space.debris,
                space.sun_position(0.0),
                0.0,
            );
            assert!(
                frame.pixels.iter().filter(|&&p| p != 0).count() > 30,
                "Debe haber asteroides visibles desde el despegue en cada galaxia"
            );
        }
    }

    #[test]
    fn umbriel_ring_gaps_are_open_but_ice_fragments_collide_and_follow_the_orbit() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let planet = space
            .active_planets()
            .find(|p| p.name == "Umbriel")
            .unwrap();
        let model = &space.umbriel_rings;
        let scale = planet.transform.radius / planet.model.radius;
        assert!(model.faces.iter().any(|f| f.opacity < 1.0));
        assert!(model.faces.iter().any(|f| f.opacity == 1.0));
        let transform = WorldTransform {
            position: Vec3::zeros(),
            radius: model.radius * scale,
            yaw: 0.0,
        };
        assert!(transform.radius < 275.0);
        let fragments = CollisionWorld::new(vec![PlanetCollider::new_opaque(
            "Hielo", model, transform, 0.0,
        )]);
        let all = CollisionWorld::new(vec![PlanetCollider::new(
            "Banda visual",
            model,
            transform,
            0.0,
        )]);
        let tilt = 27_f32.to_radians();
        let normal = Vec3::new(0.0, tilt.cos(), tilt.sin());
        let gap = Vec3::x() * (1.56 * scale);
        assert!(
            fragments
                .move_ship(gap + normal * 12.0, -normal * 24.0, 0.0, 0.1)
                .contacts
                .is_empty()
        );
        let can_cross_band = (0..24).any(|i| {
            let angle = i as f32 * std::f32::consts::TAU / 24.0;
            let p = Vec3::new(
                angle.cos(),
                -angle.sin() * tilt.sin(),
                angle.sin() * tilt.cos(),
            ) * (1.45 * scale);
            let start = p + normal * 12.0;
            fragments
                .move_ship(start, -normal * 24.0, 0.0, 0.1)
                .contacts
                .is_empty()
                && !all
                    .move_ship(start, -normal * 24.0, 0.0, 0.1)
                    .contacts
                    .is_empty()
        });
        assert!(
            can_cross_band,
            "Las bandas transparentes no deben actuar como paredes invisibles"
        );
        let face = model.faces.iter().find(|f| f.opacity == 1.0).unwrap();
        let [a, b, c] = face.indices.map(|i| model.vertices[i] * scale);
        let center = (a + b + c) / 3.0;
        let outward = (b - a).cross(&(c - a)).normalize();
        for seconds in [0.0, 80.0] {
            let position = planet.world_transform(seconds).position;
            let fragment = &space.ring_fragments[0];
            let actual = space.ring_motion.as_ref().unwrap().sample(seconds)[0].position;
            let drift = actual - (position + fragment.model.center * scale);
            let hit = space.collisions.move_ship(
                position + drift + center + outward * (crate::collision::SHIP_RADIUS + 1.0),
                -outward * 2.0,
                seconds,
                0.01,
            );
            assert!(
                hit.contacts
                    .iter()
                    .any(|c| c.planet == "Anillos de Umbriel"),
                "El contacto debe seguir la roca y su deriva gravitatoria en t={seconds}"
            );
        }
    }

    #[test]
    fn umbriel_storms_animate_without_changing_geometry_or_foreground_depth() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let planet = space
            .active_planets()
            .find(|p| p.name == "Umbriel")
            .unwrap();
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        let view = FlightView::new(&flight, 320, 240);
        let transform = WorldTransform {
            position: Vec3::new(0.0, 0.0, -1000.0),
            radius: 155.0,
            yaw: 0.0,
        };
        let render = |seconds| {
            let mut frame = Framebuffer::new(320, 240);
            frame.point(160, 120, 2.0, 0xff00ff, true);
            renderer::draw_world_model_shaded(
                &mut frame,
                &planet.model,
                &view,
                transform,
                Some(Vec3::new(0.0, 300.0, 500.0)),
                planet.surface_effect(seconds),
            );
            frame
        };
        let before = render(0.0);
        let after = render(4.0);
        assert_eq!(before.depth, after.depth);
        assert_eq!(after.pixels[120 * 320 + 160], 0xff00ff);
        assert!(
            before
                .pixels
                .iter()
                .zip(&after.pixels)
                .filter(|(a, b)| a != b)
                .count()
                > 500
        );
    }

    #[test]
    fn umbriel_day_and_night_follow_the_sun_when_the_body_rotates() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let planet = space
            .active_planets()
            .find(|p| p.name == "Umbriel")
            .unwrap();
        let mut flight = Flight::new();
        flight.camera.pitch = 0.0;
        let view = FlightView::new(&flight, 320, 240);
        for yaw in [0.0, 1.2] {
            let render = |sun| {
                let mut frame = Framebuffer::new(320, 240);
                renderer::draw_world_model_shaded(
                    &mut frame,
                    &planet.model,
                    &view,
                    WorldTransform {
                        position: Vec3::new(0.0, 0.0, -1000.0),
                        radius: 155.0,
                        yaw,
                    },
                    Some(sun),
                    planet.surface_effect(0.0),
                );
                frame
                    .pixels
                    .iter()
                    .zip(&frame.depth)
                    .filter(|(_, depth)| depth.is_finite())
                    .map(|(&p, _)| {
                        ((p >> 16) & 255) as u64 + ((p >> 8) & 255) as u64 + (p & 255) as u64
                    })
                    .sum::<u64>()
            };
            let day = render(Vec3::new(0.0, 0.0, 500.0));
            let night = render(Vec3::new(0.0, 0.0, -2500.0));
            assert!(
                day > night * 4,
                "La rotacion del material no debe mover la iluminacion solar"
            );
        }
    }

    #[test]
    fn umbriel_is_a_large_orbiting_giant_without_affecting_other_galaxies() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let planet = space
            .active_planets()
            .find(|p| p.name == "Umbriel")
            .expect("Falta Umbriel");
        assert!(planet.transform.radius >= 150.0);
        assert!(
            planet.model.radius < 1.01,
            "Los anillos no deben reducir el tamano del cuerpo"
        );
        assert!(planet.model.faces.len() < 12000);
        assert!(
            planet
                .model
                .faces
                .iter()
                .all(|f| f.opacity == 1.0 && !f.unlit)
        );
        for seconds in [0.0, 40.0, 190.0, 600.0] {
            let transform = planet.world_transform(seconds);
            assert!(
                ((transform.position - space.sun_position(seconds).unwrap()).norm() - 2150.0).abs()
                    < 0.003
            );
            let hit = space.collisions.move_ship(
                transform.position + Vec3::z() * 185.0,
                -Vec3::z() * 65.0,
                seconds,
                0.01,
            );
            assert!(hit.contacts.iter().any(|c| c.planet == "Umbriel"));
        }
        space.configure(0);
        assert!(!space.active_planets().any(|p| p.name == "Umbriel"));
        space.configure(1);
        assert!(!space.active_planets().any(|p| p.name == "Umbriel"));
    }

    #[test]
    fn nacar_has_fractured_ice_raised_icebergs_and_collisions_in_its_own_orbit() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let planet = space
            .active_planets()
            .find(|p| p.name == "Nacar")
            .expect("Falta Nacar");
        assert!(planet.model.faces.len() < 25000);
        assert!(
            planet
                .model
                .faces
                .iter()
                .all(|f| f.opacity == 1.0 && !f.unlit)
        );
        assert!(
            planet
                .model
                .faces
                .iter()
                .filter(|f| f.color.r < 25 && f.color.b > 40)
                .count()
                > 500
        );
        assert!(
            planet
                .model
                .faces
                .iter()
                .filter(|f| f.color.r > 160 && f.color.b > 210)
                .count()
                > 10000
        );
        assert!(planet.model.vertices.iter().any(|p| p.norm() < 0.97));
        assert!(planet.model.vertices.iter().any(|p| p.norm() > 1.15));
        for seconds in [0.0, 40.0, 190.0, 600.0] {
            let transform = planet.world_transform(seconds);
            assert!(
                ((transform.position - space.sun_position(seconds).unwrap()).norm() - 1690.0).abs()
                    < 0.003
            );
            let hit = space.collisions.move_ship(
                transform.position + Vec3::z() * 125.0,
                -Vec3::z() * 105.0,
                seconds,
                0.01,
            );
            assert!(hit.contacts.iter().any(|c| c.planet == "Nacar"));
        }
        let world = CollisionWorld::new(vec![PlanetCollider::new(
            "Nacar",
            &planet.model,
            WorldTransform {
                position: Vec3::zeros(),
                radius: 96.0,
                yaw: 0.0,
            },
            0.0,
        )]);
        let scale = 96.0 / planet.model.radius;
        let submerged_channel = planet
            .model
            .faces
            .iter()
            .filter(|f| f.color.r < 25)
            .any(|face| {
                let point = face
                    .indices
                    .map(|i| planet.model.vertices[i])
                    .into_iter()
                    .sum::<Vec3>()
                    / 3.0;
                let direction = point.normalize();
                let hit = world.move_ship(direction * 125.0, -direction * 100.0, 0.0, 0.1);
                !hit.contacts.is_empty()
                    && hit.position.norm() < 0.985 * scale + crate::collision::SHIP_RADIUS
            });
        assert!(
            submerged_channel,
            "Los canales deben tener menor altura y colision"
        );
        let roof = planet
            .model
            .faces
            .iter()
            .filter(|f| f.color.r > 220)
            .find_map(|face| {
                let [a, b, c] = face.indices.map(|i| planet.model.vertices[i] * scale);
                let center = (a + b + c) / 3.0;
                let normal = (b - a).cross(&(c - a)).normalize();
                (center.norm() > 1.10 * scale && normal.dot(&center.normalize()) > 0.6)
                    .then_some((center, normal))
            })
            .expect("Debe haber cubiertas elevadas en los icebergs");
        let hit = world.move_ship(
            roof.0 + roof.1 * (crate::collision::SHIP_RADIUS + 2.0),
            -roof.1 * 3.0,
            0.0,
            0.1,
        );
        assert!(
            !hit.contacts.is_empty(),
            "Los icebergs deben actuar como obstaculos"
        );
        space.configure(0);
        assert!(!space.active_planets().any(|p| p.name == "Nacar"));
    }

    #[test]
    fn prismas_ground_uses_one_neutral_material_for_continuous_fragment_coloring() {
        let mut space = Space::load().unwrap();
        space.configure(2);
        let model = &space
            .active_planets()
            .find(|p| p.name == "Prisma")
            .unwrap()
            .model;
        let neutral = model
            .faces
            .iter()
            .filter(|face| face.color == crate::color::Color::new(255, 255, 255))
            .count();
        assert_eq!(
            neutral, 5120,
            "Toda la corteza debe compartir el material del shader continuo"
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
                ((transform.position - space.sun_position(seconds).unwrap()).norm() - 1390.0).abs()
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
            assert!(((transform.position - sun).norm() - 1120.0).abs() < 0.003);
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
            assert!(((transform.position - sun).norm() - 620.0).abs() < 0.002);
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
            assert!(((planet.position - star.position).norm() - 860.0).abs() < 0.002);
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
        assert!(space.debris.iter().all(|rock| {
            (rock.transform.position - sun.transform.position).norm() - rock.transform.radius
                > sun.transform.radius + 100.0
        }));
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
        assert_eq!(star.model.textures.len(), 1);
        for face in &star.model.faces {
            assert!(face.unlit && face.texture == Some(0));
            let uv = face
                .uv
                .expect("Asterion requiere las UV exportadas en Blender");
            let min = uv.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
            let max = uv.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
            assert!(max - min <= 0.5, "La costura no debe cruzar todo el atlas");
        }
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
        for name in ["Nivalis", "Onice", "Vespera", "Prisma", "Nacar", "Umbriel"] {
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
        assert_eq!(space.active_planets().count(), 7);
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
        assert_eq!(
            space.solar_lighting(0.0).unwrap().position,
            crate::galaxy::HELIX_CENTER
        );
    }
    #[test]
    fn helix_frontier_terrain_has_deep_basins_raised_ranges_and_rotating_mesh_contacts() {
        let mut space = Space::load().unwrap();
        for (name, axis) in [
            ("Micelia", Vec3::new(0.25, 0.28, 1.0)),
            ("Azufria", Vec3::new(0.12, 0.15, 1.0)),
            ("Ferrita", Vec3::new(0.16, 0.25, 1.0)),
        ] {
            let planet = space.active_planets().find(|p| p.name == name).unwrap();
            assert_eq!(planet.model.center, Vec3::zeros());
            assert!(planet.model.textures.is_empty());
            // The first object is the continuous crust; decorations are separate objects.
            let crust = &planet.model.vertices[..10242];
            let peak = crust
                .iter()
                .max_by(|a, b| a.norm().total_cmp(&b.norm()))
                .unwrap();
            assert!(peak.norm() > 1.10, "{name} needs mountains in the mesh");
            assert!(
                crust.iter().any(|v| v.norm() < 0.90),
                "{name} needs depressed crater floors"
            );
            let scale = planet.transform.radius / planet.model.radius;
            for seconds in [0.0, 53.0] {
                let transform = planet.world_transform(seconds);
                let rotate = |d: Vec3| {
                    let (s, c) = transform.yaw.sin_cos();
                    Vec3::new(d.x * c + d.z * s, d.y, d.z * c - d.x * s).normalize()
                };
                let probe = |direction: Vec3| {
                    let direction = rotate(direction);
                    let hit = space.collisions.move_ship(
                        transform.position + direction * (transform.radius + 12.0),
                        -direction * (transform.radius * 0.65),
                        seconds,
                        0.0,
                    );
                    assert!(hit.contacts.iter().any(|c| c.planet == name));
                    (hit.position - transform.position).norm()
                };
                let basin = probe(axis);
                let ridge = probe(*peak);
                assert!(
                    ridge - basin > scale * 0.10,
                    "{name}: collisions must follow the crater, not a bounding sphere"
                );
            }
            let mut flight = space.new_flight();
            space.approach(&mut flight, name).unwrap();
            let reference = flight.navigation_reference().unwrap();
            assert_eq!(reference.name, name);
            assert!((reference.surface_radius - scale).abs() < 0.001);
            assert!(
                flight.toggle_local_navigation(),
                "{name} must support local flight"
            );
        }
        for index in [1, 2, 0] {
            space.configure(index);
            for name in ["Micelia", "Azufria", "Ferrita"] {
                assert_eq!(space.active_planets().any(|p| p.name == name), index == 0);
            }
        }
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
            // El sol ya puede provocar una pequena deriva durante el primer cuadro.
            assert!((flight.position - crate::flight::SPAWN_POSITION).norm() < 0.01);
            assert_eq!(space.debris.len(), definition.debris);
            let first_debris: Vec<_> = space.debris.iter().map(|t| t.transform.position).collect();
            space.configure(galaxy);
            assert_eq!(
                first_debris,
                space
                    .debris
                    .iter()
                    .map(|t| t.transform.position)
                    .collect::<Vec<_>>()
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
            if fast.respawn_notice > 0.0 {
                break;
            }
        }
        assert!(fast.crash.is_none() && fast.respawn_notice > 0.0);
        assert_eq!(fast.position, crate::flight::SPAWN_POSITION);
        // The planet has moved during the explosion and respawn delay.
        space.approach(&mut fast, "Siroco").unwrap();
        for _ in 0..180 {
            fast.advance(1.0 / 60.0, (1.0, 0.0), true, false);
            if fast.crash.is_some() {
                break;
            }
        }
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
    fn boreal_preserves_its_lit_turquoise_cloud_map_in_the_new_orbit() {
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
        let view = FlightView::new(&flight, fb.width, fb.height).looking_at(
            planet.transform.position + Vec3::z() * 140.0,
            planet.transform.position,
        );
        let center = view.project(planet.transform.position).unwrap();
        assert!((center.x - 640.0).abs() < 1.0);
        assert!((center.y - 420.0).abs() < 1.0);
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
    fn jovara_preserves_its_blue_polar_texture_in_the_new_orbit() {
        let space = Space::load().unwrap();
        let planet = space.planets.iter().find(|p| p.name == "Jovara").unwrap();
        assert_eq!(planet.model.textures.len(), 1);
        let flight = Flight::new();
        let mut frame = Framebuffer::new(1280, 840);
        let view = FlightView::new(&flight, frame.width, frame.height).looking_at(
            planet.transform.position + Vec3::z() * 120.0,
            planet.transform.position,
        );
        renderer::draw_world_model(&mut frame, &planet.model, &view, planet.transform);
        let point = view.project(planet.transform.position).unwrap();
        assert!((point.y - 420.0).abs() < 1.0);
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
