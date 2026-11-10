//! Load the Blender-authored Nivalis mesh around its geographical origin.
use crate::model::Model;
use nalgebra_glm::Vec3;
use std::{error::Error, path::Path};

pub fn lake_axis() -> Vec3 {
    Vec3::new(0.18, 0.12, 1.0).normalize()
}

pub fn load(path: impl AsRef<Path>) -> Result<Model, Box<dyn Error>> {
    let mut model = Model::load(path)?;
    // Asymmetric glaciers shift the bounding-box center; rotation must use the
    // planet's original origin so the frozen lake and shader stay aligned.
    model.center = Vec3::zeros();
    model.radius = model.vertices.iter().map(Vec3::norm).fold(0.0, f32::max);
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/planets/nivalis/nivalis.obj"
        ))
        .unwrap()
    }
    #[test]
    fn frozen_lake_is_planar_and_glaciers_are_large_solid_geometry() {
        let model = model();
        let axis = lake_axis();
        let mut lake_vertices = 0;
        let mut peaks = 0;
        for point in &model.vertices {
            if (point.normalize() - axis).norm() < 0.34 {
                assert!((point.dot(&axis) - 0.965).abs() < 1e-5);
                lake_vertices += 1;
            }
            if point.norm() > 1.13 {
                peaks += 1;
            }
        }
        assert!(lake_vertices > 100);
        assert!(peaks > 100, "Los glaciares deben modificar la geometria");
        for face in &model.faces {
            let [a, b, c] = face.indices.map(|i| model.vertices[i]);
            assert!((b - a).cross(&(c - a)).dot(&(a + b + c)) > 0.0);
        }
    }

    #[test]
    fn ship_collides_with_glacier_walls_and_the_frozen_lake() {
        use crate::{
            collision::{CollisionWorld, PlanetCollider, SHIP_RADIUS},
            renderer::WorldTransform,
        };
        let model = model();
        let scale = 65.0 / model.radius;
        let world = CollisionWorld::new(vec![PlanetCollider::new(
            "Nivalis",
            &model,
            WorldTransform {
                position: Vec3::zeros(),
                radius: 65.0,
                yaw: 0.0,
            },
            0.0,
        )]);
        let lake = world.move_ship(lake_axis() * 80.0, -lake_axis() * 50.0, 0.0, 0.1);
        assert_eq!(lake.contacts.first().unwrap().planet, "Nivalis");
        assert!((lake.position.dot(&lake_axis()) - (0.965 * scale + SHIP_RADIUS)).abs() < 0.05);
        let peak = model
            .faces
            .iter()
            .map(|face| {
                face.indices
                    .map(|i| model.vertices[i])
                    .into_iter()
                    .sum::<Vec3>()
                    / 3.0
            })
            .find(|point| point.norm() > 1.14)
            .unwrap();
        let outward = peak.normalize();
        // A short approach checks the obstacle itself before a long thrust can
        // legitimately slide the ship down its slope into a valley.
        let start = peak * scale + outward * (SHIP_RADIUS + 4.0);
        let glacier = world.move_ship(start, -outward * 5.0, 0.0, 0.1);
        assert!(!glacier.contacts.is_empty());
        assert!(
            glacier.position.norm() > lake.position.norm() + 5.0,
            "La colision debe incluir la altura de los glaciares"
        );
    }

    #[test]
    fn glaciers_form_separate_massifs_instead_of_planetary_bands() {
        let model = model();
        let elevated: Vec<_> = model.vertices.iter().map(|p| p.norm() > 1.065).collect();
        let mut adjacent = vec![Vec::new(); model.vertices.len()];
        for face in &model.faces {
            for [a, b] in [
                [face.indices[0], face.indices[1]],
                [face.indices[1], face.indices[2]],
                [face.indices[2], face.indices[0]],
            ] {
                if elevated[a] && elevated[b] {
                    adjacent[a].push(b);
                    adjacent[b].push(a);
                }
            }
        }
        let mut visited = vec![false; model.vertices.len()];
        let mut massifs = 0;
        for start in 0..model.vertices.len() {
            if !elevated[start] || visited[start] {
                continue;
            }
            let mut stack = vec![start];
            visited[start] = true;
            let mut size = 0;
            while let Some(vertex) = stack.pop() {
                size += 1;
                for &next in &adjacent[vertex] {
                    if !visited[next] {
                        visited[next] = true;
                        stack.push(next);
                    }
                }
            }
            if size > 10 {
                massifs += 1;
            }
        }
        assert!(
            massifs >= 8,
            "Deben existir varios macizos separados: {massifs}"
        );
    }
}
