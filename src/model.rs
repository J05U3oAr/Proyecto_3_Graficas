use crate::color::Color;
use nalgebra_glm::Vec3;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub indices: [usize; 3],
    pub color: Color,
}

pub struct Model {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Face>,
    pub center: Vec3,
    pub radius: f32,
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
        let mut materials = HashMap::<String, Color>::new();
        let mut current_color = Color::new(175, 185, 200);

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
                "mtllib" => {
                    for filename in fields {
                        let material_path = directory.join(filename);
                        if material_path.exists() {
                            materials.extend(load_materials(&material_path)?);
                        }
                    }
                }
                "usemtl" => {
                    if let Some(name) = fields.next() {
                        current_color = materials
                            .get(name)
                            .copied()
                            .unwrap_or(Color::new(175, 185, 200));
                    }
                }
                "f" => {
                    let mut indices = Vec::new();
                    for token in fields {
                        let raw_index = token.split('/').next().unwrap_or("").parse::<i64>()?;
                        if raw_index == 0 {
                            return Err(
                                format!("Índice OBJ cero en la línea {}", line_index + 1).into()
                            );
                        }
                        let resolved = if raw_index > 0 {
                            raw_index - 1
                        } else {
                            vertices.len() as i64 + raw_index
                        };
                        if resolved < 0 || resolved as usize >= vertices.len() {
                            return Err(format!(
                                "Índice OBJ fuera de rango en la línea {}",
                                line_index + 1
                            )
                            .into());
                        }
                        indices.push(resolved as usize);
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
                            color: current_color,
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
        })
    }
}

fn load_materials(path: &PathBuf) -> Result<HashMap<String, Color>, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let mut materials = HashMap::new();
    let mut current_name: Option<String> = None;
    for (line_index, line) in source.lines().enumerate() {
        let mut fields = line.split('#').next().unwrap_or("").split_whitespace();
        match fields.next() {
            Some("newmtl") => current_name = fields.next().map(str::to_owned),
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
                    materials.insert(
                        name.clone(),
                        Color::new(
                            channel(channels[0]),
                            channel(channels[1]),
                            channel(channels[2]),
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    Ok(materials)
}
