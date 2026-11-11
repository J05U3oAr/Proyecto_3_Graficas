use crate::{
    collision::PlanetCollider,
    framebuffer::Framebuffer,
    model::Model,
    renderer::{self, FlightView, WorldTransform},
    shaders::SurfaceEffect,
};
use nalgebra_glm::Vec3;
use std::{error::Error, path::PathBuf, sync::OnceLock};

const NAMES: [&str; 6] = [
    "pequeno_fragmento",
    "pequeno_erosionado",
    "mediano_alargado",
    "mediano_crateres",
    "grande_macizo",
    "grande_fracturado",
];

pub struct Meshes {
    pub detailed: Model,
    pub distant: Model,
    pub collision: PlanetCollider,
}

#[derive(Clone, Copy)]
pub struct Instance {
    pub transform: WorldTransform,
    pub variant: usize,
}

impl Instance {
    pub fn rotation_speed(self) -> f32 {
        0.035 / (1.0 + self.variant as f32)
    }
}

pub fn load() -> Result<&'static [Meshes], Box<dyn Error>> {
    static CACHE: OnceLock<Result<Vec<Meshes>, String>> = OnceLock::new();
    match CACHE.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/asteroids");
        NAMES
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let detailed =
                    Model::load(root.join(format!("{name}.obj"))).map_err(|e| e.to_string())?;
                let mut distant =
                    Model::load(root.join(format!("{name}_lod.obj"))).map_err(|e| e.to_string())?;
                // Both exports share an origin and a scale; LOD changes must not move or resize rocks.
                distant.center = detailed.center;
                distant.radius = detailed.radius;
                let collision = PlanetCollider::new(
                    ["Asteroide pequeno", "Asteroide mediano", "Asteroide grande"][index / 2],
                    &detailed,
                    WorldTransform {
                        position: Vec3::zeros(),
                        radius: 1.0,
                        yaw: 0.0,
                    },
                    0.0,
                );
                Ok(Meshes {
                    detailed,
                    distant,
                    collision,
                })
            })
            .collect()
    }) {
        Ok(meshes) => Ok(meshes),
        Err(error) => Err(error.clone().into()),
    }
}

impl Meshes {
    pub fn for_view(&self, view: &FlightView, transform: WorldTransform) -> &Model {
        let distance = (view.eye() - transform.position).norm().max(0.1);
        if transform.radius * renderer::FLIGHT_FOCAL / distance < 12.0 {
            &self.distant
        } else {
            &self.detailed
        }
    }
}

/// Only instance placement happens in Rust. All triangles and craters come from Blender.
pub fn distribute(galaxy: usize, mut clear: impl FnMut(WorldTransform) -> bool) -> Vec<Instance> {
    let definition = &crate::galaxy::GALAXIES[galaxy];
    let mut seed = definition.seed;
    let mut next = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed as f32 / u32::MAX as f32
    };
    let center = if galaxy == 2 {
        crate::galaxy::UMBRIA_CENTER
    } else {
        Vec3::zeros()
    };
    let mut instances: Vec<Instance> = Vec::with_capacity(definition.debris);
    for _ in 0..definition.debris * 300 {
        if instances.len() == definition.debris {
            break;
        }
        // Reserve a full, evenly covered belt before placing Umbria's scattered rocks.
        let umbria_belt = galaxy == 2 && instances.len() < 320;
        let category = next();
        let (small_limit, medium_limit) = if umbria_belt {
            (0.50, 0.87)
        } else {
            (0.65, 0.92)
        };
        let size = if category < small_limit {
            0
        } else if category < medium_limit {
            1
        } else {
            2
        };
        let variant = size * 2 + usize::from(next() >= 0.5);
        let radius = match size {
            0 => 1.0 + next() * 2.0,
            1 => 4.0 + next() * 3.5,
            _ => 10.0 + next() * 6.0,
        };
        let angle_fraction = next();
        let angle = if umbria_belt {
            ((instances.len() % 32) as f32 + angle_fraction) / 32.0 * std::f32::consts::TAU
        } else {
            angle_fraction * std::f32::consts::TAU
        };
        let region = next();
        let position = if umbria_belt || (galaxy == 1 && region < 0.40) {
            let distance = if umbria_belt {
                735.0 + next() * 40.0
            } else {
                860.0 + next() * 40.0
            };
            let height = if umbria_belt {
                -20.0 + next() * 40.0
            } else {
                -42.0 + next() * 65.0
            };
            center + Vec3::new(angle.cos() * distance, height, angle.sin() * distance)
        } else {
            let near_departure = region < 0.72;
            let origin = if near_departure {
                crate::flight::SPAWN_POSITION
            } else {
                center
            };
            let distance = if near_departure {
                140.0 + next() * 420.0
            } else if galaxy == 0 {
                180.0 + next() * 700.0
            } else {
                400.0 + next() * 2300.0
            };
            let y = next() * 1.6 - 0.8;
            let horizontal = (1.0 - y * y).sqrt();
            origin + Vec3::new(angle.cos() * horizontal, y, angle.sin() * horizontal) * distance
        };
        let transform = WorldTransform {
            position,
            radius,
            yaw: next() * std::f32::consts::TAU,
        };
        if (position - crate::flight::SPAWN_POSITION).norm() < radius + 110.0
            || !clear(transform)
            || instances.iter().any(|other| {
                (other.transform.position - position).norm() < other.transform.radius + radius + 5.0
            })
        {
            continue;
        }
        instances.push(Instance { transform, variant });
    }
    instances
}

pub fn draw(
    fb: &mut Framebuffer,
    view: &FlightView,
    meshes: &[Meshes],
    instances: &[Instance],
    sun: Option<Vec3>,
    seconds: f32,
) {
    for instance in instances {
        let mut transform = instance.transform;
        if !view.sphere_visible(transform.position, transform.radius) {
            continue;
        }
        transform.yaw += seconds * instance.rotation_speed();
        let model = meshes[instance.variant].for_view(view, transform);
        renderer::draw_world_model_shaded(fb, model, view, transform, sun, SurfaceEffect::Asteroid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blender_meshes_have_reduced_lods_and_three_distinct_size_categories() {
        let models = load().unwrap();
        assert_eq!(models.len(), 6);
        for (i, pair) in models.iter().enumerate() {
            assert!(pair.detailed.faces.len() >= [320, 1280, 5120][i / 2]);
            assert!(pair.distant.faces.len() * 3 <= pair.detailed.faces.len());
            assert!((pair.detailed.radius - pair.distant.radius).abs() < 0.00001);
        }
        let mut flight = crate::flight::Flight::new();
        flight.camera.pitch = 0.0;
        let view = FlightView::new(&flight, 640, 480);
        let close = WorldTransform {
            position: view.eye() + Vec3::new(0.0, 0.0, -70.0),
            radius: 14.0,
            yaw: 0.0,
        };
        let far = WorldTransform {
            position: view.eye() + Vec3::new(0.0, 0.0, -2000.0),
            ..close
        };
        assert!(
            models[4].for_view(&view, close).faces.len()
                > models[4].for_view(&view, far).faces.len() * 10
        );
    }

    #[test]
    fn every_galaxy_has_reproducible_scattered_rocks_and_all_six_variants() {
        for galaxy in 0..3 {
            let a = distribute(galaxy, |_| true);
            let b = distribute(galaxy, |_| true);
            assert_eq!(a.len(), crate::galaxy::GALAXIES[galaxy].debris);
            assert_eq!(
                a.iter()
                    .map(|i| (i.transform.position, i.variant))
                    .collect::<Vec<_>>(),
                b.iter()
                    .map(|i| (i.transform.position, i.variant))
                    .collect::<Vec<_>>()
            );
            for variant in 0..6 {
                assert!(a.iter().any(|i| i.variant == variant));
            }
            assert!(a.iter().any(|i| i.transform.position.y > 100.0));
            assert!(a.iter().any(|i| i.transform.position.y < -150.0));
            assert!(a.iter().any(|i| i.transform.radius > 10.0));
        }
    }
}
