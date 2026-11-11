use crate::{
    color::Color,
    model::{Face, Model},
    noise,
};
use nalgebra_glm::Vec3;
use std::{collections::HashMap, error::Error, path::Path};

pub const SEED: i32 = 2606;
pub const SUBDIVISIONS: u32 = 1;
pub const RELIEF: f32 = 0.10;
pub const VOLCANO_COUNT: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VolcanoKind {
    Cone,
    Shield,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volcano {
    pub kind: VolcanoKind,
    pub direction: Vec3,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct GeometryStats {
    pub source_triangles: usize,
    pub terrain_triangles: usize,
    pub object_triangles: usize,
    pub cones: usize,
    pub shields: usize,
}

fn terrain(direction: Vec3) -> f32 {
    let offset = Vec3::new(SEED as f32 * 0.017, 7.0, 19.0);
    let broad = noise::noise(direction * 3.5 + offset) - 0.5;
    let ridges = noise::noise(direction * 9.0 + offset * 0.37) - 0.5;
    1.0 + RELIEF * (broad * 1.4 + ridges * 0.45)
}

pub fn volcanoes() -> Vec<Volcano> {
    let mut result: Vec<Volcano> = Vec::with_capacity(VOLCANO_COUNT);
    // Los candidatos cubren la esfera; la semilla y el ruido deciden las ubicaciones.
    let mut candidates: Vec<_> = (0..512).collect();
    candidates.sort_by(|&a, &b| noise::hash(a, SEED, 1).total_cmp(&noise::hash(b, SEED, 1)));
    for index in candidates {
        let y = 1.0 - 2.0 * (index as f32 + 0.5) / 512.0;
        let angle = index as f32 * 2.399_963 + SEED as f32 * 0.001;
        let horizontal = (1.0 - y * y).sqrt();
        let direction = Vec3::new(horizontal * angle.cos(), y, horizontal * angle.sin());
        if noise::noise(direction * 4.0 + Vec3::new(3.0, SEED as f32 * 0.01, 7.0)) < 0.36
            || result
                .iter()
                .any(|v| (v.direction - direction).norm() < 0.62)
        {
            continue;
        }
        let kind = if result.len().is_multiple_of(2) {
            VolcanoKind::Cone
        } else {
            VolcanoKind::Shield
        };
        let variation = noise::hash(index, SEED, 9);
        let (width, height) = match kind {
            VolcanoKind::Cone => (0.12 + variation * 0.035, 0.20 + variation * 0.07),
            VolcanoKind::Shield => (0.24 + variation * 0.035, 0.075 + variation * 0.025),
        };
        result.push(Volcano {
            kind,
            direction,
            width,
            height,
        });
        if result.len() == VOLCANO_COUNT {
            break;
        }
    }
    result
}

fn midpoint(
    vertices: &mut Vec<Vec3>,
    cache: &mut HashMap<(usize, usize), usize>,
    a: usize,
    b: usize,
) -> usize {
    let key = (a.min(b), a.max(b));
    *cache.entry(key).or_insert_with(|| {
        let index = vertices.len();
        vertices.push((vertices[a] + vertices[b]).normalize());
        index
    })
}

fn subdivide(model: &mut Model) {
    let mut cache = HashMap::new();
    let mut faces = Vec::with_capacity(model.faces.len() * 4);
    for face in &model.faces {
        let [a, b, c] = face.indices;
        let ab = midpoint(&mut model.vertices, &mut cache, a, b);
        let bc = midpoint(&mut model.vertices, &mut cache, b, c);
        let ca = midpoint(&mut model.vertices, &mut cache, c, a);
        let triangles = [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]];
        let uv = face.uv.map(|[a, b, c]| {
            let ab = (a + b) * 0.5;
            let bc = (b + c) * 0.5;
            let ca = (c + a) * 0.5;
            [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]
        });
        for (i, indices) in triangles.into_iter().enumerate() {
            faces.push(Face {
                indices,
                uv: uv.map(|uv| uv[i]),
                ..*face
            });
        }
    }
    model.faces = faces;
}

fn add_face(model: &mut Model, indices: [usize; 3], color: Color, unlit: bool) {
    model.faces.push(Face {
        indices,
        color,
        uv: None,
        texture: None,
        unlit,
        opacity: 1.0,
    });
}

fn add_volcano(model: &mut Model, volcano: Volcano) {
    let d = volcano.direction;
    let right = d
        .cross(&if d.y.abs() < 0.9 {
            Vec3::y()
        } else {
            Vec3::x()
        })
        .normalize();
    let tangent = d.cross(&right);
    let (w, h) = (volcano.width, volcano.height);
    let profile: Vec<(f32, f32)> = match volcano.kind {
        VolcanoKind::Cone => vec![
            (w, -0.10),
            (w * 0.92, 0.0),
            (w * 0.70, h * 0.35),
            (w * 0.38, h * 0.92),
            (w * 0.34, h),
            (w * 0.20, h * 0.85),
            (w * 0.13, h * 0.54),
        ],
        VolcanoKind::Shield => vec![
            (w, -0.10),
            (w * 0.85, 0.0),
            (w * 0.70, h * 0.35),
            (w * 0.53, h * 0.94),
            (w * 0.41, h * 0.94),
            (w * 0.32, h * 0.43),
        ],
    };
    let ground = terrain(d);
    let start = model.vertices.len();
    const SEGMENTS: usize = 32;
    for (ring, &(radius, elevation)) in profile.iter().enumerate() {
        for segment in 0..SEGMENTS {
            let angle = segment as f32 * std::f32::consts::TAU / SEGMENTS as f32;
            let irregularity =
                1.0 + (angle * 3.0 + d.x * 9.0).sin() * 0.06 + (angle * 7.0).cos() * 0.025;
            let crest = if ring >= 2 && ring < profile.len() - 1 {
                (angle * 5.0 + d.z * 7.0).sin() * h * 0.04
            } else {
                0.0
            };
            model.vertices.push(
                d * (ground + elevation + crest)
                    + (right * angle.cos() + tangent * angle.sin()) * radius * irregularity,
            );
        }
    }
    for ring in 0..profile.len() - 1 {
        let hot = ring == 3;
        let color = if hot {
            Color::new(255, 52, 45)
        } else if volcano.kind == VolcanoKind::Cone {
            Color::new(65 + ring as u8 * 7, 13 + ring as u8 * 2, 24)
        } else {
            Color::new(83 + ring as u8 * 5, 17 + ring as u8 * 2, 27)
        };
        for segment in 0..SEGMENTS {
            let angle = segment as f32 * std::f32::consts::TAU / SEGMENTS as f32;
            let channel = ring > 0 && (angle * 3.0 + d.y * 11.0).sin() > 0.94;
            let color = if channel {
                Color::new(238, 24, 34)
            } else {
                color
            };
            let next = (segment + 1) % SEGMENTS;
            let a = start + ring * SEGMENTS + segment;
            let b = start + ring * SEGMENTS + next;
            let c = start + (ring + 1) * SEGMENTS + next;
            let e = start + (ring + 1) * SEGMENTS + segment;
            add_face(model, [a, b, c], color, hot || channel);
            add_face(model, [a, c, e], color, hot || channel);
        }
    }
    // El fondo de lava y la base cierran cada objeto; la base queda enterrada en el terreno.
    for (ring, elevation, top) in [
        (0, profile[0].1, false),
        (profile.len() - 1, profile.last().unwrap().1, true),
    ] {
        let center = model.vertices.len();
        model.vertices.push(d * (ground + elevation));
        for segment in 0..SEGMENTS {
            let a = start + ring * SEGMENTS + segment;
            let b = start + ring * SEGMENTS + (segment + 1) % SEGMENTS;
            let indices = if top { [a, b, center] } else { [b, a, center] };
            add_face(
                model,
                indices,
                if top {
                    Color::new(255, 174, 140)
                } else {
                    Color::new(48, 9, 20)
                },
                top,
            );
        }
    }
}

pub fn generate(mut model: Model, subdivisions: u32) -> (Model, GeometryStats) {
    let source_triangles = model.faces.len();
    for point in &mut model.vertices {
        *point = (*point - model.center).normalize();
    }
    // La subdivison y la deformacion se ejecutan en Rust antes de construir las colisiones y rasterizar.
    for _ in 0..subdivisions {
        subdivide(&mut model);
    }
    for point in &mut model.vertices {
        *point *= terrain(*point);
    }
    let terrain_triangles = model.faces.len();
    let objects = volcanoes();
    for object in &objects {
        add_volcano(&mut model, *object);
    }
    model.center = Vec3::zeros();
    model.radius = model.vertices.iter().map(Vec3::norm).fold(0.0, f32::max);
    let stats = GeometryStats {
        source_triangles,
        terrain_triangles,
        object_triangles: model.faces.len() - terrain_triangles,
        cones: objects
            .iter()
            .filter(|v| v.kind == VolcanoKind::Cone)
            .count(),
        shields: objects
            .iter()
            .filter(|v| v.kind == VolcanoKind::Shield)
            .count(),
    };
    (model, stats)
}

pub fn load(path: impl AsRef<Path>, subdivisions: u32) -> Result<Model, Box<dyn Error>> {
    let (model, stats) = generate(Model::load(path)?, subdivisions);
    debug_assert_eq!(
        stats.source_triangles * 4_usize.pow(subdivisions),
        stats.terrain_triangles
    );
    debug_assert_eq!(
        stats.terrain_triangles + stats.object_triangles,
        model.faces.len()
    );
    debug_assert_eq!(stats.cones + stats.shields, VOLCANO_COUNT);
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> Model {
        Model::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/planets/volcarona/volcarona.obj"
        ))
        .unwrap()
    }

    #[test]
    fn rust_subdivision_creates_new_geometry_and_noise_changes_the_surface() {
        let (model, stats) = generate(base(), SUBDIVISIONS);
        assert_eq!(
            stats.terrain_triangles,
            stats.source_triangles * 4_usize.pow(SUBDIVISIONS)
        );
        assert!(stats.object_triangles > 1000);
        assert_eq!(stats.cones, 8);
        assert_eq!(stats.shields, 8);
        assert!(model.radius > 1.20);
        let terrain_vertices = model.faces[..stats.terrain_triangles]
            .iter()
            .flat_map(|f| f.indices)
            .map(|i| model.vertices[i].norm());
        let (mut lo, mut hi) = (f32::INFINITY, 0.0_f32);
        for r in terrain_vertices {
            lo = lo.min(r);
            hi = hi.max(r);
        }
        assert!(hi - lo > 0.05);
        for face in &model.faces[..stats.terrain_triangles] {
            let [a, b, c] = face.indices.map(|i| model.vertices[i]);
            assert!((b - a).cross(&(c - a)).dot(&(a + b + c)) > 0.0);
            assert!(
                face.uv
                    .unwrap()
                    .iter()
                    .all(|uv| uv.iter().all(|v| (0.0..=1.0).contains(v)))
            );
        }
        let (coarse, _) = generate(base(), 0);
        assert!((coarse.radius - model.radius).abs() < 0.00001);
        assert!(coarse.faces.len() * 2 < model.faces.len());
    }

    #[test]
    fn both_volcano_families_are_distinct_and_reproducible_on_the_curved_ground() {
        let a = volcanoes();
        assert_eq!(a, volcanoes());
        assert_eq!(a.len(), VOLCANO_COUNT);
        for (i, v) in a.iter().enumerate() {
            assert!((v.direction.norm() - 1.0).abs() < 0.00001);
            assert!(if v.kind == VolcanoKind::Cone {
                v.height > v.width
            } else {
                v.width > v.height * 2.0
            });
            assert!(
                a[..i]
                    .iter()
                    .all(|p| (p.direction - v.direction).norm() >= 0.62)
            );
        }
        let (model, _) = generate(base(), SUBDIVISIONS);
        let cone = a.iter().find(|v| v.kind == VolcanoKind::Cone).unwrap();
        let d = cone.direction;
        let right = d
            .cross(&if d.y.abs() < 0.9 {
                Vec3::y()
            } else {
                Vec3::x()
            })
            .normalize();
        let crest = d * (terrain(d) + cone.height) + right * (cone.width * 0.34);
        let collider =
            crate::collision::CollisionWorld::new(vec![crate::collision::PlanetCollider::new(
                "Volcarona",
                &model,
                crate::renderer::WorldTransform {
                    position: Vec3::zeros(),
                    radius: model.radius * 60.0,
                    yaw: 0.0,
                },
                0.0,
            )]);
        let hit = collider.move_ship(crest * 60.0 + d * 10.0, -d * 25.0, 0.0, 0.1);
        assert_eq!(hit.contacts.first().unwrap().planet, "Volcarona");
        assert!(hit.position.dot(&d) > terrain(d) * 60.0 + cone.height * 60.0 * 0.8);
    }
}
