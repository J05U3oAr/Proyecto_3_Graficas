//! Preserve the 96 Blender-authored ring rocks as individual moving meshes.
use crate::{collision::PlanetCollider, model::Model, renderer::WorldTransform};
use nalgebra_glm::Vec3;
use std::{error::Error, fs, path::PathBuf, sync::OnceLock};

pub struct Fragment {
    pub model: Model,
    pub collider: PlanetCollider,
}

impl Fragment {
    pub fn transform(&self, planet: WorldTransform, body_radius: f32) -> WorldTransform {
        let scale = planet.radius / body_radius;
        let p = self.model.center * scale;
        let (sin, cos) = planet.yaw.sin_cos();
        WorldTransform {
            position: planet.position
                + Vec3::new(p.x * cos + p.z * sin, p.y, p.z * cos - p.x * sin),
            radius: self.model.radius * scale,
            yaw: planet.yaw,
        }
    }
}

pub fn load() -> Result<&'static [Fragment], Box<dyn Error>> {
    static MODELS: OnceLock<Result<Vec<Fragment>, String>> = OnceLock::new();
    match MODELS.get_or_init(|| load_parts().map_err(|e| e.to_string())) {
        Ok(parts) => Ok(parts),
        Err(error) => Err(error.clone().into()),
    }
}

fn load_parts() -> Result<Vec<Fragment>, Box<dyn Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/planets/umbriel/anillos.obj");
    let source = fs::read_to_string(&path)?;
    let full = Model::load(&path)?;
    let mut ranges = Vec::new();
    let (mut vertices, mut faces) = (0, 0);
    let mut current = None;
    for line in source.lines().chain(std::iter::once("o END")) {
        if let Some(name) = line.strip_prefix("o ") {
            if let Some((start_vertex, start_face)) = current.take() {
                ranges.push((start_vertex..vertices, start_face..faces));
            }
            if name.contains("asteroide_helado_") {
                current = Some((vertices, faces));
            }
        } else if line.starts_with("v ") {
            vertices += 1;
        } else if line.starts_with("f ") {
            if line.split_whitespace().count() != 4 {
                return Err("Los anillos deben exportarse triangulados desde Blender".into());
            }
            faces += 1;
        }
    }
    if ranges.len() != 96 {
        return Err(format!(
            "Se esperaban 96 rocas de anillo, se encontraron {}",
            ranges.len()
        )
        .into());
    }
    let mut fragments = Vec::new();
    for (vertex_range, face_range) in ranges {
        let vertices = full.vertices[vertex_range.clone()].to_vec();
        let mut faces = full.faces[face_range].to_vec();
        for face in &mut faces {
            for index in &mut face.indices {
                if !vertex_range.contains(index) {
                    return Err("Una roca de anillo referencia otro objeto".into());
                }
                *index -= vertex_range.start;
            }
        }
        let min = vertices
            .iter()
            .copied()
            .reduce(|a, b| a.zip_map(&b, f32::min))
            .unwrap();
        let max = vertices
            .iter()
            .copied()
            .reduce(|a, b| a.zip_map(&b, f32::max))
            .unwrap();
        let center = (min + max) * 0.5;
        let radius = vertices
            .iter()
            .map(|p| (p - center).norm())
            .fold(0.0, f32::max);
        let model = Model {
            vertices,
            faces,
            center,
            radius,
            textures: Vec::new(),
        };
        let collider = PlanetCollider::new(
            "Anillos de Umbriel",
            &model,
            WorldTransform {
                position: Vec3::zeros(),
                radius: 1.0,
                yaw: 0.0,
            },
            0.0,
        );
        fragments.push(Fragment { model, collider });
    }
    Ok(fragments)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_96_separate_rocks_preserve_every_opaque_triangle_from_blender() {
        let parts = load().unwrap();
        let full = Model::load(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/planets/umbriel/anillos.obj"),
        )
        .unwrap();
        assert_eq!(parts.len(), 96);
        assert_eq!(
            parts.iter().map(|f| f.model.faces.len()).sum::<usize>(),
            full.faces.iter().filter(|f| f.opacity == 1.0).count()
        );
        assert!(
            parts
                .iter()
                .all(|f| f.model.faces.iter().all(|face| face.opacity == 1.0))
        );
    }
}
