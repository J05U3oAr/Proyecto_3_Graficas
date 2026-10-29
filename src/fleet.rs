use crate::{camera::Camera, color::Color, model::Model};
use std::{error::Error, path::PathBuf};

pub struct Ship {
    pub name: &'static str,
    pub accent: u32,
    pub finish: Option<Color>,
    pub model: Model,
    pub camera: Camera,
}

pub fn load() -> Result<Vec<Ship>, Box<dyn Error>> {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/ships");
    Ok(vec![
        Ship {
            name: "Guardgalaxy",
            accent: 0xf1b671,
            finish: Some(Color::new(218, 135, 62)),
            model: Model::load(assets.join("Guardgalaxy.obj"))?,
            camera: Camera::new(),
        },
        Ship {
            name: "Galactwar",
            accent: 0x8eb3fa,
            finish: None,
            model: Model::load(assets.join("Galactwar.obj"))?,
            camera: Camera::new(),
        },
        Ship {
            name: "Lightflash",
            accent: 0x6fe5d2,
            finish: None,
            model: Model::load(assets.join("Lightflash.obj"))?,
            camera: Camera::new(),
        },
    ])
}
