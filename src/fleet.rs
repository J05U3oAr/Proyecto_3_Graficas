use crate::{camera::Camera, color::Color, lighting::HullShadow, model::Model};
use std::{error::Error, path::PathBuf};

pub struct Ship {
    pub name: &'static str,
    pub accent: u32,
    pub finish: Option<Color>,
    pub model: Model,
    pub shadows: HullShadow,
    pub camera: Camera,
    pub lights: crate::lighting::ShipLights,
}

pub fn load() -> Result<Vec<Ship>, Box<dyn Error>> {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/ships");
    [
        (
            "Guardgalaxy",
            "Guardgalaxy.obj",
            0xf1b671,
            Some(Color::new(218, 135, 62)),
        ),
        ("TIE Silencer", "Galactwar.obj", 0x8eb3fa, None),
        ("Crucer espacial", "Crucer_espacial.obj", 0xe0b63d, None),
    ]
    .into_iter()
    .map(|(name, file, accent, finish)| {
        let model = Model::load(assets.join(file))?;
        let shadows = HullShadow::new(&model);
        // Las salidas estan en la parte posterior de la cabina, en coordenadas del OBJ.
        let exhausts = if name == "TIE Silencer" {
            [-0.34, 0.34]
                .map(|x| (nalgebra_glm::Vec3::new(x, 0.0, -1.78) - model.center) / model.radius)
                .to_vec()
        } else {
            Vec::new()
        };
        Ok(Ship {
            name,
            accent,
            finish,
            model,
            shadows,
            camera: Camera::new(),
            lights: crate::lighting::ShipLights {
                accent: Color::new((accent >> 16) as u8, (accent >> 8) as u8, accent as u8),
                exhausts,
            },
        })
    })
    .collect()
}
