use crate::{camera::Camera, color::Color, lighting::HullShadow, model::Model};
use std::{error::Error, path::PathBuf};

pub struct Ship {
    pub name: &'static str,
    pub accent: u32,
    pub finish: Option<Color>,
    pub portal_capable: bool,
    pub impulse_capable: bool,
    pub model: Model,
    pub shadows: HullShadow,
    pub camera: Camera,
    pub lights: crate::lighting::ShipLights,
}

pub fn load() -> Result<Vec<Ship>, Box<dyn Error>> {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/ships");
    [
        ("Benatar", "Benatar.obj", 0xf1b671, None, false),
        ("TIE Silencer", "Galactwar.obj", 0x8eb3fa, None, false),
        (
            "Crucer espacial",
            "Crucer_espacial.obj",
            0xe0b63d,
            None,
            true,
        ),
    ]
    .into_iter()
    .map(|(name, file, accent, finish, portal_capable)| {
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
            portal_capable,
            impulse_capable: name == "TIE Silencer",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benatar_keeps_a_small_geometry_budget_and_uses_its_own_materials() {
        let ships = load().unwrap();
        let ship = &ships[0];
        assert_eq!(ship.name, "Benatar");
        assert!(ship.finish.is_none());
        assert!(
            ship.model.faces.len() <= 4200,
            "La Benatar debe respetar el presupuesto del renderizador CPU"
        );
        assert!(ship.model.textures.is_empty());
        let colors: std::collections::HashSet<_> = ship
            .model
            .faces
            .iter()
            .map(|face| face.color.to_hex())
            .collect();
        assert!(
            colors.len() >= 6,
            "Cabina, casco y mecanismos deben conservar materiales distintos"
        );
        assert!(ship.model.faces.iter().any(|face| face.opacity < 1.0));
        assert!(ship.model.faces.iter().any(|face| face.unlit));
        assert!(
            ship.model
                .vertices
                .iter()
                .all(|v| v.iter().all(|c| c.is_finite()))
        );
    }
}
