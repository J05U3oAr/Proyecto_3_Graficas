use crate::{color::Color, texture::Texture};
use nalgebra_glm::{Vec2, Vec3};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub indices: [usize; 3],
    pub color: Color,
    pub uv: Option<[Vec2; 3]>,
    pub texture: Option<usize>,
    pub unlit: bool,
}

pub struct Model {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Face>,
    pub center: Vec3,
    pub radius: f32,
    pub textures: Vec<Texture>,
}

#[derive(Clone, Copy)]
struct Material {
    color: Color,
    texture: Option<usize>,
    unlit: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            color: Color::new(175, 185, 200),
            texture: None,
            unlit: false,
        }
    }
}

fn obj_index(token: &str, count: usize) -> Result<usize, Box<dyn Error>> {
    let raw = token.parse::<i64>()?;
    let resolved = if raw > 0 { raw - 1 } else { count as i64 + raw };
    if raw == 0 || resolved < 0 || resolved as usize >= count {
        return Err(format!("Índice OBJ fuera de rango: {raw}").into());
    }
    Ok(resolved as usize)
}

impl Model {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let path = path.as_ref();
        let source = fs::read_to_string(path)?;
        Self::parse(&source, path.parent().unwrap_or_else(|| Path::new(".")))
    }

    fn parse(source: &str, directory: &Path) -> Result<Self, Box<dyn Error>> {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        let mut texcoords = Vec::new();
        let mut textures = Vec::new();
        let mut texture_cache = HashMap::new();
        let mut materials = HashMap::<String, Material>::new();
        let mut current = Material::default();

        for (line_index, line) in source.lines().enumerate() {
            let mut fields = line.split('#').next().unwrap_or("").split_whitespace();
            let Some(kind) = fields.next() else { continue };
            match kind {
                "v" => {
                    let xyz: Vec<f32> = fields.take(3).map(str::parse).collect::<Result<_, _>>()?;
                    if xyz.len() != 3 || xyz.iter().any(|value| !value.is_finite()) {
                        return Err(
                            format!("Vértice inválido en la línea {}", line_index + 1).into()
                        );
                    }
                    vertices.push(Vec3::new(xyz[0], xyz[1], xyz[2]));
                }
                "vt" => {
                    let uv: Vec<f32> = fields.take(2).map(str::parse).collect::<Result<_, _>>()?;
                    if uv.len() != 2 || uv.iter().any(|v| !v.is_finite()) {
                        return Err(format!("UV inválida en la línea {}", line_index + 1).into());
                    }
                    texcoords.push(Vec2::new(uv[0], uv[1]));
                }
                "mtllib" => {
                    for filename in fields {
                        let material_path = directory.join(filename);
                        if material_path.exists() {
                            materials.extend(load_materials(
                                &material_path,
                                &mut textures,
                                &mut texture_cache,
                            )?);
                        }
                    }
                }
                "usemtl" => {
                    if let Some(name) = fields.next() {
                        current = materials.get(name).copied().unwrap_or_default();
                    }
                }
                "f" => {
                    let mut indices = Vec::new();
                    let mut uvs = Vec::new();
                    for token in fields {
                        let mut parts = token.split('/');
                        indices.push(obj_index(parts.next().unwrap_or(""), vertices.len())?);
                        uvs.push(match parts.next().filter(|uv| !uv.is_empty()) {
                            Some(uv) => Some(texcoords[obj_index(uv, texcoords.len())?]),
                            None => None,
                        });
                    }
                    if indices.len() < 3 {
                        return Err(format!(
                            "Cara con menos de tres vértices en la línea {}",
                            line_index + 1
                        )
                        .into());
                    }
                    for index in 1..indices.len() - 1 {
                        faces.push(Face {
                            indices: [indices[0], indices[index], indices[index + 1]],
                            color: current.color,
                            uv: match [uvs[0], uvs[index], uvs[index + 1]] {
                                [Some(a), Some(b), Some(c)] => Some([a, b, c]),
                                _ => None,
                            },
                            texture: current.texture,
                            unlit: current.unlit,
                        });
                    }
                }
                _ => {}
            }
        }

        if vertices.is_empty() || faces.is_empty() {
            return Err("El archivo OBJ no contiene geometría".into());
        }

        let mut min = vertices[0];
        let mut max = vertices[0];
        for vertex in &vertices[1..] {
            min = min.zip_map(vertex, f32::min);
            max = max.zip_map(vertex, f32::max);
        }
        let center = (min + max) * 0.5;
        let radius = vertices
            .iter()
            .map(|vertex| (vertex - center).norm())
            .fold(0.0, f32::max)
            .max(f32::EPSILON);

        Ok(Self {
            vertices,
            faces,
            center,
            radius,
            textures,
        })
    }
}

fn load_materials(
    path: &PathBuf,
    textures: &mut Vec<Texture>,
    cache: &mut HashMap<PathBuf, usize>,
) -> Result<HashMap<String, Material>, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let mut materials = HashMap::new();
    let mut current_name: Option<String> = None;
    for (line_index, line) in source.lines().enumerate() {
        let mut fields = line.split('#').next().unwrap_or("").split_whitespace();
        match fields.next() {
            Some("newmtl") => {
                current_name = fields.next().map(str::to_owned);
                if let Some(name) = &current_name {
                    materials.insert(name.clone(), Material::default());
                }
            }
            Some("Kd") => {
                let channels: Vec<f32> =
                    fields.take(3).map(str::parse).collect::<Result<_, _>>()?;
                if channels.len() != 3 || channels.iter().any(|value| !value.is_finite()) {
                    return Err(
                        format!("Kd inválido en {}:{}", path.display(), line_index + 1).into(),
                    );
                }
                if let Some(name) = &current_name {
                    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
                    materials.get_mut(name).unwrap().color = Color::new(
                        channel(channels[0]),
                        channel(channels[1]),
                        channel(channels[2]),
                    );
                }
            }
            Some("map_Kd") => {
                if let Some(name) = &current_name {
                    let filename = fields.collect::<Vec<_>>().join(" ");
                    let image_path = path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(filename);
                    let index = if let Some(&index) = cache.get(&image_path) {
                        index
                    } else {
                        let index = textures.len();
                        textures.push(Texture::load(&image_path)?);
                        cache.insert(image_path, index);
                        index
                    };
                    materials.get_mut(name).unwrap().texture = Some(index);
                }
            }
            Some("illum") => {
                if let Some(name) = &current_name {
                    materials.get_mut(name).unwrap().unlit = fields.next() == Some("0");
                }
            }
            _ => {}
        }
    }
    Ok(materials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_loads_relative_texture_maps_negative_uv_indices_and_unlit_materials() {
        let model = Model::load(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/uv-checker.obj"),
        )
        .unwrap();
        assert_eq!(model.textures.len(), 1);
        let face = &model.faces[0];
        assert_eq!(face.indices, [0, 1, 2]);
        assert_eq!(
            face.uv.unwrap(),
            [
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0)
            ]
        );
        assert!(face.unlit);
        assert_eq!(
            model.textures[face.texture.unwrap()].sample(Vec2::new(0.0, 1.0)),
            Color::new(255, 0, 0)
        );
    }

    #[test]
    fn invalid_uv_coordinates_and_references_are_rejected() {
        for data in [
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt NaN 0\nf 1/1 2/1 3/1\n",
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nf 1/0 2/1 3/1\n",
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nf 1/2 2/1 3/1\n",
        ] {
            assert!(Model::parse(data, Path::new(".")).is_err());
        }
    }
}
